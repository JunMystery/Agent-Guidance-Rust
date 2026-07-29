# Self-contained PowerShell Installer/Uninstaller for Agent Guidance Rust on Windows
[CmdletBinding()]
param()

$ErrorActionPreference = "SilentlyContinue"

# Protection against CWD falling into System32 when invoked via CMD
if ((Get-Location).Path -like "*\system32*") {
    Set-Location $HOME
}

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

# Silently remove Python MCP edition if present via uv
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

# ── Determine project source root or download prebuilt binary ──
$localBin = "$HOME\.local\bin"
if (-not (Test-Path $localBin)) { 
    New-Item -ItemType Directory -Path $localBin -Force | Out-Null 
}

$buildDir = ""
$scriptParent = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { "" }

if (Test-Path "Cargo.toml") {
    $buildDir = (Get-Location).Path
} elseif ($scriptParent -and (Test-Path (Join-Path $scriptParent "Cargo.toml"))) {
    $buildDir = (Get-Item $scriptParent).FullName
}

if ($buildDir) {
    Write-Host ""
    Write-Host "⚙️  Building release binary from local source..." -ForegroundColor Cyan
    Push-Location $buildDir
    try {
        $env:RUSTFLAGS = "-A warnings"
        $job = Start-Job -ScriptBlock {
            param($dir)
            Set-Location $dir
            $env:RUSTFLAGS = "-A warnings"
            cargo build --release --quiet 2>&1
        } -ArgumentList $buildDir

        $anim = @("⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏")
        $i = 0
        while ($job.State -eq "Running") {
            $char = $anim[$i % $anim.Length]
            Write-Host -NoNewline "`r  $char Compiling dependencies & Rust server... "
            Start-Sleep -Milliseconds 150
            $i++
        }
        $jobResult = Receive-Job $job
        Remove-Job $job -Force

        if (Test-Path "target\release\agent-guidance.exe") {
            Get-Process -Name "agent-guidance*" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 300
            Copy-Item "target\release\agent-guidance.exe" "$localBin\agent-guidance.exe" -Force
            Write-Host -NoNewline "`r  ✓ Compilation & build finished successfully!            `n" -ForegroundColor Green
        } else {
            Write-Host -NoNewline "`r❌ Cargo build failed.                                    `n" -ForegroundColor Red
            Pop-Location
            exit 1
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host ""
    Write-Host "📦 Fetching source repository & building Rust server..." -ForegroundColor Cyan
    $globalSrc = Join-Path $HOME ".agent-guidance\src"
    if (Test-Path (Join-Path $globalSrc "Cargo.toml")) {
        Push-Location $globalSrc
        try {
            cmd /c "git fetch --depth 1 origin main >nul 2>&1"
            cmd /c "git reset --hard origin/main >nul 2>&1"
        } finally {
            Pop-Location
        }
    } else {
        if (-not (Test-Path $globalSrc)) { New-Item -ItemType Directory -Path $globalSrc -Force | Out-Null }
        cmd /c "git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git `"$globalSrc`" >nul 2>&1"
    }

    Push-Location $globalSrc
    try {
        $env:RUSTFLAGS = "-A warnings"
        $job = Start-Job -ScriptBlock {
            param($dir)
            Set-Location $dir
            $env:RUSTFLAGS = "-A warnings"
            cargo build --release --quiet 2>&1
        } -ArgumentList $globalSrc

        $anim = @("⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏")
        $i = 0
        while ($job.State -eq "Running") {
            $char = $anim[$i % $anim.Length]
            Write-Host -NoNewline "`r  $char Compiling dependencies & Rust server... "
            Start-Sleep -Milliseconds 150
            $i++
        }
        $jobResult = Receive-Job $job
        Remove-Job $job -Force

        if (Test-Path "target\release\agent-guidance.exe") {
            Get-Process -Name "agent-guidance*" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 300
            Copy-Item "target\release\agent-guidance.exe" "$localBin\agent-guidance.exe" -Force
            Write-Host -NoNewline "`r  ✓ Compilation & build finished successfully!            `n" -ForegroundColor Green
        } else {
            Write-Host -NoNewline "`r❌ Cargo build failed.                                    `n" -ForegroundColor Red
            Pop-Location
            exit 1
        }
    } finally {
        Pop-Location
    }
}

Write-Host ""
Write-Host ">> Registering Agent Guidance Rust server with detected IDE clients..." -ForegroundColor Magenta
& "$localBin\agent-guidance.exe" --setup

Write-Host ""
Write-Host ">> Precomputing skill passage cache for instant first startup..." -ForegroundColor Magenta
& "$localBin\agent-guidance.exe" --generate-passage-cache

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|         ✓  Rust Edition Installed Successfully!              |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host ""
Write-Host "  Installed Executable Binary:"
Write-Host "    $HOME\.local\bin\agent-guidance.exe" -ForegroundColor Cyan
Write-Host ""
