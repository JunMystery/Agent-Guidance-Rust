#Requires -Version 5.1
<#
.SYNOPSIS
    Helper script to sign install.ps1 with a Code Signing certificate.
.DESCRIPTION
    Searches for an available Code Signing certificate in Cert:\CurrentUser\My or Cert:\LocalMachine\My
    and applies an Authenticode signature to scripts/install.ps1.
#>

[CmdletBinding()]
param()

$ScriptPath = Join-Path $PSScriptRoot "install.ps1"

if (-not (Test-Path $ScriptPath)) {
    Write-Error "Could not locate install.ps1 at $ScriptPath"
    exit 1
}

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $cert) {
    $cert = Get-ChildItem Cert:\LocalMachine\My -CodeSigningCert -ErrorAction SilentlyContinue | Select-Object -First 1
}

if ($cert) {
    Write-Host "Signing $ScriptPath using certificate $($cert.Subject)..." -ForegroundColor Cyan
    Set-AuthenticodeSignature -FilePath $ScriptPath -Certificate $cert -TimestampServer "http://timestamp.digicert.com"
    Write-Host "OK Successfully signed install.ps1!" -ForegroundColor Green
} else {
    Write-Host "Notice: No Code Signing Certificate found in Cert:\CurrentUser\My or Cert:\LocalMachine\My." -ForegroundColor Yellow
    Write-Host "To create a local self-signed code signing certificate for testing, run:" -ForegroundColor Gray
    Write-Host "  New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=AgentGuidanceDev' -CertStoreLocation 'Cert:\CurrentUser\My'" -ForegroundColor Gray
}
