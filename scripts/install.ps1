# Self-contained PowerShell Installer/Uninstaller for Agent Guidance Rust on Windows
[CmdletBinding()]
param()

$ErrorActionPreference = "SilentlyContinue"

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
    
    cmd /c "taskkill /F /IM agent-guidance* >nul 2>&1"
    
    if (Get-Command "uv" -ErrorAction SilentlyContinue) {
        cmd /c "uv tool uninstall agent-guidance-mcp >nul 2>&1"
    }
    
    if (Test-Path "$HOME\.agent-guidance") {
        Remove-Item -Recurse -Force "$HOME\.agent-guidance" -ErrorAction SilentlyContinue
        Write-Host "  ✓ Completely removed directory $HOME\.agent-guidance" -ForegroundColor Green
    }
    
    if (Test-Path "$HOME\.local\bin\agent-guidance.exe") {
        Remove-Item -Force "$HOME\.local\bin\agent-guidance.exe" -ErrorAction SilentlyContinue
    }
    
    Write-Host ""
    Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host "|       ✓  Complete uninstallation finished!                  |" -ForegroundColor Green
    Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host ""
    exit 0
}

Write-Host ""
Write-Host "⚡ Enforcing exclusive edition: Removing Python runtime & old installs..." -ForegroundColor Yellow

# Terminate existing server processes
cmd /c "taskkill /F /IM agent-guidance* >nul 2>&1"

# Silently remove Python MCP edition if present via uv (bypass PowerShell NativeCommandError interception)
if (Get-Command "uv" -ErrorAction SilentlyContinue) {
    cmd /c "uv tool uninstall agent-guidance-mcp >nul 2>&1"
}

# Ensure Cargo / Rust toolchain is available
if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue) -and -not (Test-Path "$HOME\.cargo\bin\cargo.exe")) {
    Write-Host "⚡ Rust toolchain not found. Installing rustup..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
    Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait
    $env:Path += ";$HOME\.cargo\bin"
} else {
    $env:Path += ";$HOME\.cargo\bin"
}

# ── Determine project source root (current directory, script directory, or git clone) ──
$buildDir = ""
$scriptParent = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { "" }
$tmpClone = ""

if (Test-Path "Cargo.toml") {
    $buildDir = (Get-Location).Path
} elseif ($scriptParent -and (Test-Path (Join-Path $scriptParent "Cargo.toml"))) {
    $buildDir = (Get-Item $scriptParent).FullName
} else {
    Write-Host ""
    Write-Host "📥 Standalone execution detected. Cloning repository..." -ForegroundColor Cyan
    $tmpClone = Join-Path $env:TEMP ([Guid]::NewGuid().ToString())
    & git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git $tmpClone
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path (Join-Path $tmpClone "Cargo.toml"))) {
        Write-Host "❌ Failed to clone repository. Please check git or network connection." -ForegroundColor Red
        exit 1
    }
    $buildDir = $tmpClone
}

Write-Host ""
Write-Host "🔨 Building release binary from source..." -ForegroundColor Cyan
Push-Location $buildDir
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Cargo build failed with exit code $LASTEXITCODE." -ForegroundColor Red
        Pop-Location
        exit 1
    }
} finally {
    Pop-Location
}

$localBin = "$HOME\.local\bin"
if (-not (Test-Path $localBin)) { 
    New-Item -ItemType Directory -Path $localBin -Force | Out-Null 
}

$builtBinary = Join-Path $buildDir "target\release\agent-guidance.exe"
if (-not (Test-Path $builtBinary)) {
    Write-Host "❌ Built binary not found at $builtBinary" -ForegroundColor Red
    exit 1
}

Copy-Item $builtBinary "$localBin\agent-guidance.exe" -Force

Write-Host ""
Write-Host ">> Registering Agent Guidance Rust server with detected IDE clients..." -ForegroundColor Magenta
& "$localBin\agent-guidance.exe" --setup

if ($tmpClone -and (Test-Path $tmpClone)) {
    Remove-Item -Recurse -Force $tmpClone -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|         ✓  Rust Edition Installed Successfully!              |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host ""
Write-Host "  Installed Executable Binary:"
Write-Host "    $HOME\.local\bin\agent-guidance.exe" -ForegroundColor Cyan
Write-Host ""
