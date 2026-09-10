# Installation

[Back to README](../README.md)

This project runs as a high-performance **100% Native Rust 2024 Edition** MCP server serving AI agent guidance over Stdio transport.

## Automatic Install

Use the one-line installer script:

**Windows (PowerShell / CMD):**
```powershell
powershell -Command "iwr https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 -OutFile $env:TEMP\i.ps1; & $env:TEMP\i.ps1"
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.sh | bash
```

The installer builds the native release binary (`agent-guidance`) and configures all detected IDE client configurations via `--setup`.

## Manual Build from Source

Build the binary directly using `cargo`:

```bash
git clone https://github.com/JunMystery/Agent-Guidance-Rust.git
cd Agent-Guidance-Rust
cargo build --release
```

The compiled executable binary will be created at `target/release/agent-guidance`.

## Run The Server

Register the built binary across all installed MCP IDE clients:

```bash
./target/release/agent-guidance --setup
```

Or start the native web usage dashboard server:

```bash
./target/release/agent-guidance --dashboard
```

## Embedded Skills & Local Workspace Extension

Agent Guidance compiles all 279 official skills and passage vectors directly into the native binary via `rust_embed`. No external corpus download is required.

To add custom skills for a specific project, create markdown skill capsules in:

```bash
<project_root>/.agents/skills/<skill-name>/SKILL.md
```

The server automatically indexes local workspace skills on startup alongside the embedded catalog.

## Related Docs

- [Client Setup](setup/client-configuration.md)
- [Usage Guide](usage.md)
- [Dashboard Guide](dashboard.md)
- [Development Guide](development.md)
