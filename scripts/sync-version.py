#!/usr/bin/env python3
"""
Cross-Platform Single-Source-of-Truth Version Synchronizer

Reads version from Cargo.toml and synchronizes all files across the repository:
- Cargo.lock (via cargo check)
- scripts/install.ps1
- scripts/install.sh
- scripts/package-release.ps1
- packaging/winget/JunMystery.AgentGuidance.locale.en-US.yaml
- packaging/winget/JunMystery.AgentGuidance.installer.yaml
- packaging/homebrew/agent-guidance.rb

Usage:
    python scripts/sync-version.py [OPTIONAL_NEW_VERSION]
    (e.g., python scripts/sync-version.py 1.3.0)
"""

import re
import sys
import subprocess
from pathlib import Path

def get_script_dir():
    return Path(__file__).resolve().parent

def get_project_root():
    return get_script_dir().parent

def read_cargo_version(cargo_path):
    content = cargo_path.read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if not match:
        raise ValueError(f"Could not parse version from {cargo_path}")
    return match.group(1)

def set_cargo_version(cargo_path, new_version):
    content = cargo_path.read_text(encoding="utf-8")
    updated = re.sub(r'^(version\s*=\s*")[^"]+(")', f'\\g<1>{new_version}\\g<2>', content, count=1, flags=re.MULTILINE)
    cargo_path.write_text(updated, encoding="utf-8")
    print(f"  [OK] Updated Cargo.toml to version {new_version}")

def replace_in_file(path, pattern, replacement):
    if not path.exists():
        print(f"  [WARN] File not found {path}")
        return
    content = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, content)
    if count > 0:
        path.write_text(updated, encoding="utf-8")
        print(f"  [OK] Updated {path.relative_to(get_project_root())} ({count} replacements)")
    else:
        print(f"  [INFO] No matches found in {path.relative_to(get_project_root())}")

def main():
    root = get_project_root()
    cargo_path = root / "Cargo.toml"

    if len(sys.argv) > 1:
        new_version = sys.argv[1].lstrip("v")
        set_cargo_version(cargo_path, new_version)

    version = read_cargo_version(cargo_path)
    v_version = f"v{version}"
    print(f"Synchronizing codebase version to {version} ({v_version})...")

    # 1. scripts/install.ps1
    replace_in_file(
        root / "scripts" / "install.ps1",
        r'\$version\s*=\s*"v[^"]+"',
        f'$version = "{v_version}"'
    )

    # 2. scripts/install.sh
    replace_in_file(
        root / "scripts" / "install.sh",
        r'VERSION="v[^"]+"',
        f'VERSION="{v_version}"'
    )

    # 4. packaging/winget/JunMystery.AgentGuidance.locale.en-US.yaml
    replace_in_file(
        root / "packaging" / "winget" / "JunMystery.AgentGuidance.locale.en-US.yaml",
        r'PackageVersion:\s*\S+',
        f'PackageVersion: {version}'
    )

    # 5. packaging/winget/JunMystery.AgentGuidance.installer.yaml
    replace_in_file(
        root / "packaging" / "winget" / "JunMystery.AgentGuidance.installer.yaml",
        r'PackageVersion:\s*\S+',
        f'PackageVersion: {version}'
    )
    replace_in_file(
        root / "packaging" / "winget" / "JunMystery.AgentGuidance.installer.yaml",
        r'releases/download/v[^/]+/',
        f'releases/download/{v_version}/'
    )

    # 6. packaging/homebrew/agent-guidance.rb
    replace_in_file(
        root / "packaging" / "homebrew" / "agent-guidance.rb",
        r'version\s+"[^"]+"',
        f'version "{version}"'
    )
    replace_in_file(
        root / "packaging" / "homebrew" / "agent-guidance.rb",
        r'releases/download/v[^/]+/',
        f'releases/download/{v_version}/'
    )

    # 7. README.md badge
    replace_in_file(
        root / "README.md",
        r'badge/Version-v[0-9\.]+-blue\.svg',
        f'badge/Version-{v_version}-blue.svg'
    )

    # 8. Cargo.lock (via cargo check)
    print("  Running cargo check to update Cargo.lock...")
    subprocess.run(["cargo", "check"], cwd=root, check=True)
    print("  [OK] Updated Cargo.lock")

    print(f"\nVersion synchronization to v{version} complete across all files!")

if __name__ == "__main__":
    main()
