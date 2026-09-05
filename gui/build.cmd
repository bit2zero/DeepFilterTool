@echo off
rem GUI 本体をビルドする。Windows 同梱の C# コンパイラだけを使う。
rem
rem 出力はリポジトリ直下。実行ファイルは自分の隣にある runtime/ と
rem sessions/ を見るため、ここを変えると動かなくなる。
setlocal
cd /d "%~dp0"
if exist ..\DeepFilterTool.exe (
  echo DeepFilterTool.exe already exists. Build stopped to preserve it.
  exit /b 1
)
"%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:winexe /out:..\DeepFilterTool.exe ^
  /reference:System.Windows.Forms.dll /reference:System.Drawing.dll ^
  AudioCore.cs App.cs
if errorlevel 1 exit /b %errorlevel%
echo Built: %~dp0..\DeepFilterTool.exe
