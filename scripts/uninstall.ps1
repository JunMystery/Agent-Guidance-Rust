[CmdletBinding()]
param()

$ErrorActionPreference = "SilentlyContinue"

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Red
Write-Host "|           Agent Guidance Rust (Windows)                       |" -ForegroundColor Red
Write-Host "|                   Uninstaller                                 |" -ForegroundColor Red
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Red
Write-Host ""

try {
    schtasks /delete /tn "AgentGuidanceServer" /f 2>$null | Out-Null
    Write-Host "  OK Removed Windows Scheduled Task 'AgentGuidanceServer'" -ForegroundColor Green
} catch {}

cmd /c "taskkill /F /IM agent-guidance* >nul 2>&1"

if (Get-Command "uv" -ErrorAction SilentlyContinue) {
    cmd /c "uv tool uninstall agent-guidance-mcp >nul 2>&1"
}

if (Test-Path "$HOME\.agent-guidance") {
    Remove-Item -Recurse -Force "$HOME\.agent-guidance" -ErrorAction SilentlyContinue
    Write-Host "  OK Completely removed directory $HOME\.agent-guidance" -ForegroundColor Green
}

foreach ($bin in @("$HOME\.local\bin\agent-guidance.exe", "$HOME\.cargo\bin\agent-guidance.exe", "$env:LOCALAPPDATA\Programs\agent-guidance\bin\agent-guidance.exe")) {
    if (Test-Path $bin) {
        Remove-Item -Force $bin -ErrorAction SilentlyContinue
    }
}

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host "|       OK  Complete uninstallation finished!                  |" -ForegroundColor Green
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Green
Write-Host ""
