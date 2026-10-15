#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Vec2};
use rfd::FileDialog;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use zhipx::{compress_archive_with_progress, decompress_archive, ArchiveEntry, CompressionMode};

const MAX_LOG_ITEMS: usize = 100;

#[derive(Clone, Debug)]
struct SelectedFile {
    disk_path: PathBuf,
    archive_path: String,
}

enum AppEvent {
    Status(String),
    Log(String),
    Progress(f32),
    Finished(Result<String, String>),
}

struct ZhiprarApp {
    files: Vec<SelectedFile>,
    archive_name: String,
    compression: CompressionMode,
    archive_to_extract: Option<PathBuf>,
    extract_directory: Option<PathBuf>,
    status: String,
    log: Vec<String>,
    progress: f32,
    busy: bool,
    receiver: Option<Receiver<AppEvent>>,
}

impl Default for ZhiprarApp {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            archive_name: "MyArchive".into(),
            compression: CompressionMode::Smart,
            archive_to_extract: None,
            extract_directory: None,
            status: "Ready when you are. Your files stay on this device.".into(),
            log: vec!["Files are processed locally. No upload is required.".into()],
            progress: 0.0,
            busy: false,
            receiver: None,
        }
    }
}

impl ZhiprarApp {
    fn push_log(&mut self, message: impl Into<String>) {
        self.log.push(message.into());
        if self.log.len() > MAX_LOG_ITEMS {
            let remove_count = self.log.len() - MAX_LOG_ITEMS;
            self.log.drain(0..remove_count);
        }
    }

    fn send_log(sender: &Sender<AppEvent>, message: impl Into<String>) {
        let _ = sender.send(AppEvent::Log(message.into()));
    }

    fn add_files(&mut self) {
        if self.busy {
            return;
        }
        if let Some(paths) = FileDialog::new()
            .set_title("Choose files to archive")
            .pick_files()
        {
            let count_before = self.files.len();
            for path in paths {
                self.add_selected_path(path, None);
            }
            let added = self.files.len() - count_before;
            if added > 0 {
                self.status = format!("Added {added} file(s) to the archive list.");
                self.push_log(format!("Added {added} file(s)."));
            }
        }
    }

    fn add_folder(&mut self) {
        if self.busy {
            return;
        }
        if let Some(root) = FileDialog::new()
            .set_title("Choose a folder to archive")
            .pick_folder()
        {
            let mut found = Vec::new();
            match collect_files(&root, &root, &mut found) {
                Ok(()) => {
                    let before = self.files.len();
                    for file in found {
                        self.add_selected_path(file.disk_path, Some(file.archive_path));
                    }
                    let added = self.files.len() - before;
                    self.status = format!("Added {added} file(s) from the selected folder.");
                    self.push_log(format!("Added {added} file(s) from {}.", root.display()));
                }
                Err(error) => {
                    self.status = format!("Could not read folder: {error}");
                }
            }
        }
    }

