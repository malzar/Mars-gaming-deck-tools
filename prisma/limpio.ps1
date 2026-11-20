# Instalación en limpio de Prisma: cierra OpenDeck, borra el plugin instalado
# (y el antiguo MGDC, si sigue ahí) y la compilación anterior, y vuelve a crear
# el zip para instalarlo desde OpenDeck.
#
#   powershell -ExecutionPolicy Bypass -File .\limpio.ps1

$ErrorActionPreference = 'Continue'
Set-Location $PSScriptRoot

# Dónde está OpenDeck, para abrirlo al final: se mira el que está abierto y
# se guarda para la próxima vez
$odFile = Join-Path $env:LOCALAPPDATA 'prisma\opendeck-path.txt'
$oldOdFile = Join-Path $env:LOCALAPPDATA 'mgdc\opendeck-path.txt'
$od = (Get-Process opendeck -ErrorAction SilentlyContinue | Select-Object -First 1).Path
if (-not $od -and (Test-Path $odFile)) { $od = Get-Content $odFile -First 1 }
if (-not $od -and (Test-Path $oldOdFile)) { $od = Get-Content $oldOdFile -First 1 }
if (-not $od) {
    $od = @("$env:LOCALAPPDATA\OpenDeck\opendeck.exe", "$env:ProgramFiles\OpenDeck\opendeck.exe", "${env:ProgramFiles(x86)}\OpenDeck\opendeck.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
}

Write-Host '== Cerrando OpenDeck y el plugin'
Stop-Process -Name opendeck, prisma, mgdc -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

Write-Host '== Borrando el plugin instalado'
Remove-Item -Recurse -Force "$env:APPDATA\opendeck\plugins\com.malzar.prisma.sdPlugin" -ErrorAction SilentlyContinue
# El plugin con el nombre de antes (MGDC): ya no se usa
Remove-Item -Recurse -Force "$env:APPDATA\opendeck\plugins\com.malzar.mgdc.sdPlugin" -ErrorAction SilentlyContinue

Write-Host '== Borrando el paquete y la compilación anteriores'
Remove-Item -Recurse -Force dist -ErrorAction SilentlyContinue
cargo clean --release -p prisma

Write-Host '== Compilando y creando el zip'
powershell -ExecutionPolicy Bypass -File .\package.ps1
if ($LASTEXITCODE -ne 0) { throw 'La compilación ha fallado' }

# Comprobar que el zip lleva todo lo necesario
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = Join-Path $PSScriptRoot 'dist\com.malzar.prisma.sdPlugin.zip'
$names = [System.IO.Compression.ZipFile]::OpenRead($zip).Entries | ForEach-Object { $_.FullName.Replace('\', '/') }
$missing = @('prisma.exe', 'manifest.json', 'assets/icon.png', 'assets/actions/playpause.svg', 'layouts/audio-dial.json', 'pi/audio.html') |
    Where-Object { $f = $_; -not ($names | Where-Object { $_ -like "*$f" }) }
if ($missing) {
    Write-Host "== FALTA en el zip: $($missing -join ', ')" -ForegroundColor Red
    exit 1
}

Write-Host "== Listo: $zip" -ForegroundColor Green

if ($od -and (Test-Path $od)) {
    New-Item -ItemType Directory -Force (Split-Path $odFile) | Out-Null
    Set-Content -Path $odFile -Value $od
    Write-Host '== Abriendo OpenDeck'
    Start-Process $od
} else {
    Write-Host '== No encuentro OpenDeck: ábrelo a mano (la próxima vez, con OpenDeck abierto, el script lo recordará)' -ForegroundColor Yellow
}
Write-Host '   En OpenDeck > Plugins > instalar desde fichero, elige ese zip.'
