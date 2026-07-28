# Development Guide

[Back to README](../README.md)

This project is a high-performance **100% Native Rust 2024 Edition** MCP server exposing Agent Guidance MCP over Unix socket daemon transport.

## Setup & Build

Build the server using Rust 2024 (`cargo`):

```bash
cargo build
```

Build optimized release binary:

```bash
cargo build --release
```

## Running the Server

Run directly with Cargo (auto-detects daemon mode):

```bash
cargo run -- --setup
```

Start native web dashboard server:

```bash
cargo run -- --dashboard
```

Force daemon or client mode:

```bash
cargo run -- --force-daemon   # start as daemon (skip auto-detect)
cargo run -- --force-client   # connect to existing daemon
```

## Testing

Run the automated Rust test suite:

```bash
cargo test
```

Run a whitespace check before committing:

```bash
git diff --check
```

## Project Structure

```text
Agent-Guidance-MCP/
├── agent-guidance/          # Core standards corpus
├── docs/                    # Maintainer and user documentation
├── karpathy/                # Karpathy framework references
├── scripts/                 # Installer, launchers
├── skills/                  # On-demand skill capsules
├── src/                     # Rust source code
│   ├── main.rs              # Binary entrypoint — auto-detect daemon/proxy
│   ├── daemon.rs            # Unix socket daemon, ref-counted connections
│   ├── catalog/             # Skills catalog (store, updater)
│   ├── context/             # Project scanner, SQLite FTS5 index
│   ├── dashboard/           # HTTP usage dashboard
│   ├── mcp/                 # MCP protocol engine (router, tools, state)
│   ├── ml/                  # ML models (BERT embeddings, cross-encoder)
│   └── optimizer/           # Token compressor
├── Cargo.toml               # Rust package metadata
├── PROJECT-STANDARDS.md     # Project-specific agent standards
├── README.md                # Compact landing page
└── SKILL-REFERENCE.md       # Skill category reference
```

## Core Source Files

| Module | File | Role |
|---|---|---|
| Entrypoint | `src/main.rs` | CLI flags, auto-detect daemon/proxy mode |
| Daemon | `src/daemon.rs` | Unix socket lifecycle, connection tracking, 30s idle timeout |
| MCP Router | `src/mcp/router.rs` | Tool dispatcher, resource router, initialize handshake |
| MCP Tools | `src/mcp/tools.rs` | Tool handlers (task_pipeline, guidance, project_context, etc.) |
| MCP State | `src/mcp/state.rs` | ServerState priority gate, stage matrix, circuit breaker |
| Embeddings | `src/ml/embeddings.rs` | Candle BERT embedding engine + cached passage vectors |
| Reranker | `src/ml/llm_selector.rs` | Cross-encoder skill reranker |
| Scanner | `src/context/scanner.rs` | Bounded workspace scanner & ignore filter |
| Compressor | `src/optimizer/compressor.rs` | Language-aware token compressor & comment stripper |

## Documentation Notes

- Keep `README.md` compact and link to detailed docs.
- Keep generated documentation such as `docs/SKILLS_OVERVIEW.md` managed by its generator.
- Add new user-facing reference docs under `docs/`.
- Use relative Markdown links so GitHub and IDE previews can open files directly.

## Version Bump

Update these files when releasing a new version:

| File | Line | Action |
|---|---|---|
| `Cargo.toml` | 3 | Set `version = "X.Y.Z"` |
| `.github/workflows/release.yml` | 5 | Update default tag for workflow_dispatch (optional) |

Files that auto-follow via `env!("CARGO_PKG_VERSION")` (no manual change):
- `src/main.rs`, `src/mcp/router.rs`

Procedure: `Cargo.toml` → `cargo build --release` → `git tag vX.Y.Z` → `git push --tags`.

## Related Docs

- [Installation](installation.md)
- [Client Setup](setup/client-configuration.md)
- [MCP Surface](reference/mcp-surface.md)