    fn add_selected_path(&mut self, disk_path: PathBuf, archive_path: Option<String>) {
        if !disk_path.is_file() || self.files.iter().any(|item| item.disk_path == disk_path) {
            return;
        }
        let path = archive_path.unwrap_or_else(|| {
            disk_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        self.files.push(SelectedFile {
            disk_path,
            archive_path: path.replace('\\', "/"),
        });
    }

    fn remove_file(&mut self, index: usize) {
        if !self.busy && index < self.files.len() {
            let item = self.files.remove(index);
            self.push_log(format!("Removed {}.", item.archive_path));
            self.status = format!("{} file(s) selected.", self.files.len());
        }
    }

    fn clear_files(&mut self) {
        if !self.busy {
            self.files.clear();
            self.status = "Selection cleared.".into();
        }
    }

    fn total_size(&self) -> u64 {
        self.files
            .iter()
            .filter_map(|item| fs::metadata(&item.disk_path).ok().map(|metadata| metadata.len()))
            .fold(0u64, u64::saturating_add)
    }

    fn start_compress(&mut self) {
        if self.busy || self.files.is_empty() {
            return;
        }

        let archive_name = safe_archive_name(&self.archive_name);
        let suggested_name = format!("{archive_name}.zhip");
        let Some(output_path) = FileDialog::new()
            .set_title("Save ZHiPManager archive")
            .add_filter("ZHiP archive", &["zhip"])
            .set_file_name(&suggested_name)
            .save_file()
        else {
            self.status = "Archive creation cancelled.".into();
            return;
        };

        let files = self.files.clone();
        let mode = self.compression;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        self.progress = 0.0;
        self.status = "Preparing files for compression…".into();
        self.push_log(format!(
            "Started archive creation: {} file(s), {} input.",
            files.len(),
            format_bytes(self.total_size())
        ));

        thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let mut entries = Vec::with_capacity(files.len());
                let mut names = HashSet::new();

                for (index, file) in files.iter().enumerate() {
                    let _ = sender.send(AppEvent::Status(format!(
                        "Reading {} ({})…",
                        file.archive_path,
                        fs::metadata(&file.disk_path)
                            .map(|metadata| format_bytes(metadata.len()))
                            .unwrap_or_else(|_| "unknown size".into())
                    )));
                    let mut path = file.archive_path.replace('\\', "/");
                    if !safe_archive_path(&path) {
                        return Err(format!("Invalid archive path: {path}"));
                    }
                    if !names.insert(path.clone()) {
                        path = format!("{}_{}", index + 1, path);
                        if !names.insert(path.clone()) {
                            return Err("Could not make duplicate file names unique.".into());
                        }
                    }
                    let data = fs::read(&file.disk_path)
                        .map_err(|error| format!("Could not read {}: {error}", file.disk_path.display()))?;
                    entries.push(ArchiveEntry { path, data });
                    let _ = sender.send(AppEvent::Progress(
                        0.1 * (index + 1) as f32 / files.len() as f32,
                    ));
                }

                let archive = compress_archive_with_progress(&entries, mode, |finished, total| {
                    let _ = sender.send(AppEvent::Status(format!(
                        "Compressing and verifying files… {finished}/{total}"
                    )));
                    let _ = sender.send(AppEvent::Progress(
                        0.1 + 0.75 * finished as f32 / total.max(1) as f32,
                    ));
                })?;

                let _ = sender.send(AppEvent::Log(format!(
                    "Built ZHIP v{} archive ({}).",
                    if mode == CompressionMode::Ultra { 4 } else { 2 },
                    format_bytes(archive.len() as u64)
                )));

                let restored = decompress_archive(&archive)?;
                if restored.len() != entries.len()
                    || restored.iter().any(|item| {
                        entries
                            .iter()
                            .find(|original| original.path == item.path)
                            .map(|original| original.data != item.data)
                            .unwrap_or(true)
                    })
                {
                    return Err("Archive verification failed: restored files do not match the input.".into());
                }

                fs::write(&output_path, &archive)
                    .map_err(|error| format!("Could not save archive: {error}"))?;

                let input_size: u64 = entries.iter().map(|entry| entry.data.len() as u64).sum();
                let ratio = if input_size == 0 {
                    0.0
                } else {
                    (1.0 - archive.len() as f64 / input_size as f64) * 100.0
                };
                let _ = sender.send(AppEvent::Progress(1.0));
                Ok(format!(
                    "Done! {} created · {} → {} ({:.1}% change).",
                    output_path.display(),
                    format_bytes(input_size),
                    format_bytes(archive.len() as u64),
                    ratio
                ))
            })();

            let _ = sender.send(AppEvent::Finished(result));
        });
    }

    fn choose_archive(&mut self) {
        if self.busy {
            return;
        }
        if let Some(path) = FileDialog::new()
            .set_title("Choose a ZHiP archive")
            .add_filter("ZHiP archive", &["zhip"])
            .pick_file()
        {
            self.archive_to_extract = Some(path.clone());
            self.status = format!("Archive selected: {}", path.display());
            self.push_log(format!("Selected archive {}.", path.display()));
        }
    }

    fn choose_extract_directory(&mut self) {
        if self.busy {
            return;
        }
        if let Some(path) = FileDialog::new()
            .set_title("Choose extraction folder")
            .pick_folder()
        {
            self.extract_directory = Some(path.clone());
            self.status = format!("Extract destination: {}", path.display());
        }
    }

    fn start_extract(&mut self) {
        if self.busy {
            return;
        }
        let Some(archive_path) = self.archive_to_extract.clone() else {
            self.status = "Choose a .zhip archive first.".into();
            return;
        };
        let Some(output_directory) = self.extract_directory.clone() else {
            self.status = "Choose an extraction folder first.".into();
            return;
        };

        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        self.progress = 0.0;
        self.status = "Reading archive and checking integrity…".into();
        self.push_log(format!("Opening {}.", archive_path.display()));

        thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let bytes = fs::read(&archive_path)
                    .map_err(|error| format!("Could not read archive: {error}"))?;
                let _ = sender.send(AppEvent::Status(format!(
                    "Checking archive signature and contents… ({})",
                    format_bytes(bytes.len() as u64)
                )));
                let entries = decompress_archive(&bytes)?;
                if entries.is_empty() {
                    return Err("The archive contains no files.".into());
                }

                for (index, entry) in entries.iter().enumerate() {
                    write_entry_safely(&output_directory, entry)?;
                    let _ = sender.send(AppEvent::Log(format!(
                        "Extracted {} ({}).",
                        entry.path,
                        format_bytes(entry.data.len() as u64)
                    )));
                    let _ = sender.send(AppEvent::Progress(
                        (index + 1) as f32 / entries.len() as f32,
                    ));
                }

                Ok(format!(
                    "Extracted {} file(s) to {}. CRC32{} verified.",
                    entries.len(),
                    output_directory.display(),
                    if bytes.get(4) == Some(&3) {
                        " and ECDSA signature"
                    } else {
                        ""
                    }
                ))
            })();
            let _ = sender.send(AppEvent::Finished(result));
        });
    }

    fn poll_events(&mut self) {
        let mut completed = false;
        let mut received = Vec::new();
        if let Some(receiver) = &self.receiver {
            while let Ok(event) = receiver.try_recv() {
                received.push(event);
            }
        }
        for event in received {
            match event {
                AppEvent::Status(message) => self.status = message,
                AppEvent::Log(message) => self.push_log(message),
                AppEvent::Progress(value) => self.progress = value.clamp(0.0, 1.0),
                AppEvent::Finished(result) => {
                    match result {
                        Ok(message) => {
                            self.status = message.clone();
                            self.push_log(message);
                        }
                        Err(message) => {
                            self.status = format!("Error: {message}");
                            self.push_log(format!("ERROR: {message}"));
                        }
                    }
                    completed = true;
                }
            }
        }
        if completed {
            self.busy = false;
            self.receiver = None;
        }
    }

    fn draw_file_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("SELECTED FILES").size(11.0).strong().color(Color32::from_rgb(190, 205, 230)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_enabled(!self.busy, egui::Button::new(RichText::new("Clear all").color(Color32::from_rgb(255, 155, 170))).fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                    self.clear_files();
                }
                ui.label(
                    RichText::new(format!("{} files", self.files.len()))
                        .size(11.0)
                        .color(Color32::from_rgb(154, 170, 196)),
                );
            });
        });
        ui.add_space(8.0);

        if self.files.is_empty() {
            Frame::new()
                .fill(Color32::from_rgb(14, 23, 40))
                .stroke(Stroke::new(1.0, Color32::from_rgb(38, 54, 80)))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::same(16))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("+").size(24.0).color(Color32::from_rgb(145, 171, 255)));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Your archive starts here").strong().color(Color32::from_rgb(227, 236, 252)));
                            ui.label(RichText::new("Choose one or more files to see them listed here.").size(11.0).color(Color32::from_rgb(137, 154, 184)));
                        });
                    });
                });
        } else {
            let mut remove_index = None;
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for (index, item) in self.files.iter().enumerate() {
                    Frame::new()
                        .fill(Color32::from_rgb(21, 32, 52))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(39, 56, 82)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("FILE").size(18.0).color(Color32::from_rgb(145, 171, 255)));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&item.archive_path).strong().size(12.0).color(Color32::from_rgb(228, 237, 253)));
                                    let size = fs::metadata(&item.disk_path).map(|meta| format_bytes(meta.len())).unwrap_or_else(|_| "unavailable".into());
                                    ui.label(RichText::new(format!("{} · Local file", size)).size(10.0).color(Color32::from_rgb(135, 153, 181)));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.add_enabled(!self.busy, egui::Button::new(RichText::new("X").size(18.0).color(Color32::from_rgb(255, 155, 170))).fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                                        remove_index = Some(index);
                                    }
                                });
                            });
                        });
                    ui.add_space(6.0);
                }
            });
            if let Some(index) = remove_index {
                self.remove_file(index);
            }
        }

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!self.busy, egui::Button::new("Add files").fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                self.add_files();
            }
            if ui.add_enabled(!self.busy, egui::Button::new("Add folder").fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                self.add_folder();
            }
        });

        ui.add_space(16.0);
        ui.label(RichText::new("ARCHIVE NAME").size(11.0).strong().color(Color32::from_rgb(190, 205, 230)));
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.add_enabled(
                !self.busy,
                egui::TextEdit::singleline(&mut self.archive_name)
                    .desired_width(ui.available_width() - 67.0)
                    .hint_text("MyArchive"),
            );
            Frame::new()
                .fill(Color32::from_rgb(30, 43, 70))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(RichText::new(".zhip").strong().color(Color32::from_rgb(149, 177, 255)));
                });
        });

        ui.add_space(15.0);
        ui.label(RichText::new("COMPRESSION LEVEL").size(11.0).strong().color(Color32::from_rgb(190, 205, 230)));
        ui.add_space(5.0);
        egui::ComboBox::from_id_salt("compression-level")
            .selected_text(self.compression.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.compression, CompressionMode::Fast, "Fast");
                ui.selectable_value(&mut self.compression, CompressionMode::Smart, "Smart hybrid");
                ui.selectable_value(&mut self.compression, CompressionMode::Maximum, "Maximum DEFLATE");
                ui.selectable_value(&mut self.compression, CompressionMode::Ultra, "ULTRA ZSTD · v4");
            });
        ui.add_space(5.0);
        ui.label(
            RichText::new(match self.compression {
                CompressionMode::Fast => "Quick compression using DEFLATE level 1.",
                CompressionMode::Smart => "Compares DEFLATE levels 1 and 6; balanced size and speed.",
                CompressionMode::Maximum => "Compares DEFLATE levels 1, 6 and 9.",
                CompressionMode::Ultra => "Compares DEFLATE 1/6/9 with Zstandard 19–22; produces ZHIP v4.",
            })
            .size(10.0)
            .color(Color32::from_rgb(137, 154, 184)),
        );

        ui.add_space(16.0);
        let total = self.total_size();
        let max_size = 400 * 1024 * 1024u64;
        let ratio = (total as f32 / max_size as f32).clamp(0.0, 1.0);
        Frame::new()
            .fill(Color32::from_rgb(13, 21, 36))
            .stroke(Stroke::new(1.0, Color32::from_rgb(38, 54, 80)))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Input size").size(10.0).color(Color32::from_rgb(151, 169, 195)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} / 400 MB", format_bytes(total))).size(10.0).strong());
                    });
                });
                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(ratio).fill(Color32::from_rgb(111, 146, 255)).desired_height(7.0));
                ui.label(RichText::new(if total > max_size { "Input exceeds the desktop limit." } else if total == 0 { "Ready when you are" } else { "Files stay on your device" }).size(9.0).color(Color32::from_rgb(137, 154, 184)));
            });

        ui.add_space(10.0);
        if ui.add_enabled(!self.busy && !self.files.is_empty() && total <= max_size, egui::Button::new(RichText::new(if self.busy { "Working…" } else { "Create .zhip archive" }).strong()).fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8)).min_size(Vec2::new(ui.available_width(), 44.0))).clicked() {
            self.start_compress();
        }
    }

    fn draw_sidebar(&self, ui: &mut egui::Ui) {
        card(ui, "Made for your workflow", "LOCAL · PRIVATE", |ui| {
            stat_row(ui, "P", "Private by design", "Files are processed locally.");
            ui.add_space(8.0);
            stat_row(ui, "MAX", "Up to 400 MB", "Combined input limit.");
            ui.add_space(8.0);
            stat_row(ui, "ZIP", "Your own extension", "Archives are saved as .zhip.");
        });

        ui.add_space(12.0);
        card(ui, "How it works", "3 SIMPLE STEPS", |ui| {
            stat_row(ui, "1", "Choose files", "Select multiple files or a folder.");
            ui.add_space(10.0);
            stat_row(ui, "2", "Name your archive", "Choose a name and compression mode.");
            ui.add_space(10.0);
            stat_row(ui, "3", "Create & verify", "CRC32 and round-trip checks run locally.");
        });

        ui.add_space(12.0);
        Frame::new()
            .fill(Color32::from_rgb(19, 39, 48))
            .stroke(Stroke::new(1.0, Color32::from_rgb(43, 83, 89)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.label(RichText::new("Your files stay yours.").strong().color(Color32::from_rgb(121, 225, 210)));
                ui.add_space(5.0);
                ui.label(RichText::new("No upload, account or server is required. ZHiP v2/v4 archives can be opened in the web version.").size(10.0).color(Color32::from_rgb(172, 200, 211)));
            });
    }

    fn draw_extract(&mut self, ui: &mut egui::Ui) {
        card(ui, "Open an archive", "STEP 02 — EXTRACT", |ui| {
            ui.label(RichText::new("Inspect and extract native ZHiP v1–v4 archives. CRC32 is checked for every file; v3 archives also require a valid ECDSA signature.").size(11.0).color(Color32::from_rgb(151, 169, 195)));
            ui.add_space(12.0);

            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!self.busy, egui::Button::new("Choose archive…").fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                    self.choose_archive();
                }
                if ui.add_enabled(!self.busy, egui::Button::new("Output folder…").fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                    self.choose_extract_directory();
                }
                if ui.add_enabled(!self.busy && self.archive_to_extract.is_some() && self.extract_directory.is_some(), egui::Button::new(RichText::new("Extract files").strong()).fill(Color32::from_rgb(27, 42, 68)).stroke(Stroke::new(1.0, Color32::from_rgb(53, 74, 112))).corner_radius(CornerRadius::same(8))).clicked() {
                    self.start_extract();
                }
            });

            ui.add_space(10.0);
            Frame::new()
                .fill(Color32::from_rgb(13, 21, 36))
                .stroke(Stroke::new(1.0, Color32::from_rgb(38, 54, 80)))
                .corner_radius(CornerRadius::same(11))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.label(RichText::new("ARCHIVE").size(9.0).strong().color(Color32::from_rgb(143, 164, 200)));
                    ui.label(self.archive_to_extract.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "No archive selected.".into()));
                    ui.add_space(8.0);
                    ui.label(RichText::new("DESTINATION").size(9.0).strong().color(Color32::from_rgb(143, 164, 200)));
                    ui.label(self.extract_directory.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "No folder selected.".into()));
                });
        });
    }
}

