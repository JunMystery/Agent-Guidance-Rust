<#
.SYNOPSIS
    Signs the agent-guidance Windows executable with Authenticode SHA-256 and verifies fingerprint.
.PARAMETER Target
    Path to agent-guidance.exe (default: target/release/agent-guidance.exe)
.PARAMETER CertificatePath
    Optional path to custom PFX certificate file.
.PARAMETER TimestampServer
    RFC-3161 Timestamp Server (default: http://timestamp.digicert.com)
#>
param (
    [string]$Target = "target/release/agent-guidance.exe",
    [string]$CertificatePath = "",
    [string]$CertificateBase64 = "",
    [string]$CertificatePassword = "",
    [string]$TimestampServer = "http://timestamp.digicert.com",
    [switch]$EnrollRoot = $false
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Target)) {
    if (Test-Path "target/debug/agent-guidance.exe") {
        $Target = "target/debug/agent-guidance.exe"
    } else {
        Write-Error "Target executable not found: $Target. Run 'cargo build --release' first."
        exit 1
    }
}

$fullPath = (Resolve-Path $Target).Path
Write-Host "=== Agent Guidance Authenticode Signing & Fingerprint ===" -ForegroundColor Cyan
Write-Host "Target: $fullPath"

$cert = $null
$b64 = if ($CertificateBase64) { $CertificateBase64 } else { $env:WINDOWS_CERT_BASE64 }
$pass = if ($CertificatePassword) { $CertificatePassword } else { $env:WINDOWS_CERT_PASSWORD }

if ($b64) {
    Write-Host "Loading certificate from base64 secret..." -ForegroundColor Cyan
    $pfxBytes = [Convert]::FromBase64String($b64)
    $tmpPfx = Join-Path ([System.IO.Path]::GetTempPath()) "signing-$([System.Guid]::NewGuid().ToString('N')).pfx"
    [System.IO.File]::WriteAllBytes($tmpPfx, $pfxBytes)
    try {
        if ($pass) {
            $secPass = ConvertTo-SecureString -String $pass -AsPlainText -Force
            $cert = Get-PfxCertificate -FilePath $tmpPfx -Password $secPass
        } else {
            $cert = Get-PfxCertificate -FilePath $tmpPfx
        }
    } finally {
        Remove-Item -Path $tmpPfx -Force -ErrorAction SilentlyContinue
    }
} elseif ($CertificatePath -and (Test-Path $CertificatePath)) {
    Write-Host "Loading certificate from file: $CertificatePath"
    $cert = Get-PfxCertificate -FilePath $CertificatePath
} else {
    # Check CurrentUser\My store for Jun Mystery Code Signing certificate
    $certs = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -like "*CN=Jun Mystery*" }
    if ($certs) {
        $cert = $certs[0]
        Write-Host "Using existing certificate: $($cert.Thumbprint)"
    } else {
        Write-Host "Creating trusted self-signed Code Signing certificate for Jun Mystery..." -ForegroundColor Yellow
        $cert = New-SelfSignedCertificate `
            -Type CodeSigningCert `
            -Subject "CN=Jun Mystery, O=Jun Mystery, OU=Agent Guidance, E=darkzeuslk@gmail.com" `
            -CertStoreLocation "Cert:\CurrentUser\My" `
            -KeyUsage DigitalSignature `
            -KeyLength 2048 `
            -NotAfter (Get-Date).AddYears(5) `
            -FriendlyName "Jun Mystery Agent Guidance Code Signing"

        # Enroll public key into TrustedPublisher for machine trust
        $pubStore = New-Object System.Security.Cryptography.X509Certificates.X509Store("TrustedPublisher", "CurrentUser")
        $pubStore.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
        $pubStore.Add($cert)
        $pubStore.Close()

        Write-Host "Enrolled certificate into CurrentUser\TrustedPublisher store." -ForegroundColor Green
    }

    if ($EnrollRoot) {
        try {
            $rootStore = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "CurrentUser")
            $rootStore.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
            $rootStore.Add($cert)
            $rootStore.Close()
            Write-Host "Enrolled certificate into CurrentUser\Root store." -ForegroundColor Green
        } catch {
            Write-Warning "Could not enroll into Root store: $_"
        }
    }
}

Write-Host "Signing executable with SHA-256 and timestamping..."
try {
    $sig = Set-AuthenticodeSignature -FilePath $fullPath -Certificate $cert -TimestampServer $TimestampServer -HashAlgorithm SHA256
} catch {
    Write-Warning "Timestamp server unreachable; signing without timestamp..."
    $sig = Set-AuthenticodeSignature -FilePath $fullPath -Certificate $cert -HashAlgorithm SHA256
}

$verify = Get-AuthenticodeSignature -FilePath $fullPath
$hashSha256 = (Get-FileHash -Path $fullPath -Algorithm SHA256).Hash
$hashSha512 = (Get-FileHash -Path $fullPath -Algorithm SHA512).Hash

$statusColor = if ($verify.Status -eq "Valid") { "Green" } else { "Yellow" }
Write-Host "`n[VERIFICATION REPORT]" -ForegroundColor Green
Write-Host "Publisher:    Jun Mystery <darkzeuslk@gmail.com>"
Write-Host "Signature:    $($verify.Status)" -ForegroundColor $statusColor
Write-Host "Status Msg:   $($verify.StatusMessage)"
Write-Host "Thumbprint:   $($cert.Thumbprint)"
Write-Host "SHA-256 Hash: $hashSha256"
Write-Host "SHA-512 Hash: $hashSha512"
Write-Host "=== Signing complete ===" -ForegroundColor Cyan
