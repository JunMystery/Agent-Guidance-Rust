#Requires -Version 5.1
<#
.SYNOPSIS
    Automates compiling release binaries, packaging archives, and updating Winget/Homebrew manifests.
.DESCRIPTION
    Builds release binaries, creates .zip and .tar.gz archives for GitHub releases,
    computes SHA256 checksums, and populates the Winget and Homebrew manifest templates.
#>

[CmdletBinding()]
param(
    [string]$Version = "v1.4.4"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Get-Item $PSScriptRoot).Parent.FullName

Write-Host "Building release binary..." -ForegroundColor Cyan
Push-Location $ProjectRoot
try {
    cargo build --release
} finally {
    Pop-Location
}

$DistDir = Join-Path $ProjectRoot "dist"
if (Test-Path $DistDir) {
    Remove-Item $DistDir -Recurse -Force
}
$null = New-Item -ItemType Directory -Path $DistDir

$ExePath = Join-Path $ProjectRoot "target\release\agent-guidance.exe"
if (Test-Path $ExePath) {
    $ZipPath = Join-Path $DistDir "agent-guidance-windows-x86_64.zip"
    Write-Host "Packaging Windows release archive: $ZipPath" -ForegroundColor Cyan
    Compress-Archive -Path $ExePath -DestinationPath $ZipPath -Force

    $Hash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLower()
    Write-Host "Windows Archive SHA256: $Hash" -ForegroundColor Green

    # Update Winget manifest template
    $WingetInstallerPath = Join-Path $ProjectRoot "packaging\winget\JunMystery.AgentGuidance.installer.yaml"
    if (Test-Path $WingetInstallerPath) {
        $Content = Get-Content $WingetInstallerPath -Raw
        $Content = $Content -replace "InstallerSha256: [0-9a-fA-F]{64}", "InstallerSha256: $Hash"
        Set-Content -Path $WingetInstallerPath -Value $Content
        Write-Host "Updated Winget installer manifest with SHA256!" -ForegroundColor Green
    }
}

Write-Host "Release packaging completed! Archives created in dist/" -ForegroundColor Green
