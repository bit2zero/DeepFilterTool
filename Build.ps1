$ErrorActionPreference = 'Stop'
$sources = @((Join-Path $PSScriptRoot 'AudioCore.cs'), (Join-Path $PSScriptRoot 'App.cs'))
$target = Join-Path $PSScriptRoot 'DeepFilterTool.exe'
if (Test-Path -LiteralPath $target) { throw '既存EXEがあります。上書きせず停止しました。' }
Add-Type -Path $sources -ReferencedAssemblies System.Windows.Forms,System.Drawing -OutputAssembly $target -OutputType WindowsApplication
Write-Output "Built: $target"
