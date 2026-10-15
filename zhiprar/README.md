# ZHiPManager Native Windows App

The `ZHiPRaR.exe` application is a native Windows GUI using the same `.zhip` container structure as the ZHiPManager web app.

## Build

1. Install Rust stable with target `x86_64-pc-windows-msvc`.
2. Install Visual Studio Build Tools with the **Desktop development with C++** workload.
3. Run `build.cmd`.
4. Output files appear in `dist\`.

The build script recognizes the custom Rust paths `E:\RustUp\.rustup` and `E:\RustUp\.cargo` if those directories exist.

## Usage

- **Choose files** selects multiple files at once.
- **Choose folder** adds the selected folder's files using relative archive paths.
- **Create .zhip archive** creates a web-compatible ZHIP v2 archive for DEFLATE modes or ZHIP v4 for ULTRA ZSTD.
- **Choose archive** and **Output folder** prepare extraction. **Extract files** opens ZHIP v1–v4 archives and checks the CRC32 of every file.
- ZHIP v3 archives also need a valid ECDSA signature.
- Extraction blocks unsafe paths and refuses to overwrite existing files.

## Compression modes

- Fast: DEFLATE level 1 plus raw fallback.
- Smart hybrid: compare DEFLATE levels 1 and 6 with raw storage.
- Maximum DEFLATE: compare levels 1, 6 and 9 with raw storage.
- ULTRA ZSTD: compare DEFLATE levels 1/6/9, Zstandard levels 19–22 and raw storage.

All candidates are round-trip checked before selection. Combined input is limited to 400 MB. Processing is local; the app does not upload selected files.

## DLL

`zhipx.dll` retains the small single-file C ABI wrapper for native integration. The GUI uses the archive APIs directly. The Windows x64 workflow runs the codec tests and publishes `ZHiPRaR-Windows-x64.zip` as an artifact.
