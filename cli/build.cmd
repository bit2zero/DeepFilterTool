@echo off
rem Windows 向けビルド。外部クレートを使わないため cargo だけで完結します。
rem Rust の MSVC ツールチェーンには Visual Studio Build Tools が必要です。
setlocal
cd /d "%~dp0"
cargo build --release
if errorlevel 1 exit /b %errorlevel%
echo.
echo 完成: %~dp0target\release\deepfilter-tool.exe
echo 次に実行: target\release\deepfilter-tool.exe setup
