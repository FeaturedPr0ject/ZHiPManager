# ZHiPManager — Windows

ZHiPManager for Windows is a native desktop application with a dark, Liquid Glass-inspired interface. It creates multi-file `.zhip` archives and opens archives produced by the web version.

## Features

- Select multiple files or add a whole folder.
- Choose Fast, Smart hybrid, Maximum DEFLATE, or ULTRA ZSTD compression.
- Create web-compatible ZHIP v2 archives for DEFLATE modes and ZHIP v4 archives for Zstandard mode.
- Open ZHIP v1–v4 archives, verify CRC32 for each entry, and verify the ECDSA signature on v3 archives.
- Extract into a chosen folder with path-traversal protection and no silent overwrites.
- All file processing happens locally.

The desktop app follows the ZHIP container used by the web version. It does not create ZIP files, and `.zhip` is not the ZIP format.

## Build on Windows

1. Install Rust with the MSVC toolchain.
2. Install Visual Studio Build Tools with the **Desktop development with C++** workload.
3. Run `zhiprar\build.cmd`.
4. Find `ZHiPRaR.exe` and `zhipx.dll` in `zhiprar\dist\`.

If Rust was installed under `E:\RustUp\`, the build script uses `E:\RustUp\.rustup` and `E:\RustUp\.cargo` when those folders exist.

GitHub Actions also tests the codec and publishes a Windows x64 build artifact from the `windows-zhiprar` branch.

## Format compatibility

| Mode | Container | Codecs tested |
|---|---|---|
| Fast | ZHIP v2 | DEFLATE level 1 and raw fallback |
| Smart hybrid | ZHIP v2 | DEFLATE levels 1 and 6, raw fallback |
| Maximum DEFLATE | ZHIP v2 | DEFLATE levels 1, 6 and 9, raw fallback |
| ULTRA ZSTD | ZHIP v4 | DEFLATE levels 1/6/9 and Zstandard levels 19–22, raw fallback |

Each compressed candidate is round-trip checked and CRC32-verified before it can be selected. ZHIP v3 archives are opened only when their embedded ECDSA signature verifies.

The desktop release is limited to 400 MB combined input to match the web version's standard tier. Keep a backup of important originals until you have confirmed the archive can be opened on your devices.
