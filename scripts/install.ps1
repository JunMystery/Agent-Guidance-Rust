#Requires -Version 5.1
<#
.SYNOPSIS
    Installer for Agent Guidance Rust (Windows).
#>
[CmdletBinding()]
param(
    [Alias("Profile", "Mode")][string]$InstallProfile = "",
    [string]$ServerUrl = "http://127.0.0.1:11998",
    [string]$Bind = "0.0.0.0",
    [int]$WorkerPort = 11998,
    [int]$DashboardPort = 11997,
    [string]$ApiKey = "",
    [switch]$Uninstall,
    [switch]$NonInteractive
)

$ErrorActionPreference = "Stop"
if ((Get-Location).Path -like "*\system32*") { Set-Location $HOME }

Write-Host "`n+--------------------------------------------------------------+" -ForegroundColor Magenta
Write-Host "|           Agent Guidance Rust (Windows)                      |" -ForegroundColor Magenta
Write-Host "+--------------------------------------------------------------+`n" -ForegroundColor Magenta

function Perform-Uninstall {
    Write-Host "Uninstalling Agent Guidance..." -ForegroundColor Red
    try { schtasks /delete /tn "AgentGuidanceServer" /f 2>$null | Out-Null } catch {}
    Get-Process -Name "agent-guidance" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 400
    if (Test-Path "$HOME\.agent-guidance") {
        Remove-Item -Recurse -Force "$HOME\.agent-guidance" -ErrorAction SilentlyContinue
        Write-Host "  OK Removed directory $HOME\.agent-guidance" -ForegroundColor Green
    }
    foreach ($bin in @("$HOME\.local\bin\agent-guidance.exe", "$HOME\.cargo\bin\agent-guidance.exe", "$env:LOCALAPPDATA\Programs\agent-guidance\bin\agent-guidance.exe")) {
        if (Test-Path $bin) { Remove-Item -Force $bin -ErrorAction SilentlyContinue }
    }
    Write-Host "`n+--------------------------------------------------------------+" -ForegroundColor Green
    Write-Host "|         OK  Uninstallation finished!                         |" -ForegroundColor Green
    Write-Host "+--------------------------------------------------------------+`n" -ForegroundColor Green
    exit 0
}

if ($Uninstall) { Perform-Uninstall }

$action = "1"
if (-not $NonInteractive -and -not $InstallProfile) {
    Write-Host "What would you like to do?"
    Write-Host "  [1] Install / Update" -ForegroundColor Green
    Write-Host "  [2] Uninstall" -ForegroundColor Red
    $resp = Read-Host "Choice [1]"
    if ($resp -eq "2") { Perform-Uninstall }
}

if (-not $InstallProfile -and -not $NonInteractive) {
    Write-Host "`nSelect installation profile:" -ForegroundColor White
    Write-Host "  [1] Full Standalone       (Single binary with local Candle/ORT + SQLite FTS5)" -ForegroundColor Green
    Write-Host "  [2] Lightweight Client    (Zero-ML ~15 MB RAM, forwards queries to Remote ML Worker)" -ForegroundColor Cyan
    Write-Host "  [3] Dedicated Server Worker (Dedicated ML node, compiles binary registry, runs OS task)" -ForegroundColor Magenta
    $pResp = Read-Host "Profile [1]"
    $InstallProfile = if ($pResp) { $pResp } else { "1" }
} elseif (-not $InstallProfile) {
    $InstallProfile = "1"
}

Get-Process -Name "agent-guidance" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

$localBin = Join-Path $env:LOCALAPPDATA "Programs\agent-guidance\bin"
if (-not (Test-Path $localBin)) { New-Item -ItemType Directory -Path $localBin -Force | Out-Null }
$stagingDir = Join-Path $HOME ".agent-guidance\staging"
if (-not (Test-Path $stagingDir)) { New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null }

