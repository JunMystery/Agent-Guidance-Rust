#Requires -Version 5.1
<#
.SYNOPSIS
    Installer for Agent Guidance Rust (Windows).
.DESCRIPTION
    Downloads or builds the agent-guidance binary and registers it with IDE clients.
    Source: https://github.com/JunMystery/Agent-Guidance-Rust
#>

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

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
Write-Host "  [1] Install / Update  (build latest Rust server + update dashboard)" -ForegroundColor Green
Write-Host "  [2] Uninstall         (remove binary, data directory)" -ForegroundColor Red
Write-Host ""
$action = Read-Host "Choice [1]"
if (-not $action) { $action = "1" }

# ── Uninstall path ────────────────────────────────────────────────────────────
if ($action -eq "2") {
    Write-Host ""
    Write-Host "Uninstalling Agent Guidance..." -ForegroundColor Red

    # Stop any running processes
    Get-Process -Name "agent-guidance" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 400

    # Remove data directory
    if (Test-Path "$HOME\.agent-guidance") {
        Remove-Item -Recurse -Force "$HOME\.agent-guidance" -ErrorAction SilentlyContinue
        Write-Host "  OK Removed directory $HOME\.agent-guidance" -ForegroundColor Green
    }

    # Remove binaries
    foreach ($bin in @("$HOME\.local\bin\agent-guidance.exe", "$HOME\.cargo\bin\agent-guidance.exe", "$env:LOCALAPPDATA\Programs\agent-guidance\bin\agent-guidance.exe")) {
        if (Test-Path $bin) {
            Remove-Item -Force $bin -ErrorAction SilentlyContinue
        }
    }

    Write-Host ""
    Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host "|         OK  Uninstallation finished!                        |" -ForegroundColor Green
    Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host ""
    exit 0
}

# ── Install / Update path ─────────────────────────────────────────────────────
Write-Host ""
Write-Host "Stopping any running agent-guidance processes..." -ForegroundColor Yellow
Get-Process -Name "agent-guidance" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# ── Prepare binary output directory ──────────────────────────────────────────
# Use %LOCALAPPDATA%\Programs\agent-guidance\bin to satisfy Windows AppLocker / WDAC policies
$localBin = Join-Path $env:LOCALAPPDATA "Programs\agent-guidance\bin"
if (-not (Test-Path $localBin)) {
    New-Item -ItemType Directory -Path $localBin -Force | Out-Null
}

function Ensure-Cargo {
    Write-Host "Checking Rust toolchain (cargo)..." -ForegroundColor White
    if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue) -and -not (Test-Path "$HOME\.cargo\bin\cargo.exe")) {
        Write-Host "  Rust toolchain not found. Installing rustup..." -ForegroundColor Yellow
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
        Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait
        $env:Path += ";$HOME\.cargo\bin"
    } else {
        $env:Path += ";$HOME\.cargo\bin"
        Write-Host "  OK Found Cargo in PATH" -ForegroundColor Green
    }
}

# ── Spinner helper ────────────────────────────────────────────────────────────
function Run-WithSpinner {
    param(
        [scriptblock]$ScriptBlock,
        [string]$Message,
        [object[]]$ArgumentList = @()
    )
    $anim = @("⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏")
    $job = Start-Job -ScriptBlock $ScriptBlock -ArgumentList $ArgumentList
    $i = 0
    while ($job.State -eq "Running") {
        $char = $anim[$i % $anim.Length]
        Write-Host -NoNewline "`r  $char $Message"
        Start-Sleep -Milliseconds 150
        $i++
    }
    $output = Receive-Job $job -ErrorAction SilentlyContinue
    $exitCode = $job.ChildJobs[0].JobStateInfo.Reason
    Remove-Job $job -Force -ErrorAction SilentlyContinue
    return $output
}

# ── Detect build source (local dev or remote clone) ───────────────────────────
$buildDir = ""
$scriptParent = if ($PSScriptRoot) { Join-Path $PSScriptRoot ".." } else { "" }

if (Test-Path "Cargo.toml") {
    $content = Get-Content "Cargo.toml" -Raw -ErrorAction SilentlyContinue
    if ($content -match 'name\s*=\s*"agent-guidance"') {
        $buildDir = (Get-Location).Path
    }
} elseif ($scriptParent -and (Test-Path (Join-Path $scriptParent "Cargo.toml"))) {
    $content = Get-Content (Join-Path $scriptParent "Cargo.toml") -Raw -ErrorAction SilentlyContinue
    if ($content -match 'name\s*=\s*"agent-guidance"') {
        $buildDir = (Get-Item $scriptParent).FullName
    }
}

