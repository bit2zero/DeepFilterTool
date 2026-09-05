@echo off
rem WaveData の単体テストをビルドする。エンジンもモデルも不要。
setlocal
cd /d "%~dp0"
if exist ..\Tests.exe del ..\Tests.exe
"%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:exe /out:..\Tests.exe AudioCore.cs Tests.cs
if errorlevel 1 exit /b %errorlevel%
echo Built: %~dp0..\Tests.exe
echo 実行: Tests.exe            ^(すべて^)
echo       Tests.exe Read_       ^(名前で絞り込み^)
