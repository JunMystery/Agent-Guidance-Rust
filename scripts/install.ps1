# Self-contained PowerShell Installer/Uninstaller for Agent Guidance Rust on Windows
$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Magenta
Write-Host "|           Agent Guidance Rust (Windows)                      |" -ForegroundColor Magenta
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Magenta
Write-Host ""

Write-Host "What would you like to do?"
Write-Host "  [1] Install Rust Edition (removes Python edition & builds Rust server)" -ForegroundColor Green
Write-Host "  [2] Uninstall - remove entire Agent Guidance directory & toolchains" -ForegroundColor Red
Write-Host ""
$action = Read-Host "Choice [1]"
if (-not $action) { $action = "1" }

if ($action -eq "2") {
    Write-Host ""
    Write-Host "🗑️  Completely uninstalling Agent Guidance..." -ForegroundColor Red
    
    Get-Process -Name "agent-guidance*" -ErrorAction SilentlyContinue | Stop-Process -Force
    
    if (Get-Command "uv" -ErrorAction SilentlyContinue) {
        & uv tool uninstall agent-guidance-mcp 2>$null
    }
    
    if (Test-Path "$HOME\.agent-guidance") {
        Remove-Item -Recurse -Force "$HOME\.agent-guidance"
        Write-Host "  v Completely removed directory $HOME\.agent-guidance" -ForegroundColor Green
    }
    
    if (Test-Path "$HOME\.local\bin\agent-guidance.exe") {
        Remove-Item -Force "$HOME\.local\bin\agent-guidance.exe"
    }
    
    Write-Host ""
    Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host "|       v  Complete uninstallation finished!                  |" -ForegroundColor Green
    Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host ""
    exit 0
}

Write-Host ""
Write-Host "⚡ Enforcing exclusive edition: Removing Python runtime & old installs..." -ForegroundColor Yellow

Get-Process -Name "agent-guidance-mcp*" -ErrorAction SilentlyContinue | Stop-Process -Force

if (Get-Command "uv" -ErrorAction SilentlyContinue) {
    & uv tool uninstall agent-guidance-mcp 2>$null
}

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

$localBin = "$HOME\.local\bin"
if (-not (Test-Path $localBin)) { New-Item -ItemType Directory -Path $localBin | Out-Null }
Copy-Item "target\release\agent-guidance.exe" "$localBin\agent-guidance.exe" -Force

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|         v  Rust Edition Installed Successfully!              |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host ""
Write-Host "  Installed Executable Binary:"
Write-Host "    $HOME\.local\bin\agent-guidance.exe" -ForegroundColor Cyan
Write-Host ""
