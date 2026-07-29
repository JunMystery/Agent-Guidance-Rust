# Installation

[Back to README](../README.md)

This project runs as a high-performance **100% Native Rust 2024 Edition** MCP server serving AI agent guidance over Stdio transport.

## Automatic Install

Use the one-line installer script:

**Windows (CMD Prompt):**
```cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 | iex"
```

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 | iex"
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

## Standards Corpus Root

By default, the package discovers the bundled standards corpus. To point the server to a different standards folder, set:

```bash
AGENT_GUIDANCE_ROOT=/path/to/Agent-Guidance
```


The target folder must contain:

- `karpathy/principles.md`
- `SKILL-REFERENCE.md`
- `agent-guidance/INDEX.md`

## Related Docs

- [Client Setup](setup/client-configuration.md)
- [Usage Guide](usage.md)
- [Development Guide](development.md)
