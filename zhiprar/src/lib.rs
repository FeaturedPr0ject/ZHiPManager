use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use std::io::{Read, Write};

const MAGIC: &[u8; 4] = b"ZHIP";
const VERSION_DEFLATE: u8 = 2;
const VERSION_ZSTD: u8 = 4;
const MAX_ENTRIES: usize = 100_000;
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMode {
    Fast,
    Smart,
    Maximum,
    Ultra,
}

impl CompressionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Smart => "Smart hybrid",
            Self::Maximum => "Maximum DEFLATE",
            Self::Ultra => "ULTRA ZSTD",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub data: Vec<u8>,
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn safe_archive_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    !normalized.is_empty()
        && !normalized.starts_with('/')
        && !normalized.starts_with("//")
        && !normalized.as_bytes().contains(&0)
        && !normalized.starts_with("\\")
        && !normalized.starts_with("./")
        && !normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !normalized
            .split('/')
            .next()
            .unwrap_or_default()
            .contains(':')
}

fn deflate(data: &[u8], level: u32) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(level));
    encoder
        .write_all(data)
        .map_err(|error| format!("DEFLATE compression failed: {error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("DEFLATE compression failed: {error}"))
}

fn inflate(data: &[u8], expected_size: usize) -> Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(data);
    let max_read = u64::try_from(expected_size)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut output = Vec::with_capacity(expected_size.min(16 * 1024 * 1024));
    decoder
        .by_ref()
        .take(max_read)
        .read_to_end(&mut output)
        .map_err(|error| format!("DEFLATE extraction failed: {error}"))?;
    if output.len() != expected_size {
        return Err("Expanded size does not match the archive header".into());
    }
    Ok(output)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = input
        .get(offset..offset.saturating_add(2))
        .ok_or("Archive header is truncated")?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = input
        .get(offset..offset.saturating_add(4))
        .ok_or("Archive header is truncated")?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn compress_payload(raw: &[u8], mode: CompressionMode) -> Result<(u8, Vec<u8>), String> {
    let levels: &[u32] = match mode {
        CompressionMode::Fast => &[1],
        CompressionMode::Smart => &[1, 6],
        CompressionMode::Maximum | CompressionMode::Ultra => &[1, 6, 9],
    };

    let checksum = crc32(raw);
    let mut best = raw.to_vec();
    let mut best_codec = 0u8;

    for &level in levels {
        let candidate = deflate(raw, level)?;
        let restored = inflate(&candidate, raw.len())?;
        if crc32(&restored) != checksum {
            return Err(format!("DEFLATE level {level} failed CRC32 verification"));
        }
        if candidate.len() < best.len() {
            best = candidate;
            best_codec = 1;
        }
    }

    if mode == CompressionMode::Ultra {
        for level in [19, 20, 21, 22] {
            let candidate = zstd::bulk::compress(raw, level)
                .map_err(|error| format!("Zstandard level {level} failed: {error}"))?;
            let restored = zstd::bulk::decompress(&candidate, raw.len())
                .map_err(|error| format!("Zstandard level {level} verification failed: {error}"))?;
            if restored.len() != raw.len() || crc32(&restored) != checksum {
                return Err(format!("Zstandard level {level} failed CRC32 verification"));
            }
            if candidate.len() < best.len() {
                best = candidate;
                best_codec = 2;
            }
        }
    }

    Ok((best_codec, best))
}

pub fn compress_archive(files: &[ArchiveEntry], mode: CompressionMode) -> Result<Vec<u8>, String> {
    compress_archive_with_progress(files, mode, |_, _| {})
}

pub fn compress_archive_with_progress(
    files: &[ArchiveEntry],
    mode: CompressionMode,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<u8>, String> {
    if files.is_empty() {
        return Err("Choose at least one file to create an archive".into());
    }
    if files.len() > MAX_ENTRIES || files.len() > u32::MAX as usize {
        return Err("The archive contains too many files".into());
    }

    let total_input: u64 = files.iter().try_fold(0u64, |sum, file| {
        let size = u64::try_from(file.data.len()).map_err(|_| "A file is too large")?;
        sum.checked_add(size).ok_or("Combined input size is too large")
    })?;
    if total_input > 400 * 1024 * 1024 {
        return Err("Combined input exceeds the 400 MB desktop limit".into());
    }

    let version = if mode == CompressionMode::Ultra {
        VERSION_ZSTD
    } else {
        VERSION_DEFLATE
    };
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    output.push(version);
    output.extend_from_slice(&(files.len() as u32).to_le_bytes());

    for (index, file) in files.iter().enumerate() {
        if !safe_archive_path(&file.path) {
            return Err(format!("Invalid or unsafe file path: {}", file.path));
        }
        let name = file.path.replace('\\', "/").into_bytes();
        if name.len() > u16::MAX as usize {
            return Err(format!("File path is too long: {}", file.path));
        }
        if file.data.len() > u32::MAX as usize {
            return Err(format!("File is too large for the ZHIP container: {}", file.path));
        }

        let (codec, payload) = compress_payload(&file.data, mode)?;
        if payload.len() > u32::MAX as usize {
            return Err(format!("Compressed file is too large: {}", file.path));
        }

        output.extend_from_slice(&(name.len() as u16).to_le_bytes());
        output.extend_from_slice(&(file.data.len() as u32).to_le_bytes());
        output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        output.extend_from_slice(&crc32(&file.data).to_le_bytes());
        output.push(codec);
        output.extend_from_slice(&name);
        output.extend_from_slice(&payload);
        progress(index + 1, files.len());
    }

    Ok(output)
}

pub fn decompress_archive(input: &[u8]) -> Result<Vec<ArchiveEntry>, String> {
    if input.len() < 9 || input.get(..4) != Some(MAGIC.as_slice()) {
        return Err("Invalid ZHIP signature".into());
    }

    let version = input[4];
    if !(1..=4).contains(&version) {
        return Err(format!("Unsupported ZHIP version: {version}"));
    }

    let count = read_u32(input, 5)? as usize;
    if count > MAX_ENTRIES {
        return Err("Archive contains an unreasonable number of entries".into());
    }

    let mut offset = 9usize;
    let mut total_expanded = 0u64;
    let mut entries = Vec::with_capacity(count.min(4096));

    for _ in 0..count {
        let fixed_size = if version == 1 { 14usize } else { 15usize };
        if offset.checked_add(fixed_size).filter(|end| *end <= input.len()).is_none() {
            return Err("Archive is truncated in an entry header".into());
        }

        let name_length = read_u16(input, offset)? as usize;
        let original_length = read_u32(input, offset + 2)? as usize;
        let compressed_length = read_u32(input, offset + 6)? as usize;
        let expected_crc = read_u32(input, offset + 10)?;
        let codec = if version == 1 { 1 } else { input[offset + 14] };
        offset += fixed_size;

        let name_end = offset.checked_add(name_length).ok_or("Invalid file name length")?;
        let payload_end = name_end.checked_add(compressed_length).ok_or("Invalid compressed length")?;
        if name_length == 0 || payload_end > input.len() {
            return Err("Invalid entry lengths or truncated archive".into());
        }

        let name = std::str::from_utf8(&input[offset..name_end])
            .map_err(|_| "Archive contains a file name that is not valid UTF-8")?
            .replace('\\', "/");
        if !safe_archive_path(&name) {
            return Err(format!("Unsafe path rejected: {name}"));
        }
        offset = name_end;

        total_expanded = total_expanded
            .checked_add(original_length as u64)
            .ok_or("Expanded data size overflow")?;
        if total_expanded > MAX_EXPANDED_BYTES {
            return Err("Expanded data exceeds the 2 GiB safety limit".into());
        }

        let compressed = &input[offset..payload_end];
        let data = match codec {
            0 => {
                if compressed.len() != original_length {
                    return Err(format!("Raw payload size does not match header for {name}"));
                }
                compressed.to_vec()
            }
            1 => inflate(compressed, original_length)?,
            2 if version == 4 => zstd::bulk::decompress(compressed, original_length)
                .map_err(|error| format!("Zstandard extraction failed for {name}: {error}"))?,
            _ => return Err(format!("Unsupported compression codec {codec} in ZHIP v{version}")),
        };

        if data.len() != original_length {
            return Err(format!("Size verification failed for {name}"));
        }
        if crc32(&data) != expected_crc {
            return Err(format!("CRC32 integrity check failed for {name}"));
        }

        entries.push(ArchiveEntry { path: name, data });
        offset = payload_end;
    }

    if version == 3 {
        let trailer_header = input
            .get(offset..offset.saturating_add(8))
            .ok_or("This ZHIP v3 archive has no digital signature")?;
        if &trailer_header[..4] != b"SIGZ" {
            return Err("Digital signature trailer is missing".into());
        }
        let public_length = u16::from_le_bytes([trailer_header[4], trailer_header[5]]) as usize;
        let signature_length = u16::from_le_bytes([trailer_header[6], trailer_header[7]]) as usize;
        let public_start = offset + 8;
        let signature_start = public_start.checked_add(public_length).ok_or("Invalid signature lengths")?;
        let trailer_end = signature_start.checked_add(signature_length).ok_or("Invalid signature lengths")?;
        if public_length == 0 || signature_length == 0 || trailer_end != input.len() {
            return Err("Invalid digital signature block".into());
        }

        let public_key = VerifyingKey::from_public_key_der(&input[public_start..signature_start])
            .map_err(|_| "Invalid ECDSA public key in signature block")?;
        let signature = Signature::from_slice(&input[signature_start..trailer_end])
            .map_err(|_| "Invalid ECDSA signature encoding")?;
        public_key
            .verify(&input[..offset], &signature)
            .map_err(|_| "Digital signature verification failed. The archive may have been changed.")?;
    } else if offset != input.len() {
        return Err("Unexpected trailing data after ZHIP entries".into());
    }

    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

pub fn compress(input: &[u8]) -> Vec<u8> {
    let entry = ArchiveEntry {
        path: "file.bin".into(),
        data: input.to_vec(),
    };
    compress_archive(&[entry], CompressionMode::Fast).unwrap_or_default()
}

pub fn decompress(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut entries = decompress_archive(input)?;
    if entries.len() != 1 {
        return Err("This archive contains multiple files; use decompress_archive".into());
    }
    Ok(entries.remove(0).data)
}

#[no_mangle]
pub unsafe extern "C" fn zhipx_compress(
    input: *const u8,
    input_len: usize,
    output: *mut u8,
    output_capacity: usize,
) -> isize {
    if input.is_null() || output.is_null() {
        return -1;
    }
    let source = std::slice::from_raw_parts(input, input_len);
    let result = compress(source);
    if result.is_empty() || result.len() > output_capacity {
        return -(result.len().max(1) as isize);
    }
    std::ptr::copy_nonoverlapping(result.as_ptr(), output, result.len());
    result.len() as isize
}

#[no_mangle]
pub unsafe extern "C" fn zhipx_decompress(
    input: *const u8,
    input_len: usize,
    output: *mut u8,
    output_capacity: usize,
) -> isize {
    if input.is_null() || output.is_null() {
        return -1;
    }
    let source = std::slice::from_raw_parts(input, input_len);
    let result = match decompress(source) {
        Ok(data) => data,
        Err(_) => return -1,
    };
    if result.len() > output_capacity {
        return -(result.len() as isize);
    }
    std::ptr::copy_nonoverlapping(result.as_ptr(), output, result.len());
    result.len() as isize
}

#[cfg(test)]
mod tests {
    use super::{compress_archive, decompress_archive, ArchiveEntry, CompressionMode};

    fn files() -> Vec<ArchiveEntry> {
        vec![
            ArchiveEntry {
                path: "notes.txt".into(),
                data: b"ZHiPManager archive compatibility test".repeat(30),
            },
            ArchiveEntry {
                path: "images/sample.bin".into(),
                data: (0u8..=255).cycle().take(4096).collect(),
            },
        ]
    }

    #[test]
    fn creates_web_compatible_v2_archive() {
        let archive = compress_archive(&files(), CompressionMode::Smart).unwrap();
        assert_eq!(&archive[..4], b"ZHIP");
        assert_eq!(archive[4], 2);
        assert_eq!(u32::from_le_bytes(archive[5..9].try_into().unwrap()), 2);
        assert_eq!(decompress_archive(&archive).unwrap(), files());
    }

    #[test]
    fn creates_web_compatible_v4_archive() {
        let archive = compress_archive(&files(), CompressionMode::Ultra).unwrap();
        assert_eq!(&archive[..4], b"ZHIP");
        assert_eq!(archive[4], 4);
        assert_eq!(decompress_archive(&archive).unwrap(), files());
    }

    #[test]
    fn rejects_invalid_signature() {
        assert!(decompress_archive(b"ZHRX-not-a-zhip-file").is_err());
    }

    #[test]
    fn rejects_tampered_entry() {
        let mut archive = compress_archive(&files(), CompressionMode::Fast).unwrap();
        let last = archive.len() - 1;
        archive[last] ^= 0x55;
        assert!(decompress_archive(&archive).is_err());
    }
}
