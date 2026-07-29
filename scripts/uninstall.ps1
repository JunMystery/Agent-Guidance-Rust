[CmdletBinding()]
param()

$ErrorActionPreference = "SilentlyContinue"

Write-Host ""
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Red
Write-Host "|           Agent Guidance Rust (Windows)                       |" -ForegroundColor Red
Write-Host "|                   Uninstaller                                 |" -ForegroundColor Red
Write-Host "+--------------------------------------------------------------+" -ForegroundColor Red
Write-Host ""

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
