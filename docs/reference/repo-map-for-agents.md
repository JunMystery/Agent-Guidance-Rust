# Repo Map For Agents

Use this map before changing the repository. Prefer the smallest relevant source file and avoid broad documentation moves unless explicitly requested.

## Root Instruction Files

- `AGENTS.md`: OpenAI Codex and Codex VS Code instructions.
- `CLAUDE.md`: Claude Code instructions.
- `GEMINI.md`: Gemini Code Assist and Gemini CLI instructions.
- `COPILOT.md`: GitHub Copilot Chat instructions.
- `.instructions.md`: VS Code Copilot instructions.
- `.cursorrules`: Windsurf and legacy Cursor fallback.
- `.cursor/rules/karpathy-guidelines.mdc`: Cursor rule file with frontmatter.

These files are generated in the upstream parent repository from `karpathy/principles.md`, `rules/agent-manifest.json`, and `rules/templates/`. The generated files are shipped in this package as-is.

## Core Sources

| Path | Description |
|---|---|
| `src/` | Rust source code for the MCP server |
| `src/main.rs` | Binary entrypoint — daemon/proxy auto-detection, CLI flags |
| `src/daemon.rs` | Unix socket daemon, ref-counted connections, 30s idle timeout |
| `src/mcp/` | MCP protocol engine (router, tools, state, config) |
| `src/ml/` | ML models (BERT embeddings, cross-encoder reranker) |
| `src/catalog/` | Skills catalog (embedded store, updater) |
| `src/context/` | Project scanner, SQLite FTS5 code index |
| `src/optimizer/` | Token compressor |
| `src/dashboard/` | HTTP usage dashboard |
| `Cargo.toml` | Rust package metadata |
| `skills/` | On-demand workflow capsules |
| `agent-guidance/` | Framework documentation, standards, checklists, prompts |
| `docs/` | Maintainer-facing documentation |

## Common Workflows

- Updating core behavior: edit `karpathy/principles.md` in upstream repo.
- Updating a skill: edit only the matching `skills/<name>/SKILL.md`.
- Adding/updating tools: edit `src/mcp/tools.rs` and `src/mcp/router.rs`.
- Updating ML models: edit `src/ml/embeddings.rs` or `src/ml/llm_selector.rs`.
- Releasing: bump `Cargo.toml` → `cargo build --release` → `git tag vX.Y.Z` → `git push --tags`.
- Refactoring large files: load `skills/large-file-refactor/SKILL.md`.

## Verification

Use the narrowest checks that prove the change:

```bash
cargo test
git diff --check
```