impl eframe::App for ZhiprarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        egui::CentralPanel::default()
            .frame(Frame::new().fill(Color32::from_rgb(9, 15, 28)).inner_margin(Margin::same(18)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        Frame::new()
                            .fill(Color32::from_rgb(132, 160, 255))
                            .corner_radius(CornerRadius::same(13))
                            .inner_margin(Margin::same(10))
                            .show(ui, |ui| {
                                ui.label(RichText::new("Z").size(22.0).strong().color(Color32::from_rgb(17, 29, 57)));
                            });
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.heading(RichText::new("ZHiPManager").size(22.0).strong().color(Color32::from_rgb(239, 244, 255)));
                                ui.label(RichText::new("WINDOWS").size(9.0).strong().color(Color32::from_rgb(150, 179, 255)));
                            });
                            ui.label(RichText::new("Zone Hybrid Improvement Plan · Built for your files").size(10.0).color(Color32::from_rgb(143, 161, 190)));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            Frame::new()
                                .fill(Color32::from_rgb(17, 29, 45))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(43, 62, 88)))
                                .corner_radius(CornerRadius::same(20))
                                .inner_margin(Margin::symmetric(11, 7))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("ON").size(8.0).color(Color32::from_rgb(112, 224, 208)));
                                        ui.label(RichText::new("LOCAL PROCESSING").size(9.0).strong().color(Color32::from_rgb(201, 214, 236)));
                                    });
                                });
                        });
                    });

                    ui.add_space(23.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Your files. Your format.").size(27.0).strong().color(Color32::from_rgb(240, 245, 255)));
                        ui.add_space(4.0);
                        ui.label(RichText::new("Create, verify and open .zhip archives without uploading your files.").size(12.0).color(Color32::from_rgb(157, 172, 198)));
                    });
                    ui.add_space(22.0);

                    ui.columns(2, |columns| {
                        self.draw_file_list(&mut columns[0]);
                        columns[0].add_space(12.0);
                        draw_activity(&mut columns[0], &self.status, self.progress, &self.log);
                        self.draw_sidebar(&mut columns[1]);
                    });

                    ui.add_space(14.0);
                    self.draw_extract(ui);
                    ui.add_space(12.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("© 2026 ZHiPManager").size(10.0).color(Color32::from_rgb(126, 145, 177)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("ZHIP v1–v4 · CRC32 · Local files").size(10.0).color(Color32::from_rgb(126, 145, 177)));
                        });
                    });
                });
            });
    }
}

