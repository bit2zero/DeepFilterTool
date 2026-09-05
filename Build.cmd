@echo off
setlocal
cd /d "%~dp0"
if exist DeepFilterTool.exe (
  echo DeepFilterTool.exe already exists. Build stopped to preserve it.
  exit /b 1
)
"%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:winexe /out:DeepFilterTool.exe /reference:System.Windows.Forms.dll /reference:System.Drawing.dll AudioCore.cs App.cs
exit /b %errorlevel%
