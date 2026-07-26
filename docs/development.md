# Development Guide

[Back to README](../README.md)

This project is a high-performance **100% Native Rust 2024 Edition** MCP server exposing Agent Guidance MCP over Stdio transport.

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

Run directly with Cargo:

```bash
cargo run -- --setup
```

Start native web dashboard server:

```bash
cargo run -- --dashboard
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
├── docs/                        # Maintainer and user documentation
├── karpathy/                    # Karpathy framework references
├── scripts/                     # Installer, launchers, docs generators
├── skills/                      # On-demand skill capsules
├── src/agent_guidance_mcp/  # Python package source
├── tests/                       # Pytest suite
├── PROJECT-STANDARDS.md         # Project-specific agent standards
├── pyproject.toml               # Python package metadata
├── README.md                    # Compact landing page
└── SKILL-REFERENCE.md           # Skill category reference
```

## Core Source Files

- `server.py`: FastMCP registration and MCP surface declarations.
- `catalog.py`: standards catalog indexing, search, and recommendations.
- `paths.py`: standards root discovery and safe corpus path resolution.
- `text.py`: text normalization, snippet, and keyword helpers.
- `project_context.py`: public project-context tool helpers.
- `project_scan.py`: project-context traversal and filtering internals.
- `__main__.py`: command-line module launcher.

## Documentation Notes

- Keep `README.md` compact and link to detailed docs.
- Keep generated documentation such as `docs/SKILLS_OVERVIEW.md` managed by its generator.
- Add new user-facing reference docs under `docs/`.
- Use relative Markdown links so GitHub and IDE previews can open files directly.

## Packaging Notes

The wheel includes:

- `SKILL-REFERENCE.md`
- `docs/`
- `karpathy/`
- `skills/`
- `agent-guidance/`

These paths are configured in `pyproject.toml`.

## Version Bump

Update these files when releasing a new version:

| File | Line | Action |
|---|---|---|
| `Cargo.toml` | 3 | Set `version = "X.Y.Z"` |

Files that auto-follow via `env!("CARGO_PKG_VERSION")` (no manual change):
- `src/main.rs`, `src/mcp/router.rs`, `src/mcp/tools.rs`

Procedure: `Cargo.toml` → `cargo build --release` → test.

## Related Docs

- [Installation](installation.md)
- [Client Setup](setup/client-configuration.md)
- [MCP Surface](reference/mcp-surface.md)
