use std::{env, fs, process};

fn usage() {
    eprintln!("ZHiPRaR — ZHiP-X prototype");
    eprintln!("Usage:");
    eprintln!("  ZHiPRaR.exe pack <input-file> [output.zhip]");
    eprintln!("  ZHiPRaR.exe unpack <input.zhip> [output-file]");
    eprintln!("  ZHiPRaR.exe verify <input.zhip>");
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage();
        return Err("Missing command or input file".into());
    }
    let command = args[1].to_lowercase();
    let input_path = &args[2];
    let input = fs::read(input_path).map_err(|e| format!("Cannot read input: {e}"))?;
    match command.as_str() {
        "pack" => {
            let output_path = args.get(3).cloned().unwrap_or_else(|| format!("{input_path}.zhip"));
            let packed = zhipx::compress(&input);
            let size = packed.len();
            fs::write(&output_path, packed).map_err(|e| format!("Cannot write archive: {e}"))?;
            println!("Created: {output_path}");
            println!("Input: {} bytes | Output: {} bytes | Saved: {:.2}%", input.len(), size,
                if input.is_empty() { 0.0 } else { (1.0 - size as f64 / input.len() as f64) * 100.0 });
        }
        "unpack" => {
            let output_path = args.get(3).cloned().unwrap_or_else(|| {
                input_path.strip_suffix(".zhip").or_else(|| input_path.strip_suffix(".ZHIP"))
                    .map(str::to_string).unwrap_or_else(|| format!("{input_path}.out"))
            });
            let unpacked = zhipx::decompress(&input)?;
            fs::write(&output_path, unpacked).map_err(|e| format!("Cannot write output: {e}"))?;
            println!("Extracted: {output_path}");
        }
        "verify" => {
            let unpacked = zhipx::decompress(&input)?;
            println!("OK: archive valid, {} bytes verified.", unpacked.len());
        }
        _ => {
            usage();
            return Err(format!("Unknown command: {command}"));
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}
