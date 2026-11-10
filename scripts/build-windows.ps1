# PowerShell Build Script for Antigravity Account Switcher on Windows
$ErrorActionPreference = "Stop"

Write-Host "========================================================" -ForegroundColor Cyan
Write-Host "  Building Antigravity Account Switcher for Windows" -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan

$RootDir = Split-Path -Parent $PSScriptRoot
Set-Location $RootDir

Write-Host "[1/3] Installing NPM dependencies..." -ForegroundColor Yellow
npm install

Write-Host "[2/3] Building Web Assets (Vite + TypeScript)..." -ForegroundColor Yellow
npm run build

Write-Host "[3/3] Compiling Tauri 2 Native Windows Executable..." -ForegroundColor Yellow
npx tauri build --no-bundle

$OutputDir = Join-Path $RootDir "bin\windows"
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$ExePath = Join-Path $RootDir "src-tauri\target\release\antigravity-account-switcher.exe"
if (Test-Path $ExePath) {
    Copy-Item $ExePath -Destination $OutputDir -Force
    Write-Host "========================================================" -ForegroundColor Green
    Write-Host "  Build Successful!" -ForegroundColor Green
    Write-Host "  Executable: bin\windows\antigravity-account-switcher.exe" -ForegroundColor Green
    Write-Host "========================================================" -ForegroundColor Green
} else {
    Write-Host "Error: Executable not found at $ExePath" -ForegroundColor Red
}
