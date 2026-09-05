@echo off
rem 実エンジンを使う統合テスト（GUI 経由）をビルドする。
setlocal
cd /d "%~dp0"
if exist Verify.exe del Verify.exe
"%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:exe /main:Verify /out:Verify.exe ^
  /reference:System.Windows.Forms.dll /reference:System.Drawing.dll ^
  AudioCore.cs App.cs Verify.cs
if errorlevel 1 exit /b %errorlevel%
echo Built: Verify.exe
