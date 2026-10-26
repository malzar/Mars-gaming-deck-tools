# Compila MGDC y genera el zip instalable en OpenDeck (dist\com.malzar.mgdc.sdPlugin.zip)
# Uso: desde la carpeta mgdc ->  powershell -ExecutionPolicy Bypass -File .\package.ps1

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "La compilación ha fallado" }

$name = 'com.malzar.mgdc.sdPlugin'
$dist = Join-Path $PSScriptRoot 'dist'
$out  = Join-Path $dist $name

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $out | Out-Null

Copy-Item manifest.json, LICENSE -Destination $out
Copy-Item -Recurse assets -Destination $out
Copy-Item target\release\mgdc.exe -Destination $out

Compress-Archive -Path $out -DestinationPath (Join-Path $dist "$name.zip") -Force
Write-Host "Listo: $dist\$name.zip"