function Try-DownloadPrebuilt {
    param([string]$Prof)
    $repo = "JunMystery/Agent-Guidance-Rust"
    try {
        $meta = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -UseBasicParsing -ErrorAction Stop
        $version = $meta.tag_name
    } catch { $version = "v1.8.1" }

    $tag = switch ($Prof) { { $_ -in "Client", "2" } { "client" }; { $_ -in "Server", "3" } { "server" }; default { "standalone" } }
    $candidates = @("agent-guidance-$tag-windows-x86_64.zip")
    if ($tag -ne "standalone") { $candidates += "agent-guidance-standalone-windows-x86_64.zip" }
    $candidates += "agent-guidance-windows-x86_64.zip"

    $tmpDir = Join-Path $stagingDir "ag-dl-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    foreach ($asset in $candidates) {
        $url = "https://github.com/$repo/releases/download/$version/$asset"
        $zipPath = Join-Path $tmpDir $asset
        try {
            Write-Host "  Probing release asset: $asset..." -ForegroundColor Gray
            Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing -ErrorAction Stop
            if ((Test-Path $zipPath) -and ((Get-Item $zipPath).Length -gt 0)) {
                Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force -ErrorAction Stop
                $bin = Join-Path $tmpDir "agent-guidance.exe"
                if (Test-Path $bin) {
                    Copy-Item $bin "$localBin\agent-guidance.exe" -Force
                    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
                    Write-Host "  OK Downloaded and installed $asset ($version)" -ForegroundColor Green
                    return $true
                }
            }
        } catch {}
    }
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    return $false
}

function Build-FromSource {
    Write-Host "`nBuilding release binary from source..." -ForegroundColor Cyan
    $sDir = ""
    if (Test-Path "Cargo.toml") {
        if ((Get-Content "Cargo.toml" -Raw -ErrorAction SilentlyContinue) -match 'name\s*=\s*"agent-guidance"') { $sDir = (Get-Location).Path }
    }
    if (-not $sDir) {
        $sDir = Join-Path $HOME ".agent-guidance\src"
        if (Test-Path (Join-Path $sDir "Cargo.toml")) {
            Push-Location $sDir; try { git pull --depth 1 2>$null | Out-Null } finally { Pop-Location }
        } else {
            New-Item -ItemType Directory -Path $sDir -Force | Out-Null
            git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git "$sDir" 2>$null | Out-Null
        }
    }
    Push-Location $sDir
    try {
        $env:RUSTFLAGS = "-A warnings"
        cargo build --release --quiet
        Copy-Item (Join-Path $sDir "target\release\agent-guidance.exe") "$localBin\agent-guidance.exe" -Force
        Write-Host "  OK Build successful!" -ForegroundColor Green
    } finally { Pop-Location }
}

if (-not (Try-DownloadPrebuilt -Prof $InstallProfile)) {
    Write-Host "  Prebuilt download failed, compiling from source..." -ForegroundColor Yellow
    Build-FromSource
}

function Merge-JsonMCPConfig {
    param([string]$Path, [string]$Bin, [string]$Key = "mcpServers", [bool]$WithStdio = $false)
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path $parent) -and (Test-Path (Split-Path -Parent $parent))) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    if (Test-Path $parent) {
        try {
            $j = if (Test-Path $Path) { Get-Content $Path -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json -ErrorAction SilentlyContinue } else { $null }
            if (-not $j) { $j = [PSCustomObject]@{} }
            if (-not $j.PSObject.Properties[$Key]) { $j | Add-Member -NotePropertyName $Key -NotePropertyValue ([PSCustomObject]@{}) }
            $def = if ($WithStdio) { [PSCustomObject]@{ type = "stdio"; command = $Bin; args = @() } } else { [PSCustomObject]@{ command = $Bin; args = @() } }
            if ($j.$Key.PSObject.Properties["agent-guidance"]) { $j.$Key."agent-guidance" = $def } else { $j.$Key | Add-Member -NotePropertyName "agent-guidance" -NotePropertyValue $def }
            $j | ConvertTo-Json -Depth 10 | Set-Content $Path -Encoding UTF8
        } catch {}
    }
}

