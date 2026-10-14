# ZHiPRaR for Windows

ZHiPRaR is a minimal Windows prototype with a JavaScript-friendly native Rust core.

## Architecture

- `ZHiPRaR.exe`: command-line pack, unpack and verify utility.
- `zhipx.dll`: exported C ABI entry points for JavaScript/native integration.
- `ZHiP-X`: experimental lossless codec with raw fallback and a small RLE token format.
- `build.cmd`: builds the EXE and DLL for Windows x64.

## Build

1. Install Rust using https://rustup.rs.
2. Install the Visual Studio Build Tools C++ workload for the MSVC linker.
3. Run `build.cmd`.
4. Artifacts appear in `dist\`.

## Usage

```cmd
ZHiPRaR.exe pack input.bin output.zhip
ZHiPRaR.exe unpack output.zhip restored.bin
ZHiPRaR.exe verify output.zhip
```

## Important status

This is a codec prototype, not yet a replacement for ZSTD. It uses raw storage when RLE does not reduce size and verifies restored bytes with CRC32. It is expected to perform poorly on already-compressed images, video and audio. No lossless codec can guarantee reducing every such file by 50%.

The archive format in this folder is **ZHiP-X v1**, not the existing ZHiPManager v1-v4 container. Compatibility between the two formats has not been implemented yet. Keep test files and originals until integration and cross-version tests are complete.

The DLL functions `zhipx_compress` and `zhipx_decompress` accept caller-provided buffers. If a buffer is too small, the function returns the negative required capacity; a null pointer or invalid archive returns -1.


Windows build automation is defined in `.github/workflows/build-windows.yml`; successful runs publish `ZHiPRaR-Windows-x64.zip` as a workflow artifact.