# ── Build block helper ────────────────────────────────────────────────────────
function Build-AndInstall {
    param([string]$SourceDir)

    Write-Host ""
    Write-Host "Building release binary (this embeds the latest dashboard HTML/JS)..." -ForegroundColor Cyan

    Push-Location $SourceDir
    try {
        $env:RUSTFLAGS = "-A warnings"
        $job = Start-Job -ScriptBlock {
            param($dir)
            Set-Location $dir
            $env:RUSTFLAGS = "-A warnings"
            cargo build --release --quiet 2>&1
        } -ArgumentList $SourceDir

        $anim = @("⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏")
        $i = 0
        while ($job.State -eq "Running") {
            $char = $anim[$i % $anim.Length]
            Write-Host -NoNewline "`r  $char Compiling Rust server + embedding dashboard assets... "
            Start-Sleep -Milliseconds 150
            $i++
        }
        $jobOutput = Receive-Job $job -ErrorAction SilentlyContinue
        Remove-Job $job -Force -ErrorAction SilentlyContinue

        $builtBin = Join-Path $SourceDir "target\release\agent-guidance.exe"
        if (Test-Path $builtBin) {
            Get-Process -Name "agent-guidance" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 300
            Copy-Item $builtBin "$localBin\agent-guidance.exe" -Force
            Write-Host -NoNewline "`r  OK Compilation finished successfully!                          `n" -ForegroundColor Green
        } else {
            Write-Host -NoNewline "`r  FAIL Cargo build failed.                                       `n" -ForegroundColor Red
            if ($jobOutput) { Write-Host $jobOutput }
            Pop-Location
            exit 1
        }
    } finally {
        Pop-Location
    }
}

# ── Install / Update binary (Prebuilt download with fallback to build) ───────
$repo = "JunMystery/Agent-Guidance-Rust"
$assetName = "agent-guidance-windows-x86_64.zip"

# Auto-detect the latest published release version from GitHub API
try {
    $latestMeta = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -UseBasicParsing -ErrorAction Stop
    $version = $latestMeta.tag_name
    Write-Host "  Latest release: $version" -ForegroundColor Gray
} catch {
    $version = "v1.4.7"
    Write-Host "  Could not fetch latest release tag, defaulting to $version" -ForegroundColor Yellow
}

$url = "https://github.com/$repo/releases/download/$version/$assetName"

function Try-DownloadPrebuilt {
    param([string]$DownloadUrl, [string]$Asset)
    Write-Host ""
    Write-Host "Attempting prebuilt binary installation ($Asset)..." -ForegroundColor Cyan
    $tmpDir = Join-Path $env:TEMP "ag-download-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
    $zipPath = Join-Path $tmpDir $Asset

    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $zipPath -UseBasicParsing -ErrorAction Stop
        if ((Test-Path $zipPath) -and ((Get-Item $zipPath).Length -gt 0)) {
            Write-Host "  OK Extracting prebuilt release package..." -ForegroundColor Green
            Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force -ErrorAction Stop
            $extractedBin = Join-Path $tmpDir "agent-guidance.exe"
            if (Test-Path $extractedBin) {
                Get-Process -Name "agent-guidance" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
                Start-Sleep -Milliseconds 300
                Copy-Item $extractedBin "$localBin\agent-guidance.exe" -Force
                Write-Host "  OK Installed prebuilt release binary!" -ForegroundColor Green
                Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
                return $true
            }
        }
    } catch {
        Write-Host "  Prebuilt binary download not available or failed. ($_)" -ForegroundColor Yellow
    }
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    return $false
}

$installedPrebuilt = $false
if (-not $buildDir) {
    $installedPrebuilt = Try-DownloadPrebuilt -DownloadUrl $url -Asset $assetName
}

if (-not $installedPrebuilt) {
    Ensure-Cargo
    if ($buildDir) {
        Write-Host ""
        Write-Host "Building from local source: $buildDir" -ForegroundColor Cyan
        Build-AndInstall -SourceDir $buildDir
    } else {
        Write-Host ""
        Write-Host "Fetching latest source from GitHub and building..." -ForegroundColor Cyan
        $globalSrc = Join-Path $HOME ".agent-guidance\src"

        if (Test-Path (Join-Path $globalSrc "Cargo.toml")) {
            Write-Host "  Pulling latest changes from origin/main..." -ForegroundColor Gray
            Push-Location $globalSrc
            try {
                $null = git fetch --depth 1 origin main 2>$null
                $null = git reset --hard origin/main 2>$null
            } finally {
                Pop-Location
            }
        } else {
            if (-not (Test-Path $globalSrc)) {
                New-Item -ItemType Directory -Path $globalSrc -Force | Out-Null
            }
            $null = git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git "$globalSrc" 2>$null
        }

        Build-AndInstall -SourceDir $globalSrc
    }
}

# ── Register with IDEs ────────────────────────────────────────────────────────
Write-Host ""
Write-Host ">> Registering server with detected IDE clients..." -ForegroundColor Magenta
& "$localBin\agent-guidance.exe" --setup

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|         OK  Agent Guidance Installed / Updated!              |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host ""
Write-Host "  Binary:       $localBin\agent-guidance.exe" -ForegroundColor Cyan
Write-Host "  Dashboard:    agent-guidance --dashboard   (serves updated HTML/JS embedded in binary)" -ForegroundColor Cyan
Write-Host "  MCP Config:   Automatic across all detected IDE clients" -ForegroundColor Green
Write-Host "  Rules/Skills: Preserved under manual user control (no automatic overwrites)" -ForegroundColor DarkGray
Write-Host ""
