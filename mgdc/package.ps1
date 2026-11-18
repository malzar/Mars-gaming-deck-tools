# Compila Prisma y genera el zip instalable en OpenDeck (dist\com.malzar.prisma.sdPlugin.zip)
# Uso: desde esta carpeta ->  powershell -ExecutionPolicy Bypass -File .\package.ps1

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "La compilación ha fallado" }

$name = 'com.malzar.prisma.sdPlugin'
$dist = Join-Path $PSScriptRoot 'dist'
$out  = Join-Path $dist $name

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force $out | Out-Null

Copy-Item manifest.json, LICENSE, THIRD_PARTY.md -Destination $out
Copy-Item -Recurse assets, layouts, pi, locales -Destination $out

# Nombres de las acciones para OpenDeck en cada idioma que tiene: salen de la
# sección «opendeck» de locales\<idioma>.json (o del inglés si no hay fichero)
$utf8 = New-Object System.Text.UTF8Encoding $false
foreach ($code in 'en', 'es', 'fr', 'de', 'ja', 'ko', 'zh_CN') {
    $src = Join-Path 'locales' "$code.json"
    if (-not (Test-Path $src)) { $src = Join-Path 'locales' 'en.json' }
    $text = [System.IO.File]::ReadAllText((Resolve-Path $src), $utf8)
    $json = ($text | ConvertFrom-Json).opendeck | ConvertTo-Json -Depth 10
    [System.IO.File]::WriteAllText((Join-Path $out "$code.json"), $json, $utf8)
}
Copy-Item target\release\prisma.exe -Destination $out

Compress-Archive -Path $out -DestinationPath (Join-Path $dist "$name.zip") -Force
Write-Host "Listo: $dist\$name.zip"
