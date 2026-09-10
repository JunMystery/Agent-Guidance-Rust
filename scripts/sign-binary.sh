#!/usr/bin/env bash
# ==============================================================================
# Agent Guidance Unified Unix Signing & Fingerprint Script (macOS / Linux)
# Publisher: Jun Mystery <darkzeuslk@gmail.com>
# ==============================================================================

set -euo pipefail

TARGET="${1:-target/release/agent-guidance}"

if [ ! -f "$TARGET" ]; then
    if [ -f "target/debug/agent-guidance" ]; then
        TARGET="target/debug/agent-guidance"
    else
        echo "Error: Target binary not found: $TARGET. Run 'cargo build --release' first." >&2
        exit 1
    fi
fi

OS_NAME="$(uname -s)"
echo "=== Agent Guidance Unix Signing & Fingerprint ==="
echo "Target:    $TARGET"
echo "Platform:  $OS_NAME ($(uname -m))"
echo "Publisher: Jun Mystery <darkzeuslk@gmail.com>"

# 1. Platform-Specific Signing
if [ "$OS_NAME" = "Darwin" ]; then
    echo "Applying macOS codesign..."
    IDENTITY="${APPLE_SIGNING_IDENTITY:--}"
    
    if [ "$IDENTITY" = "-" ]; then
        echo "Note: APPLE_SIGNING_IDENTITY not set, applying hardened ad-hoc signature..."
        codesign --force --options runtime --identifier "com.junmystery.agent-guidance" -s - "$TARGET"
    else
        echo "Signing with Apple Developer Identity: $IDENTITY"
        codesign --force --options runtime --timestamp --identifier "com.junmystery.agent-guidance" -s "$IDENTITY" "$TARGET"
    fi

    echo "Verifying codesign signature..."
    codesign -dvv "$TARGET" 2>&1 | grep -E "(Authority|TeamIdentifier|Identifier|Sealed Resources)" || true

    if command -v spctl >/dev/null 2>&1; then
        echo "Gatekeeper assessment:"
        spctl -a -v "$TARGET" 2>&1 || echo "Gatekeeper: ad-hoc or self-signed binary"
    fi
elif [ "$OS_NAME" = "Linux" ]; then
    echo "Linux ELF binary integrity check..."
    if command -v readelf >/dev/null 2>&1; then
        BUILD_ID=$(readelf -n "$TARGET" 2>/dev/null | grep "Build ID:" | awk '{print $3}' || echo "N/A")
        echo "GNU Build-ID: $BUILD_ID"
    fi

    if [ -n "${GPG_PRIVATE_KEY:-}" ]; then
        echo "Importing GPG private key from CI secret..."
        echo "$GPG_PRIVATE_KEY" | gpg --batch --import || true
    fi

    if command -v gpg >/dev/null 2>&1; then
        echo "Generating GPG detached signature..."
        GPG_KEY="${GPG_KEY_ID:-darkzeuslk@gmail.com}"
        if gpg --list-secret-keys "$GPG_KEY" >/dev/null 2>&1; then
            gpg --batch --yes --detach-sign --armor --default-key "$GPG_KEY" -o "$TARGET.sig" "$TARGET"
            echo "GPG signature generated: $TARGET.sig"
        else
            echo "Note: GPG key for '$GPG_KEY' not found; skipping GPG detached signature."
        fi
    fi
fi

# 2. Cryptographic Fingerprint Generation
echo "Computing cryptographic hashes..."
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$TARGET" | tee "$TARGET.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$TARGET" | tee "$TARGET.sha256"
fi

if command -v sha512sum >/dev/null 2>&1; then
    sha512sum "$TARGET" > "$TARGET.sha512"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 512 "$TARGET" > "$TARGET.sha512"
fi

echo "=== Verification complete ==="
