#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;
use rfd::FileDialog;
use std::fs;
use std::path::PathBuf;

#[derive(Default)]
struct ZhiprarApp {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    status: String,
    busy: bool,
}

impl ZhiprarApp {
    fn choose_input(&mut self) {
        if let Some(path) = FileDialog::new().set_title("Choose a file").pick_file() {
            self.input = Some(path.clone());
            self.output = None;
            self.status = format!("Selected: {}", path.display());
        }
    }

    fn compress(&mut self) {
        let Some(input_path) = self.input.clone() else {
            self.status = "Choose a file first.".into();
            return;
        };
        let output_path = FileDialog::new()
            .set_title("Save ZHiP-X archive")
            .set_file_name(format!("{}.zhip", input_path.file_name().unwrap_or_default().to_string_lossy()))
            .add_filter("ZHiP-X archive", &["zhip"])
            .save_file();
        let Some(output_path) = output_path else {
            self.status = "Compression cancelled.".into();
            return;
        };
        self.busy = true;
        self.status = "Compressing and verifying...".into();
        match fs::read(&input_path) {
            Ok(bytes) => {
                let archive = zhipx::compress(&bytes);
                match zhipx::decompress(&archive) {
                    Ok(restored) if restored == bytes => match fs::write(&output_path, &archive) {
                        Ok(()) => {
                            let saved = if bytes.is_empty() { 0.0 } else { (1.0 - archive.len() as f64 / bytes.len() as f64) * 100.0 };
                            self.output = Some(output_path.clone());
                            self.status = format!("Done: {}\n{} bytes → {} bytes ({:.1}% change).", output_path.display(), bytes.len(), archive.len(), saved);
                        }
                        Err(e) => self.status = format!("Could not save archive: {e}"),
                    },
                    Ok(_) => self.status = "Verification failed: restored bytes differ from input.".into(),
                    Err(e) => self.status = format!("Verification failed: {e}"),
                }
            }
            Err(e) => self.status = format!("Could not read input: {e}"),
        }
        self.busy = false;
    }

    fn extract(&mut self) {
        let Some(input_path) = self.input.clone() else {
            self.status = "Choose a .zhip file first.".into();
            return;
        };
        let output_name = input_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let output_path = FileDialog::new().set_title("Save extracted file").set_file_name(output_name).save_file();
        let Some(output_path) = output_path else {
            self.status = "Extraction cancelled.".into();
            return;
        };
        self.busy = true;
        self.status = "Extracting and checking CRC32...".into();
        match fs::read(&input_path) {
            Ok(bytes) => match zhipx::decompress(&bytes) {
                Ok(restored) => match fs::write(&output_path, restored) {
                    Ok(()) => {
                        self.output = Some(output_path.clone());
                        self.status = format!("Extracted successfully:\n{}", output_path.display());
                    }
                    Err(e) => self.status = format!("Could not save extracted file: {e}"),
                },
                Err(e) => self.status = format!("Cannot extract this file: {e}"),
            },
            Err(e) => self.status = format!("Could not read archive: {e}"),
        }
        self.busy = false;
    }
}

impl eframe::App for ZhiprarApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.heading("ZHiPRaR");
                ui.label(egui::RichText::new("WINDOWS").small().color(egui::Color32::from_rgb(110, 170, 255)));
            });
            ui.label("Independent archive tool · ZHiP-X experimental lossless format");
            ui.add_space(16.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new("INPUT FILE").strong().small());
                if let Some(path) = &self.input {
                    ui.label(path.display().to_string());
                } else {
                    ui.label(egui::RichText::new("No file selected").weak());
                }
                ui.add_space(6.0);
                if ui.button("Browse files…").clicked() && !self.busy {
                    self.choose_input();
                }
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.busy && self.input.is_some(), egui::Button::new("Compress to .zhip").min_size(egui::vec2(170.0, 42.0))).clicked() {
                    self.compress();
                }
                if ui.add_enabled(!self.busy && self.input.is_some(), egui::Button::new("Extract .zhip").min_size(egui::vec2(140.0, 42.0))).clicked() {
                    self.extract();
                }
            });

            ui.add_space(14.0);
            ui.label(egui::RichText::new("ACTIVITY").strong().small());
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(100.0);
                ui.label(&self.status);
            });
            ui.add_space(10.0);
            ui.label(egui::RichText::new("ZHiP-X is experimental. It uses raw storage or RLE and may not shrink already-compressed media.").small().weak());
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(egui::RichText::new("ZHiPRaR · Windows x64").small().weak());
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 500.0])
            .with_min_inner_size([620.0, 420.0]),
            .with_title("ZHiPRaR"),
        ..Default::default()
    };
    eframe::run_native("ZHiPRaR", options, Box::new(|_cc| Ok(Box::new(ZhiprarApp::default()))))
}
