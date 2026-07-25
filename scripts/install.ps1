# Self-contained PowerShell Installer for Agent Guidance Rust on Windows
$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Magenta
Write-Host "|           Agent Guidance Rust (Windows)                      |" -ForegroundColor Magenta
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Magenta
Write-Host ""

if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue) -and -not (Test-Path "$HOME\.cargo\bin\cargo.exe")) {
    Write-Host "⚡ Rust toolchain not found. Installing rustup..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
    Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait
    $env:Path += ";$HOME\.cargo\bin"
} else {
    $env:Path += ";$HOME\.cargo\bin"
}

Write-Host "🔨 Building release binary..." -ForegroundColor Cyan
cargo build --release

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|         v  Build completed successfully!                     |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host ""
Write-Host "  Executable Binary Path:"
Write-Host "    $((Get-Item .).FullName)\target\release\agent-guidance.exe" -ForegroundColor Cyan
Write-Host ""