function Register-IDEMCP {
    param([string]$BinPath)
    Write-Host "`nRegistering server with detected IDE clients..." -ForegroundColor Magenta
    & $BinPath --setup
    $payload = "{\`"name\`":\`"agent-guidance\`",\`"type\`":\`"stdio\`",\`"command\`":\`"$($BinPath -replace '\\','\\')\`",\`"args\`":[]}"
    foreach ($cmd in @("code", "code-insiders")) { if (Get-Command $cmd -ErrorAction SilentlyContinue) { try { & $cmd --add-mcp $payload 2>$null | Out-Null } catch {} } }
    foreach ($cmd in @("claude", "claude.cmd")) { if (Get-Command $cmd -ErrorAction SilentlyContinue) { try { & $cmd mcp add --scope user agent-guidance -- $BinPath 2>$null | Out-Null; break } catch {} } }
    foreach ($cmd in @("codex", "codex.cmd")) { if (Get-Command $cmd -ErrorAction SilentlyContinue) { try { & $cmd mcp add agent-guidance -- $BinPath 2>$null | Out-Null; break } catch {} } }
    @( (Join-Path $env:APPDATA "Code\User\mcp.json"), (Join-Path $env:APPDATA "Code - Insiders\User\mcp.json") ) | ForEach-Object { Merge-JsonMCPConfig -Path $_ -Bin $BinPath -Key "servers" -WithStdio $true }
    @( (Join-Path $HOME ".copilot\mcp-config.json"), (Join-Path $HOME ".cursor\mcp.json"), (Join-Path $env:APPDATA "Cursor\User\mcp.json"), (Join-Path $env:APPDATA "Claude\claude_desktop_config.json") ) | ForEach-Object { Merge-JsonMCPConfig -Path $_ -Bin $BinPath -Key "mcpServers" }
    Merge-JsonMCPConfig -Path (Join-Path $HOME ".claude.json") -Bin $BinPath -Key "mcpServers" -WithStdio $true
}

$binPath = "$localBin\agent-guidance.exe"
switch ($InstallProfile) {
    { $_ -in "Client", "2" } {
        if (-not $NonInteractive) {
            $inputUrl = Read-Host "Remote Worker Server URL [$ServerUrl]"
            if ($inputUrl) { $ServerUrl = $inputUrl }
        }
        & $binPath --set-server $ServerUrl
        Register-IDEMCP -BinPath $binPath
        Write-Host "`nOK Lightweight Client configured! (Connected to $ServerUrl)" -ForegroundColor Green
        & $binPath --stats
    }
    { $_ -in "Server", "3" } {
        Write-Host "`nConfiguring Dedicated Server Worker..." -ForegroundColor Magenta
        $stgSkills = Join-Path $HOME ".agent-guidance\staging\skills"
        if (-not (Test-Path $stgSkills)) { New-Item -ItemType Directory -Path $stgSkills -Force | Out-Null }
        $tomb = Join-Path $HOME ".agent-guidance\tombstones.json"
        if (-not (Test-Path $tomb)) { Set-Content -Path $tomb -Value "[]" -Encoding UTF8 }

        if (-not $NonInteractive) {
            $inBind = Read-Host "Listen Address [$Bind]"; if ($inBind) { $Bind = $inBind }
            $inWPort = Read-Host "ML Worker Port [$WorkerPort]"; if ($inWPort) { $WorkerPort = [int]$inWPort }
            $inDPort = Read-Host "Dashboard Port [$DashboardPort]"; if ($inDPort) { $DashboardPort = [int]$inDPort }
            $inKey = Read-Host "Optional Bearer API Key [$ApiKey]"; if ($inKey) { $ApiKey = $inKey }
        }
        $keyArg = if ($ApiKey) { " --api-key $ApiKey" } else { "" }
        $taskCmd = "`"$binPath`" --server --bind $Bind --worker-port $WorkerPort --port $DashboardPort$keyArg"
        try {
            schtasks /create /tn "AgentGuidanceServer" /tr $taskCmd /sc onlogon /rl highest /f | Out-Null
            schtasks /run /tn "AgentGuidanceServer" 2>$null | Out-Null
            Write-Host "  OK Windows Task 'AgentGuidanceServer' created and started!" -ForegroundColor Green
        } catch { Write-Host "  Warning: Failed creating scheduled task: $_" -ForegroundColor Yellow }

        Write-Host "`nOK Dedicated Server Worker active!" -ForegroundColor Green
        Write-Host "  Worker API:    http://${Bind}:${WorkerPort}" -ForegroundColor Gray
        Write-Host "  Dashboard:     http://${Bind}:${DashboardPort}" -ForegroundColor Gray
        Write-Host "  Client Setup:  agent-guidance --set-server http://<SERVER_IP>:${WorkerPort}`n" -ForegroundColor Cyan
    }
    default {
        Register-IDEMCP -BinPath $binPath
        Write-Host "`nOK Agent Guidance Standalone Installed!" -ForegroundColor Green
    }
}

if ($env:Path -split ';' -notcontains $localBin) { $env:Path = "$localBin;$env:Path" }
try {
    $uPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
    $uList = if ($uPath) { $uPath -split ';' | Where-Object { $_ } } else { @() }
    if ($uList -notcontains $localBin) {
        [Environment]::SetEnvironmentVariable("Path", $(if ($uPath) { "$localBin;$uPath" } else { $localBin }), [EnvironmentVariableTarget]::User)
    }
} catch {}

Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|         Agent Guidance Setup Complete!                       |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "  Binary: $binPath`n" -ForegroundColor Green
