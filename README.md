# ZHiPRaR — Windows

ZHiPRaR is a Windows-first Rust command-line prototype for the experimental ZHiP-X lossless format.

## Build on Windows

1. Install Rust with https://rustup.rs.
2. Install Visual Studio Build Tools with the MSVC C++ workload.
3. Run `zhiprar\build.cmd`.
4. Find `ZHiPRaR.exe` and `zhipx.dll` in `zhiprar\dist\`.

## Commands

```cmd
ZHiPRaR.exe pack input.bin output.zhip
ZHiPRaR.exe unpack output.zhip restored.bin
ZHiPRaR.exe verify output.zhip
```

This branch contains only the Windows project and its build files. It does not contain the web app or server.

## Status

ZHiP-X is an early experimental RLE/raw codec, not yet comparable to ZSTD and not compatible with the ZHiPManager v1-v4 archive container. Do not use it as the only copy of important files. The workflow builds and tests the native project on Windows and publishes EXE/DLL build artifacts.