fn card(
    ui: &mut egui::Ui,
    title: &str,
    label: &str,
    contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::new()
        .fill(Color32::from_rgb(17, 27, 46))
        .stroke(Stroke::new(1.0, Color32::from_rgb(38, 54, 80)))
        .corner_radius(CornerRadius::same(17))
        .inner_margin(Margin::same(15))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(15.0).strong().color(Color32::from_rgb(238, 244, 255)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(label).size(8.0).strong().color(Color32::from_rgb(145, 171, 255)));
                });
            });
            ui.add_space(13.0);
            contents(ui);
        });
}

fn stat_row(ui: &mut egui::Ui, icon: &str, title: &str, description: &str) {
    ui.horizontal(|ui| {
        Frame::new()
            .fill(Color32::from_rgb(27, 42, 68))
            .corner_radius(CornerRadius::same(9))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.label(RichText::new(icon).strong().color(Color32::from_rgb(151, 177, 255)));
            });
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(11.0).strong().color(Color32::from_rgb(224, 234, 252)));
            ui.label(RichText::new(description).size(9.0).color(Color32::from_rgb(139, 157, 187)));
        });
    });
}

fn draw_activity(ui: &mut egui::Ui, status: &str, progress: f32, logs: &[String]) {
    card(ui, "Activity log", "LOCAL STATUS", |ui| {
        ui.label(RichText::new(status).size(11.0).color(Color32::from_rgb(210, 224, 244)));
        if progress > 0.0 && progress < 1.0 {
            ui.add_space(7.0);
            ui.add(egui::ProgressBar::new(progress).show_percentage().fill(Color32::from_rgb(112, 224, 208)));
        }
        ui.add_space(10.0);
        egui::ScrollArea::vertical().max_height(145.0).stick_to_bottom(true).show(ui, |ui| {
            for line in logs.iter().rev().take(14).rev() {
                let color = if line.starts_with("ERROR:") {
                    Color32::from_rgb(255, 143, 161)
                } else if line.contains("Done!") || line.contains("Extracted") {
                    Color32::from_rgb(112, 224, 208)
                } else {
                    Color32::from_rgb(144, 162, 191)
                };
                ui.label(RichText::new(format!("- {line}")).size(9.0).color(color));
            }
        });
    });
}

