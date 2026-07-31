# Architecture

[Back to README](../README.md)

## Overview

Agent Guidance MCP is a **100% Native Rust 2024 Edition** MCP server that gives AI coding agents standards guidance, skill references, workflow prompts, and bounded project code context. It runs as a **ref-counted daemon** with 30s idle auto-shutdown — models are loaded once and shared across all IDE/CLI connections.

---

## Rust Module Map

```
src/
├── main.rs            # Binary entrypoint — auto-detects daemon/proxy mode
├── daemon.rs          # Unix socket daemon, connection tracking, 30s idle timeout
├── catalog/           # Skills catalog management
│   ├── store.rs       # Embedded rust_embed skills + workspace-local scanning
│   ├── updater.rs     # Async auto-updater for 3rd-party skill repositories
│   └── mod.rs
├── context/           # Project context & indexing
│   ├── scanner.rs     # Bounded workspace scanner & ignore filter
│   ├── db.rs          # SQLite FTS5 code symbol indexing & usage database
│   └── mod.rs
├── dashboard/         # Native HTTP usage dashboard server & embedded HTML frontend
│   └── mod.rs
├── mcp/               # Model Context Protocol engine
│   ├── protocol.rs    # JSON-RPC request & response structs
│   ├── router.rs      # Tool dispatcher & resource router
│   ├── state.rs       # ServerState priority gate, stage matrix & circuit breaker
│   ├── tools.rs       # Tool handlers (task_pipeline, guidance, project_context, etc.)
│   ├── config.rs      # IDE client auto-registration & tagged block section deployment
│   ├── templates.rs   # Embedded AGENTS.md rules & templates
│   └── mod.rs
├── ml/                # Machine learning & vector search
│   ├── embeddings.rs  # Candle BERT (intfloat/multilingual-e5-small) — cached model + passage embeddings
│   ├── llm_selector.rs# Cross-encoder (cross-encoder/ms-marco-MiniLM-L-6-v2) reranker
│   └── mod.rs
└── optimizer/         # Token optimization engine
    ├── compressor.rs  # Language-aware token compressor & comment stripper
    └── mod.rs
```

---

## Transport Architecture

### Daemon / Proxy Auto-Detection

On every launch, `agent-guidance` auto-detects its role:

```
agent-guidance (start)
  ├─ Unix socket ~/.cache/agent-guidance/mcp.sock EXISTS?
  │   └─ YES → PROXY mode: connect socket, forward stdin↔socket↔stdout, exit
  └─ NO → DAEMON mode: bind socket, load models, accept connections
```

### Daemon Mode

```
IDE 1 → stdin/stdout → agent-guidance (DAEMON)
                            │
                       Unix socket ←→ agent-guidance (PROXY) ← stdin/stdout → IDE 2
                                        agent-guidance (PROXY) ← stdin/stdout → IDE 3
```

- **First process** becomes daemon, binds Unix socket, loads models, handles its own stdio
- **Subsequent processes** detect existing socket → connect as proxy (stdio↔socket bridge)
- **Socket path**: `~/.cache/agent-guidance/mcp.sock` (XDG-compliant via `dirs::cache_dir()`)

### Connection Tracking & Idle Shutdown

```
                    Arc<AtomicUsize>
                    ┌────────────────┐
                    │  ref_count = N  │
                    └────────────────┘
                           │
                    ┌──────┴──────┐
                    │  30s timer   │
                    │  (when 0)    │
                    └──────┬──────┘
                           │
              ╔════════════╧════════════╗
              ║ All connections closed  ║
              ║ → wait 30s             ║
              ║ → still 0? → exit      ║
              ╚════════════════════════╝
```

| Event | `ref_count` | Action |
|---|---|---|
| New connection accepted | `+= 1` | Spawn `handle_mcp_lines` task |
| Connection closed | `-= 1` | Check if 0 |
| ref_count reaches 0 | — | Start 30s countdown (checks every 1s) |
| New connection during countdown | `+= 1` | Cancel countdown |
| 30s elapsed, still 0 | — | Delete socket, exit process |

---

## Model Architecture

### Background Model Warmup

The daemon accepts connections immediately and warms both models in a bounded blocking worker:

```
daemon start
  ├─ warmup_cache()
  │   ├─ cached_model() → BERT OnceLock init (~0.6s disk load)
  │   └─ embed all catalog skills as passage vectors (cache miss only)
  └─ cached_cross_encoder() → CrossEncoder OnceLock init (~0.07s)
```

| Component | Model | Size | Init Time |
|---|---|---|---|
| Embedding | `intfloat/multilingual-e5-small` (384-dim) | 118MB | ~560ms |
| Cross-encoder | `cross-encoder/ms-marco-MiniLM-L-6-v2` | 80MB | ~70ms |
| Passage cache | — | 384-dimension f32 vectors, sized by catalog | cache-miss dependent |

### Cached Passage Embeddings (`PASSAGE_CACHE`)

```rust
static PASSAGE_CACHE: OnceLock<RwLock<Vec<PassageCache>>> = OnceLock::new();
```

- Embedded and workspace-local skill passage vectors are cached by catalog fingerprint during `warmup_cache()`
- Shared across all daemon connections via module-level `OnceLock`
- Subsequent `task_pipeline` calls: 1 query embed plus cached vector scoring
- Skill content truncated to 300 chars for embedding

### Hybrid Vector Search

