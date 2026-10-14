@echo off
setlocal
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust toolchain not found.
  echo Install Rust from https://rustup.rs and select the MSVC toolchain.
  pause
  exit /b 1
)
echo Building ZHiPRaR for Windows x64...
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
if errorlevel 1 (
  echo Build failed.
  pause
  exit /b 1
)
if not exist dist mkdir dist
copy /y "target\x86_64-pc-windows-msvc\release\ZHiPRaR.exe" "dist\ZHiPRaR.exe" >nul
copy /y "target\x86_64-pc-windows-msvc\release\zhipx.dll" "dist\zhipx.dll" >nul
echo.
echo Build complete: zhiprar\dist
echo Commands:
echo   ZHiPRaR.exe pack input.bin output.zhip
echo   ZHiPRaR.exe unpack output.zhip restored.bin
echo   ZHiPRaR.exe verify output.zhip
pause
