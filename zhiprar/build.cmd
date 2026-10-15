@echo off
setlocal
cd /d "%~dp0"

if exist "E:\RustUp\.rustup" set "RUSTUP_HOME=E:\RustUp\.rustup"
if exist "E:\RustUp\.cargo" set "CARGO_HOME=E:\RustUp\.cargo"
if defined CARGO_HOME set "PATH=%CARGO_HOME%\bin;%PATH%"

where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust toolchain not found.
  echo Install Rust from https://rustup.rs and select the MSVC toolchain.
  pause
  exit /b 1
)

echo Building ZHiPManager for Windows x64...
rustup target add x86_64-pc-windows-msvc
if errorlevel 1 (
  echo Could not install the Windows MSVC target.
  pause
  exit /b 1
)

cargo test --target x86_64-pc-windows-msvc
if errorlevel 1 (
  echo Codec tests failed.
  pause
  exit /b 1
)

cargo build --release --target x86_64-pc-windows-msvc
if errorlevel 1 (
  echo Build failed.
  pause
  exit /b 1
)

if not exist dist mkdir dist
copy /y "target\x86_64-pc-windows-msvc\release\ZHiPRaR.exe" "dist\ZHiPRaR.exe" >nul
if errorlevel 1 (
  echo Could not copy ZHiPRaR.exe.
  pause
  exit /b 1
)
copy /y "target\x86_64-pc-windows-msvc\release\zhipx.dll" "dist\zhipx.dll" >nul
if errorlevel 1 (
  echo Could not copy zhipx.dll.
  pause
  exit /b 1
)
copy /y "README.md" "dist\README.txt" >nul

echo.
echo Build and tests completed successfully.
echo Output: %CD%\dist
pause