fn collect_files(root: &Path, current: &Path, output: &mut Vec<SelectedFile>) -> Result<(), String> {
    let items = fs::read_dir(current)
        .map_err(|error| format!("{}: {error}", current.display()))?;
    for item in items {
        let item = item.map_err(|error| error.to_string())?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_files(root, &path, output)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            output.push(SelectedFile {
                disk_path: path,
                archive_path: relative,
            });
        }
    }
    Ok(())
}

fn safe_archive_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    !normalized.is_empty()
        && !normalized.starts_with('/')
        && !normalized.starts_with("./")
        && !normalized.as_bytes().contains(&0)
        && !normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !normalized
            .split('/')
            .next()
            .unwrap_or_default()
            .contains(':')
}

fn safe_archive_name(name: &str) -> String {
    let trimmed = name.trim();
    let without_extension = trimmed.strip_suffix(".zhip").or_else(|| trimmed.strip_suffix(".ZHIP")).unwrap_or(trimmed);
    let cleaned: String = without_extension
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                character
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('.').trim();
    if cleaned.is_empty() {
        "MyArchive".into()
    } else {
        cleaned.chars().take(100).collect()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".into();
    }
    let units = ["B", "KB", "MB", "GB"];
    let index = ((bytes as f64).log(1024.0).floor() as usize).min(units.len() - 1);
    let value = bytes as f64 / 1024f64.powi(index as i32);
    if index == 0 {
        format!("{value:.0} {}", units[index])
    } else {
        format!("{value:.1} {}", units[index])
    }
}

