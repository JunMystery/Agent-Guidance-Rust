# Contributing to Agent Guidance

Thank you for your interest in contributing to **Agent Guidance**! We welcome contributions from developers, architects, and AI researchers across the open-source community.

Please take a moment to review this document before submitting your contribution.

---

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to [contact@junmystery.com](mailto:contact@junmystery.com).

---

## How Can I Contribute?

### 1. Reporting Bugs
- Search existing [GitHub Issues](https://github.com/JunMystery/Agent-Guidance-Rust/issues) before opening a new report.
- Use our [Bug Report Template](.github/ISSUE_TEMPLATE/bug_report.yml) and include:
  - Agent Guidance version (`agent-guidance --version`)
  - OS and deployment profile (Standalone, Client, Server)
  - IDE / AI client (Claude Code, Cursor, Windsurf, etc.)
  - Clear steps to reproduce and terminal/stderr output

### 2. Suggesting Enhancements
- Open a feature proposal using our [Feature Request Template](.github/ISSUE_TEMPLATE/feature_request.yml).
- Describe the motivation, concrete use-case, and architectural trade-offs.

### 3. Adding or Updating Skills
Agent Guidance embeds a catalog of 279+ domain skills under `skills/`:
- Each skill directory must contain a valid `SKILL.md` with YAML frontmatter:
  ```markdown
  ---
  name: your-skill-name
  description: Short summary of what this skill guides
  ---

  # Your Skill Title

  ## When to Use
  ...
  ## Key Rules
  ...
  ```
- Use `##` header delimiters so our **Surgical Section RAG Slicer** can extract targeted passages.
- Run `agent-guidance --reindex-skills` to verify indexing and precomputed vector generation.

---

## Development Setup

### Prerequisites
- **Rust Toolchain**: Rust 1.80+ (2024 edition). Install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Platform Dependencies**:
  - **Linux**: `sudo apt-get install -y libdbus-1-dev pkg-config`
  - **Windows / macOS**: Standard OS build tools (Visual Studio C++ / Xcode command line tools).

### Building the Project
```bash
git clone https://github.com/JunMystery/Agent-Guidance-Rust.git
cd Agent-Guidance-Rust

# Build debug binary
cargo build

# Build optimized release binary
cargo build --release
```

### Running Tests
All contributions must pass the test suite:
```bash
cargo test --bin agent-guidance
```

---

## Architectural Rules & Code Standards

Agent Guidance enforces strict internal code quality gates that all pull requests must respect:

1. **300 LOC Hard Ceiling**:
   - Every source code file (`.rs`) MUST remain strictly `< 300 LOC` (aim for `< 150 LOC` per sub-module).
   - Decompose complex services into dedicated sub-modules rather than growing monolithic files.
2. **Single Responsibility & Naming**:
   - Do NOT create junk-drawer files or compound plural suffixes (`*services.rs`, `*helpers.rs`, `*utils.rs`, `*controllers.rs`).
   - Each module must have a clear, singular purpose.
3. **Stdio Protocol Purity**:
   - In MCP server mode, never write debug output to `stdout` (it corrupts the JSON-RPC wire protocol).
   - Direct all logging exclusively through `tracing` to `stderr`.
4. **Rust Idioms & Safety**:
   - Avoid `.unwrap()` and `.expect()` in production error paths; propagate errors via `anyhow::Result`.
   - Minimize unnecessary `.clone()` allocations on large buffers or vectors.

---

## Pull Request Workflow

1. **Fork & Branch**:
   ```bash
   git checkout -b feat/your-feature-name
   ```
2. **Make Surgical Changes**:
   - Keep commits focused and atomic.
   - Write unit tests for new functionality under module `tests` modules or `tests/`.
3. **Verify Locally**:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test --bin agent-guidance
   ```
4. **Commit Convention**:
   Follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat: add AST parser for Zig`
   - `fix: prevent tray icon duplicate spawn on Windows`
   - `docs: update MCP surface tool parameters`
   - `perf: optimize cosine dot product loop`
5. **Open Pull Request**:
   - Submit your PR against the `main` branch.
   - Describe the changes, verification steps, and link any related issues.

---

## Community & Questions

- Join discussions and ask questions on [GitHub Discussions](https://github.com/JunMystery/Agent-Guidance-Rust/discussions).
- Check the [Documentation Index](README.md#-documentation-index) for architecture deep dives and tool specs.