```
task_pipeline(query)
  ├─ embed query: "query: ..." → q_vec (384-dim)
  ├─ for each skill: normalized dot product with cached_passage[i]
  ├─ keyword boost:
  │   ├─ name exact match:  +0.5
  │   ├─ name contains:     +0.3
  │   └─ word in name:      +0.1
  └─ sort → top 8 → LLMSelector::rerank()
```

### Cross-Encoder Reranker

Second-stage re-ranking using a single-output regression cross-encoder:

```rust
struct CrossEncoder {
    model: BertModel,      // MiniLM-L-6 backbone
    classifier: Linear,    // [1, 384] single-output head (not 2-class)
}
```

- Scores: relevance logit from `narrow(1, 0, 1)` (single label)
- Fallback: keyword-frequency ranking if cross-encoder fails

### `--setup` Pre-download

```bash
agent-guidance --setup
  ├─ Configure MCP clients in IDE configs
  └─ download_models()
      ├─ hf_hub: intfloat/multilingual-e5-small → ~/.cache/huggingface/
      └─ hf_hub: cross-encoder/ms-marco-MiniLM-L-6-v2 → ~/.cache/huggingface/
```

Both models are cached on disk by `hf-hub`. The `--setup` flag pre-downloads them so the first MCP session doesn't wait for network.

---

## Priority Gate (2 layers)

```
Tool call
  └─ can_call_tool(name, state)
      ├─ priority_gate_passed? → pass
      └─ blocked → return WORKFLOW_STAGE_BLOCKED

task_pipeline call
  └─ priority_gate_pass()
      └─ Unlocks gate for subsequent calls
```

### Tool Gate Status

| Tool | Gate | Notes |
|---|---|---|
| `task_pipeline` | ✅ Unlocks | Sets `priority_gate_passed = true` |
| `guidance` | 🔒 Gated | Blocked before `task_pipeline` |
| `project_context` | 🔒 Gated | Blocked before `task_pipeline` |
| `ui_ux` | 🔒 Gated | Blocked before `task_pipeline` |
| `session_continuity` | 🔒 Gated | Blocked before `task_pipeline` |
| `workflow_gate` | 🔒 Gated | Blocked before `task_pipeline` |
| `require_edit_approval` | ✅ Open | Delegates to workflow stage check |
| `usage_report` | ✅ Open | — |
| `health_check`, `diagnose`, `token_stats` | ✅ Open | Whitelisted |

---

## Key Flows

### Tool Call Flow

```
AI calls tool
  ├─ can_call_tool(name, arguments)
  │   ├─ priority_gate_passed? → pass
  │   └─ blocked → WORKFLOW_STAGE_BLOCKED
  ├─ handle_request(method, params, &mut state)
  │   └─ match method
  │       ├─ "initialize" → load models, warm up, return capabilities
  │       ├─ "tools/list"  → return tool list
  │       ├─ "tools/call"  → handle_tool_call(name, arguments, state)
  │       └─ "resources/*" → serve resources
  └─ write JSON-RPC response
```

### Task Pipeline

```
task_pipeline(task, project_path)
  ├─ detect_project_path() → resolve workspace root
  ├─ scan_project() → build file tree (capped)
  ├─ load_all_skills() → embedded + workspace-local skills
  ├─ hybrid_vector_search(task, skills, 8) → embedding + keyword
  ├─ LLMSelector::rerank(task, stage1, 8) → cross-encoder scores
  └─ format response: recs + exec sequence + tree preview
```

### 3-Tier Search Fallback

```
project_context(search, query)
  ├─ FTS5 (SQLite full-text index)
  ├─ Documentation + manifests
  ├─ Structural + config files
  └─ General code files (capped)
```

---

## Deployment

### Setup

```bash
agent-guidance --setup
  ├─ configure_mcp_clients() → register in IDE configs
  ├─ configure_global_rules() → append AGENTS.md rules
  ├─ configure_workspace_rules() → append tagged blocks to .cursorrules, etc.
  ├─ configure_skills_enforcer() → write SKILL.md to skill dirs
  └─ download_models() → pre-cache BERT + cross-encoder from HuggingFace
```

### CLI Flags

| Flag | Action |
|---|---|
| `--setup` | Register MCP clients + pre-download models |
| `--update` | Download updated 3rd-party skill repos |
| `--auto-update` | Enable scheduled skill updates |
| `--session-start` | Pass priority gate (for hooks) |
| `--re-gate` | Re-pass priority gate (subagent recovery) |
| `--uninstall` | Remove all registrations + rules |
| `--force-daemon` | Start as daemon (skip auto-detect) |
| `--force-client` | Connect as proxy (fail if no daemon) |
| `--dashboard` | Start HTTP usage dashboard |
| `--project-path` | Specify project root for --session-start |

### Uninstall

```bash
agent-guidance --uninstall
  ├─ remove_mcp_clients() → delete from IDE configs
  ├─ remove_global_rules() → strip tagged blocks
  └─ remove_workspace_rules() → strip tagged blocks
```

All rule/skill sections use HTML-comment tags (`<!-- agent-guidance:start -->` / `<!-- agent-guidance:end -->`) for reliable find-and-replace.

---

## Related

- [MCP Surface](reference/mcp-surface.md) — full tool/resource reference
- [Development Guide](development.md) — tests, project structure, maintainer
- [Installation](installation.md) — automatic and manual setup
- [README](../README.md) — project overview