fn write_entry_safely(root: &Path, entry: &ArchiveEntry) -> Result<(), String> {
    if !safe_archive_path(&entry.path) {
        return Err(format!("Unsafe path refused: {}", entry.path));
    }

    let components: Vec<&str> = entry.path.split('/').collect();
    if components.is_empty() {
        return Err("Archive entry has an empty path".into());
    }

    let mut current = root.to_path_buf();
    for component in &components[..components.len() - 1] {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(format!("Extraction path is not a safe directory: {}", current.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|create_error| format!("Could not create {}: {create_error}", current.display()))?;
            }
            Err(error) => return Err(format!("Could not inspect {}: {error}", current.display())),
        }
    }

    let filename = components.last().ok_or("Archive entry has an empty file name")?;
    current.push(filename);
    if fs::symlink_metadata(&current).is_ok() {
        return Err(format!("Refusing to overwrite existing file: {}", current.display()));
    }
    fs::write(&current, &entry.data)
        .map_err(|error| format!("Could not write {}: {error}", current.display()))
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1160.0, 960.0])
            .with_min_inner_size([900.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ZHiPManager",
        options,
        Box::new(|creation_context| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = Color32::from_rgb(9, 15, 28);
            visuals.window_fill = Color32::from_rgb(17, 27, 46);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(17, 27, 46);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(27, 42, 68);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 70, 109);
            visuals.widgets.active.bg_fill = Color32::from_rgb(91, 125, 230);
            visuals.widgets.disabled.bg_fill = Color32::from_rgb(16, 24, 39);
            visuals.widgets.disabled.fg_stroke.color = Color32::from_rgb(102, 119, 148);
            visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(53, 74, 112));
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(112, 146, 255));
            visuals.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(130, 158, 255));
            visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
            visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
            visuals.widgets.active.corner_radius = CornerRadius::same(8);
            visuals.selection.bg_fill = Color32::from_rgb(103, 137, 240);
            visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(224, 234, 252);
            visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(224, 234, 252);
            creation_context.egui_ctx.set_visuals(visuals);
            Ok(Box::new(ZhiprarApp::default()))
        }),
    )
}
