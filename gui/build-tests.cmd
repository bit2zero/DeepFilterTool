@echo off
rem 単体テストをビルドする。エンジンもモデルも不要。
rem
rem App.cs も取り込む。画面の読み上げ対応（AccessibleName）を検査するために
rem FilterForm を生成するため。画面は表示せず、生成して調べるだけなので
rem エンジンは要らない。
setlocal
cd /d "%~dp0"
if exist ..\Tests.exe del ..\Tests.exe
rem App.cs にも Main があるため、入口を明示する。
"%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:exe /main:TestRunner /out:..\Tests.exe ^
  /reference:System.Windows.Forms.dll /reference:System.Drawing.dll ^
  AudioCore.cs App.cs Tests.cs
if errorlevel 1 exit /b %errorlevel%
echo Built: %~dp0..\Tests.exe
echo 実行: Tests.exe            ^(すべて^)
echo       Tests.exe Read_       ^(名前で絞り込み^)
