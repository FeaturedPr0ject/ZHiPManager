const MAGIC: &[u8; 4] = b"ZHRX";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 18;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn encode_rle(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let mut run = 1usize;
        while i + run < input.len() && input[i + run] == input[i] && run < 130 {
            run += 1;
        }
        if run >= 3 {
            out.push(0x80 | ((run - 3) as u8));
            out.push(input[i]);
            i += run;
            continue;
        }
        let start = i;
        i += run;
        while i < input.len() && i - start < 128 {
            let mut next_run = 1usize;
            while i + next_run < input.len() && input[i + next_run] == input[i] && next_run < 3 {
                next_run += 1;
            }
            if next_run >= 3 {
                break;
            }
            i += next_run;
        }
        let len = i - start;
        out.push((len - 1) as u8);
        out.extend_from_slice(&input[start..i]);
    }
    out
}

fn decode_rle(input: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(expected.min(64 * 1024 * 1024));
    let mut i = 0usize;
    while i < input.len() {
        let token = input[i];
        i += 1;
        if token & 0x80 != 0 {
            if i >= input.len() {
                return Err("Truncated RLE run".into());
            }
            let count = (token as usize & 0x7f) + 3;
            let value = input[i];
            i += 1;
            if out.len().checked_add(count).filter(|n| *n <= expected).is_none() {
                return Err("Expanded data exceeds declared size".into());
            }
            out.resize(out.len() + count, value);
        } else {
            let count = token as usize + 1;
            let end = i.checked_add(count).ok_or("Invalid literal length")?;
            if end > input.len() || out.len().checked_add(count).filter(|n| *n <= expected).is_none() {
                return Err("Truncated or oversized RLE literal".into());
            }
            out.extend_from_slice(&input[i..end]);
            i = end;
        }
    }
    if out.len() != expected {
        return Err("Expanded size does not match header".into());
    }
    Ok(out)
}

pub fn compress(input: &[u8]) -> Vec<u8> {
    let rle = encode_rle(input);
    let (codec, payload) = if rle.len() < input.len() { (1u8, rle.as_slice()) } else { (0u8, input) };
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(codec);
    out.extend_from_slice(&(input.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc32(input).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub fn decompress(input: &[u8]) -> Result<Vec<u8>, String> {
    if input.len() < HEADER_LEN || &input[..4] != MAGIC {
        return Err("Not a ZHiP-X archive".into());
    }
    if input[4] != VERSION {
        return Err(format!("Unsupported ZHiP-X version {}", input[4]));
    }
    let codec = input[5];
    let raw_len_u64 = u64::from_le_bytes(input[6..14].try_into().map_err(|_| "Invalid size")?);
    let raw_len = usize::try_from(raw_len_u64).map_err(|_| "File is too large for this system")?;
    let expected_crc = u32::from_le_bytes(input[14..18.min(input.len())].try_into().map_err(|_| "Invalid checksum")?);
    let payload = &input[18..];
    let out = match codec {
        0 => {
            if payload.len() != raw_len {
                return Err("Raw payload size does not match header".into());
            }
            payload.to_vec()
        }
        1 => decode_rle(payload, raw_len)?,
        _ => return Err(format!("Unsupported codec {}", codec)),
    };
    if crc32(&out) != expected_crc {
        return Err("CRC32 verification failed".into());
    }
    Ok(out)
}

#[no_mangle]
pub unsafe extern "C" fn zhipx_compress(input: *const u8, input_len: usize, output: *mut u8, output_capacity: usize) -> isize {
    if input.is_null() || output.is_null() {
        return -1;
    }
    let source = std::slice::from_raw_parts(input, input_len);
    let result = compress(source);
    if result.len() > output_capacity {
        return -(result.len() as isize);
    }
    std::ptr::copy_nonoverlapping(result.as_ptr(), output, result.len());
    result.len() as isize
}

#[no_mangle]
pub unsafe extern "C" fn zhipx_decompress(input: *const u8, input_len: usize, output: *mut u8, output_capacity: usize) -> isize {
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
    use super::{compress, decompress};

    #[test]
    fn round_trip_repeated_data() {
        let input = vec![b'A'; 10000];
        assert_eq!(decompress(&compress(&input)).unwrap(), input);
    }

    #[test]
    fn round_trip_binary_data() {
        let input: Vec<u8> = (0..=255).cycle().take(8193).collect();
        assert_eq!(decompress(&compress(&input)).unwrap(), input);
    }

    #[test]
    fn round_trip_empty_data() {
        let input = Vec::new();
        assert_eq!(decompress(&compress(&input)).unwrap(), input);
    }
}
