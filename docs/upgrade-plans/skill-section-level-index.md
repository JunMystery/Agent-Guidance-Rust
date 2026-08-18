# Upgrade Plan: Skill Section-Level Index (ML-Fast Skill Browsing & Proposal)

> Tracking file for the skill browsing / skill proposal mechanism upgrade.
> Status: **APPROVED** (plan_approved = true, stage = Build) — 2026-08-18
> Owner: user + agent-guidance
> Target release: v1.5.0 (planned)
>
> **UPDATE 2026-08-18 (user directive):** Indexing of skill files is performed **by the agent itself, file by file** (not by an external agent). Progress tracked in §12. The prompt skeleton (§5) is retained as regeneration tooling.

---

## 1. Goals

1. Replace the current "embed `name + first 300 chars` per skill" mechanism with a **section-level index dataset** so ML vector search can point to the exact skill section instead of relying on skill intro text.
2. Create a chunked dataset (each chunk ≤ 512 tokens — the BERT embedder hard cap) stored in a dedicated folder, read fast by the embed ML pipeline.
3. Provide a **prompt skeleton** (English) for an external agent to generate the index dataset by summarizing each skill's headings/sections, recording exact line positions into the original `SKILL.md`.
4. Change `select_skills` to read only the matched sections (by line range) from the original skill file — no more dumping the whole skill into context.
5. **Index generation performed by agent-guidance itself**, file by file, across all 175 built-in skills (progress in §12).

## 2. Current Mechanism Analysis (root causes)

| # | Problem | Evidence |
|---|---------|----------|
| P1 | ML only sees `name + first 300 chars` of each skill (frontmatter + intro); the rest is invisible to search | `src/ml/embeddings/cache.rs:118` (`embed_skills_cache`), `src/ml/embeddings/precomputed.rs:97` (`generate_precomputed_cache`), `src/ml/embeddings/search.rs` |
| P2 | **Every** embedded file under `skills/` is treated as a skill — `.schema.json`, `.example.json` (e.g. `session-schemas/*.json`, `session-templates/*.json`) get indexed as skills with unrelated content | `src/catalog/store.rs:265` (`load_all_skills` iterates `list_embedded_skills()` without `.md` filter) |
| P3 | `select_skills` loads the **full** SKILL.md into context; `slice_skill_markdown` re-embeds every section at query time (slow + token-heavy) | `src/mcp/tools/skills.rs:77`, `src/catalog/slicing.rs:40` |
| P4 | Embedder truncates hard at 512 tokens | `src/ml/embeddings/model.rs:84` (`embed_text` → `TruncationParams max_length: 512`) |
| P5 | `catalog_fingerprint` only hashes `relative_path + content`; a change to index files would not invalidate the passage cache | `src/ml/embeddings/precomputed.rs:28` |

## 3. Confirmed Decisions (user answers, 2026-08-18)

- **Index folder location**: `skills/.index/` — inside the repo, embedded into the binary via `rust_embed` (`#[folder = "skills/"]`), auto-synced by `sync_embedded_skills_to_disk()`, shipped with releases, works offline.
- **Index scope**: built-in `skills/` only. Workspace/custom skills (`.claude/skills`, `.opencode/skills`, `~/.agents/skills`) keep the current fallback (name + 300 chars).
- **Retrieval granularity**: **section-level vectors** — one vector per section summary; search returns exact sections; `select_skills` extracts matched line ranges from the original file.
- **Prompt language**: English only.
- **Indexing executor**: agent-guidance (this session) performs indexing file-by-file per §12; external prompt is fallback/regeneration tooling only.

## 4. Dataset Design

### 4.1 Location

```
skills/.index/<skill-name>.json
```

One JSON file per built-in skill. `.index/` will be embedded automatically (rust_embed includes all files under `skills/`).

### 4.2 JSON Schema (schema_version = 1)

```json
{
  "skill": "rust-patterns",
  "source_path": "skills/rust-patterns/SKILL.md",
  "schema_version": 1,
  "sections": [
    {
      "id": "sec-1",
      "heading": "## Error Handling Patterns",
      "heading_level": 2,
      "start_line": 15,
      "end_line": 48,
      "summary": "120-300 word summary. Describes: what the section covers, when to use it, key patterns/APIs, gotchas. MUST be <= 350 tokens.",
      "keywords": ["unwrap", "result", "panic"]
    }
  ]
}
```

### 4.3 Invariants

- `start_line` / `end_line`: **1-based, inclusive**, pointing into the original skill file. `1 <= start_line <= end_line <= total_lines(source_path)`.
- `heading` must match a literal heading line in the source file (`#`, `##`, `###`).
- Each `summary` ≤ 350 tokens (target 120–300 words). Embed input = `"passage: {skill} {heading}\n{summary}"` must stay < 512 tokens.
- Frontmatter, badges, image links, TOC, and boilerplate are **excluded** from sections (summarized at most once into an optional `overview` section if meaningful).
- Sections cover **every** content heading in the source file; no gaps allowed (validator warns on missing ranges).

### 4.4 Embedding text per section (runtime)

```
passage: <skill_name> <heading>
<summary>
```

One passage vector per section → `SectionPassage { skill_index, section_id, vector }`.

### 4.5 Fallback

Skill without an index file (or invalid JSON) → fallback passage = current behavior (`name + first 300 chars`), so the system degrades gracefully before all 175 indexes exist.

## 5. Prompt Skeleton (regeneration tooling — secondary path)

> Full file to be saved at `docs/skills/skill-index-generation-prompt.md`. Primary path is agent-executed indexing (§9 Step 10); this prompt is for regenerating indexes after skill edits or for custom/workspace skills.

```text
# Skill Index Builder

## Role
You are a Skill Index Builder. You produce the section-level index dataset for
the agent-guidance skill catalog so an ML embedding model can retrieve the exact
section of a SKILL.md file that matches a user task.

## Input
- The repository path.
- A list of skill directories under `skills/` (each contains a `SKILL.md`).

## Output
For EVERY skill directory, create ONE JSON file: `skills/.index/<skill-name>.json`
using the exact schema below.

{
  "skill": "<folder name, e.g. rust-patterns>",
  "source_path": "skills/<folder>/SKILL.md",
  "schema_version": 1,
  "sections": [
    {
      "id": "sec-<n>",                       // n = 1, 2, 3 ... sequential
      "heading": "<exact heading line text, e.g. ## Error Handling>",
      "heading_level": 2,                    // 1, 2 or 3
      "start_line": 15,                      // 1-based, inclusive, in source file
      "end_line": 48,                        // 1-based, inclusive, in source file
      "summary": "<120-300 word summary>",
      "keywords": ["kw1", "kw2", "kw3"]
    }
  ]
}

## Workflow — repeat for each SKILL.md
1. Read the whole file. Count its lines (1-based). Note the frontmatter range
   (from line 1 to the closing `---`) and skip it.
2. List all headings (`#`, `##`, `###`). Each heading starts a new section; the
   section body runs from the line after the heading up to (but excluding) the
   next heading of equal or higher level, or the end of the file.
3. For each section, record `start_line` and `end_line` (1-based, inclusive)
   covering heading + body. Verify with a line-by-line check against the file.
4. Write the `summary` (120-300 words) covering, in order:
   - WHAT: what this section is about (key concepts, patterns, APIs, code examples).
   - WHEN: in which situations an agent should apply this section.
   - HOW: the main steps/patterns, names of key functions/structs/commands.
   - GOTCHAS: common mistakes or constraints.
   Use plain markdown text. Do NOT paste long code blocks (max 3 short lines each).
5. Add 3-8 `keywords` — the terms an agent would search for to find this section.

## Rules
- Every section's summary MUST be <= 350 tokens. Shorter is better.
- Exclude from sections: frontmatter, badges, TOC, image embeds, license blocks,
  changelog/history. If they contain anything useful, fold a one-line note into
  the summary of the Overview section.
- Skip empty sections (heading with no body) only if truly empty.
- Do NOT modify any SKILL.md. Only create/overwrite files under `skills/.index/`.
- The file name must be the skill folder name, e.g. `skills/.index/rust-patterns.json`.

## Validation checklist (before writing each file)
- [ ] JSON is valid and matches the schema exactly.
- [ ] `start_line >= 1`, `end_line <= total line count of source file`.
- [ ] `end_line >= start_line`.
- [ ] `heading` matches a literal line in the source file.
- [ ] All content headings of the file are covered by at least one section.
- [ ] No section overlaps another (ranges are disjoint).
- [ ] Each summary is within the token budget.

## Done criteria
- Exactly one `skills/.index/<skill-name>.json` per skill directory in scope.
- All files pass the validation checklist.
- Report the list of created files and the total count.
```

## 6. Rust Implementation Spec

All new files must respect the **300 LOC cap** (split tests via `#[cfg(test)] #[path = "..._tests.rs"] mod tests;` pattern used elsewhere).

### 6.1 NEW `src/catalog/index.rs` (~250 LOC) + `src/catalog/index_tests.rs`

- `pub struct SkillSection { id, heading, heading_level, start_line, end_line, summary, keywords }` (`Deserialize`)
- `pub struct SkillIndex { skill, source_path, schema_version, sections }` (`Deserialize`)
- `pub struct SkillIndexCatalog { entries: HashMap<String, SkillIndex> }`
- `pub fn load_skill_indexes() -> SkillIndexCatalog` — scan `skills/.index/*.json` via `SkillAssets` (rust_embed), parse + validate
- `pub fn index_for(catalog, skill_name) -> Option<&SkillIndex>` — name lookup (try exact, then folder-name fallback)
- `pub fn validate_index(idx) -> Result<(), String>` — invariants in §4.3
- `pub fn extract_sections(content: &str, sections: &[&SkillSection]) -> String` — cuts `content.lines()[start_line-1 .. end_line]` for each section, joins with `---`, compress via `compress_markdown`
- `pub fn section_passage_text(skill: &str, sec: &SkillSection) -> String` — `"{skill} {heading}\n{summary}"`
- Register `pub mod index;` in `src/catalog/mod.rs`.

### 6.2 `src/catalog/store.rs` (fix P2)

- `load_all_skills`: filter embedded paths to **markdown skill files only** (`.md`); skip `.json`, `.bin`, `.png`, etc. (kills the `session-schemas/*.json` noise skills).
- Keep `get_embedded_skill` behavior for lookups (index files are loaded via dedicated `load_skill_indexes`, not as skills).

### 6.3 `src/ml/embeddings/`

- `cache.rs` (~197 → ~250 LOC):
  - Extend `PassageCache` to `enum PassageKind { Skill, Section }` or add a parallel `SECTION_PASSAGE_CACHE` keyed by fingerprint.
  - `embed_skills_cache(candidates, model)` → when index exists: embed each section passage; else fallback `name + 300 chars`.
  - `warmup_cache()` loads indexes and prefers section embeddings; precomputed path unchanged structurally.
- NEW `src/ml/embeddings/section_search.rs` (~150 LOC):
  - `pub struct SectionHit { skill_index: usize, section_id: String, heading: String, start_line: usize, end_line: usize, score: f32 }`
  - `pub fn hybrid_section_search(query, candidates, indexes, top_k) -> Vec<SectionHit>` — dot product of query vector vs section vectors (GPU matrix reuse when fingerprint matches), plus name/keyword boosts; ranks sections.
  - `pub fn skill_scores_from_sections(hits) -> Vec<(f32, usize)>` — max section score per skill, for stage-2 `LLMSelector::rerank` (API unchanged).
- NEW `src/ml/embeddings/precomputed_sections.rs` (~150 LOC):
  - `generate_precomputed_section_cache()` — embed all sections of all indexed skills, save `~/.agent-guidance/sections.bin` + manifest (count, dim, index fingerprint).
  - `load_precomputed_section_cache(indexes) -> Option<Vec<Vec<f32>>>`.
  - Keep `precomputed.rs` below 300 LOC (existing generation stays for fallback skills).
- `precomputed.rs` (fix P5): `catalog_fingerprint` extended to hash index data (or add `index_fingerprint(&indexes)` consumed by section caches).

### 6.4 `src/mcp/tools/skills.rs` (select_skills — section-targeted read)

- When `task` non-empty AND skill has index:
  1. `hybrid_section_search(task, ...)` → top-3 sections for the requested skill.
  2. `extract_sections(raw_content, sections)` from original file content.
  3. Output ONLY those sections with heading + line-range annotation.
- When no `task` / no index: current behavior (`compress_markdown` / `slice_skill_markdown`).
- Output format per skill: `### Skill: <name>\n#### <heading> (lines Lx-Ly)\n<content>`.

### 6.5 `src/mcp/tools/guidance.rs` (search results show evidence)

- `search` / `docs` output: after stage-2 rerank, attach matched section heading + `start_line-end_line` per recommended skill as proposal evidence.

### 6.6 `src/mcp/tools/pipeline.rs` (optional, low priority)

- `task_pipeline` skill proposal list may show matched section titles (only if §6.5 lands cleanly; otherwise skip).

## 7. Seed Indexing (Step 10 output)

Indexes are produced by the agent file-by-file (§12 tracker). The first files (e.g. `skills/.index/rust-patterns.json`) validate the schema E2E; every subsequent file follows the same invariants (§4.3). Line ranges are computed by the agent from the actual file content; summaries ≤ 350 tokens.

## 8. Test Plan

| Test | Location | Verifies |
|---|---|---|
| JSON parse + validation (bad ranges, overlapping sections, missing file) | `index_tests.rs` | §4.3 invariants |
| `extract_sections` 1-based inclusive correctness | `index_tests.rs` | line-range extraction |
| `section_search` returns correct section for a query | `search.rs` / `section_search.rs` tests | retrieval precision |
| Fallback when index missing → old 300-char behavior | `cache.rs` / `skills.rs` tests | backward compatibility |
| `load_all_skills` excludes `.json`/non-md embedded files | `store.rs` tests | P2 fix |
| `select_skills` with task returns only matched sections | `tools_tests.rs` | section-targeted read |

Commands:
- `cargo test`
- `cargo run -- --generate-precomputed` (regenerates `precomputed_vectors.bin` + manifest — run only after indexes exist)

## 9. Build Order (execution checklist)

- [ ] **Step 1** — Save prompt skeleton to `docs/skills/skill-index-generation-prompt.md` (§5 content) — regeneration tooling
- [x] **Step 2** — Plan updated: agent-executed indexing directive + progress tracker (§12)
- [ ] **Step 3** — `store.rs`: filter non-md embedded skills + fingerprint fix (P2, P5)
- [ ] **Step 4** — `src/catalog/index.rs` + `index_tests.rs` + `mod.rs` registration (§6.1)
- [ ] **Step 5** — `cache.rs` section passages + `section_search.rs` + `precomputed_sections.rs` (§6.3)
- [ ] **Step 6** — `skills.rs` section-targeted `select_skills` (§6.4)
- [ ] **Step 7** — `guidance.rs` evidence lines (§6.5)
- [ ] **Step 8** — Full test suite + review (§8)
- [ ] **Step 9** — (After Step 10 completes) regenerate precomputed cache, run bench `benches/performance.rs`
- [ ] **Step 10** — **Index all 175 built-in skills file-by-file** → `skills/.index/<skill-name>.json`; progress in §12 (IN PROGRESS)

### 9.1 Agent indexing procedure (per skill)

1. Read the full `skills/<name>/SKILL.md` (page past 300 LOC when needed).
2. Locate frontmatter (line 1 → closing `---`); skip it.
3. List every `#`/`##`/`###` heading; section range = heading line → line before next heading of equal/higher level (or EOF). 1-based inclusive.
4. Write summary per §4.3 budget + 3-8 keywords.
5. Write `skills/.index/<name>.json`; mark §12 tracker DONE.

## 10. Out of Scope

- Index files for workspace/custom skills (fallback behavior retained).
- `skills/README.md` (repo doc, not a skill — excluded from indexing).
- Changes to the 512-token embedder (`model.rs`).

## 11. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Line numbers drift if SKILL.md is edited after index generation | Indexes regenerated by external agent; fingerprint includes index hash so stale caches invalidate; extraction clamps out-of-range lines |
| Bigger precomputed payload (175 skills × ~6 sections × 384 dims × 4B ≈ ~1.6 MB) | Acceptable; loaded lazily; on-disk cache in `~/.agent-guidance/` |
| `skills/.index/` files mistaken for skills | Explicit `.md`-only filter in `load_all_skills` (Step 3) |
| Backward compat before full index coverage | Fallback path (§4.5) preserved and tested |

## 12. Progress Tracker (agent-executed indexing, file-by-file)

Legend: `[ ]` pending · `[x]` index written · `[~]` index invalid/needs rework

| # | Skill | Index file | Status |
|---|-------|------------|--------|
| 1 | accessibility | `skills/.index/accessibility.json` | [x] |
| 2 | adaptive-language | `skills/.index/adaptive-language.json` | [x] |
| 3 | agent-sort | `skills/.index/agent-sort.json` | [x] |
| 4 | agent-workflow-ops | `skills/.index/agent-workflow-ops.json` | [x] |
| 5 | android-clean-architecture | `skills/.index/android-clean-architecture.json` | [x] |
| 6 | api-design | `skills/.index/api-design.json` | [x] |
| 7 | architecture-decision-records | `skills/.index/architecture-decision-records.json` | [x] |
| 8 | automation-audit-ops | `skills/.index/automation-audit-ops.json` | [x] |
| 9 | backend-patterns | `skills/.index/backend-patterns.json` | [x] |
| 10 | blender-motion-state-inspection | `skills/.index/blender-motion-state-inspection.json` | [x] |
| 11 | blueprint | `skills/.index/blueprint.json` | [x] |
| 12 | browser-qa | `skills/.index/browser-qa.json` | [x] |
| 13 | bun-runtime | `skills/.index/bun-runtime.json` | [x] |
| 14 | canary-watch | `skills/.index/canary-watch.json` | [x] |
| 15 | carrier-relationship-management | `skills/.index/carrier-relationship-management.json` | [x] |
| 16 | ci-cd-and-automation | `skills/.index/ci-cd-and-automation.json` | [x] |
| 17 | cisco-ios-patterns | `skills/.index/cisco-ios-patterns.json` | [x] |
| 18 | claude-devfleet | `skills/.index/claude-devfleet.json` | [x] |
| 19 | codebase-onboarding | `skills/.index/codebase-onboarding.json` | [x] |
| 20 | codehealth-mcp | `skills/.index/codehealth-mcp.json` | [x] |
| 21 | code-review-and-quality | `skills/.index/code-review-and-quality.json` | [x] |
| 22 | code-reviewer | `skills/.index/code-reviewer.json` | [x] |
| 23 | code-simplification | `skills/.index/code-simplification.json` | [x] |
| 24 | codex-vscode | `skills/.index/codex-vscode.json` | [x] |
| 25 | coding-standards | `skills/.index/coding-standards.json` | [x] |
| 26 | compose-multiplatform-patterns | `skills/.index/compose-multiplatform-patterns.json` | [ ] |
| 27 | configure-ecc | `skills/.index/configure-ecc.json` | [ ] |
| 28 | connections-optimizer | `skills/.index/connections-optimizer.json` | [ ] |
| 29 | context-budget | `skills/.index/context-budget.json` | [ ] |
| 30 | continuous-learning-v2 | `skills/.index/continuous-learning-v2.json` | [ ] |
| 31 | cost-aware-llm-pipeline | `skills/.index/cost-aware-llm-pipeline.json` | [ ] |
| 32 | cost-tracking | `skills/.index/cost-tracking.json` | [ ] |
| 33 | council | `skills/.index/council.json` | [ ] |
| 34 | cpp-coding-standards | `skills/.index/cpp-coding-standards.json` | [ ] |
| 35 | cpp-testing | `skills/.index/cpp-testing.json` | [ ] |
| 36 | customer-billing-ops | `skills/.index/customer-billing-ops.json` | [ ] |
| 37 | customs-trade-compliance | `skills/.index/customs-trade-compliance.json` | [ ] |
| 38 | database-migrations | `skills/.index/database-migrations.json` | [ ] |
| 39 | data-scraper-agent | `skills/.index/data-scraper-agent.json` | [ ] |
| 40 | debugging-and-error-recovery | `skills/.index/debugging-and-error-recovery.json` | [ ] |
| 41 | deep-research | `skills/.index/deep-research.json` | [ ] |
| 42 | deployment-patterns | `skills/.index/deployment-patterns.json` | [ ] |
| 43 | deprecation-and-migration | `skills/.index/deprecation-and-migration.json` | [ ] |
| 44 | design-system | `skills/.index/design-system.json` | [ ] |
| 45 | django-celery | `skills/.index/django-celery.json` | [ ] |
| 46 | django-patterns | `skills/.index/django-patterns.json` | [ ] |
| 47 | django-security | `skills/.index/django-security.json` | [ ] |
| 48 | django-tdd | `skills/.index/django-tdd.json` | [ ] |
| 49 | django-verification | `skills/.index/django-verification.json` | [ ] |
| 50 | dmux-workflows | `skills/.index/dmux-workflows.json` | [ ] |
| 51 | docker-patterns | `skills/.index/docker-patterns.json` | [ ] |
| 52 | documentation-lookup | `skills/.index/documentation-lookup.json` | [ ] |
| 53 | doubt-driven-development | `skills/.index/doubt-driven-development.json` | [ ] |
| 54 | e2e-testing | `skills/.index/e2e-testing.json` | [ ] |
| 55 | energy-procurement | `skills/.index/energy-procurement.json` | [ ] |
| 56 | error-handling | `skills/.index/error-handling.json` | [ ] |
| 57 | eval-harness | `skills/.index/eval-harness.json` | [ ] |
| 58 | fastapi-patterns | `skills/.index/fastapi-patterns.json` | [ ] |
| 59 | finance-billing-ops | `skills/.index/finance-billing-ops.json` | [ ] |
| 60 | foundation-models-on-device | `skills/.index/foundation-models-on-device.json` | [ ] |
| 61 | frontend-a11y | `skills/.index/frontend-a11y.json` | [ ] |
| 62 | frontend-patterns | `skills/.index/frontend-patterns.json` | [ ] |
| 63 | gan-style-harness | `skills/.index/gan-style-harness.json` | [ ] |
| 64 | github-ops | `skills/.index/github-ops.json` | [ ] |
| 65 | git-workflow | `skills/.index/git-workflow.json` | [ ] |
| 66 | golang-patterns | `skills/.index/golang-patterns.json` | [ ] |
| 67 | golang-testing | `skills/.index/golang-testing.json` | [ ] |
| 68 | google-workspace-ops | `skills/.index/google-workspace-ops.json` | [ ] |
| 69 | healthcare-emr-patterns | `skills/.index/healthcare-emr-patterns.json` | [ ] |
| 70 | healthcare-eval-harness | `skills/.index/healthcare-eval-harness.json` | [ ] |
| 71 | hermes-imports | `skills/.index/hermes-imports.json` | [ ] |
| 72 | homelab-network-setup | `skills/.index/homelab-network-setup.json` | [ ] |
| 73 | homelab-pihole-dns | `skills/.index/homelab-pihole-dns.json` | [ ] |
| 74 | homelab-vlan-segmentation | `skills/.index/homelab-vlan-segmentation.json` | [ ] |
| 75 | homelab-wireguard-vpn | `skills/.index/homelab-wireguard-vpn.json` | [ ] |
| 76 | hookify-rules | `skills/.index/hookify-rules.json` | [ ] |
| 77 | humanizer | `skills/.index/humanizer.json` | [ ] |
| 78 | idea-refine | `skills/.index/idea-refine.json` | [ ] |
| 79 | incremental-implementation | `skills/.index/incremental-implementation.json` | [ ] |
| 80 | intent-driven-development | `skills/.index/intent-driven-development.json` | [ ] |
| 81 | interview-me | `skills/.index/interview-me.json` | [ ] |
| 82 | inventory-demand-planning | `skills/.index/inventory-demand-planning.json` | [ ] |
| 83 | investor-materials | `skills/.index/investor-materials.json` | [ ] |
| 84 | investor-outreach | `skills/.index/investor-outreach.json` | [ ] |
| 85 | ios-icon-gen | `skills/.index/ios-icon-gen.json` | [ ] |
| 86 | iterative-retrieval | `skills/.index/iterative-retrieval.json` | [ ] |
| 87 | ito-prediction-market-ops | `skills/.index/ito-prediction-market-ops.json` | [ ] |
| 88 | java-coding-standards | `skills/.index/java-coding-standards.json` | [ ] |
| 89 | jira-integration | `skills/.index/jira-integration.json` | [ ] |
| 90 | jpa-patterns | `skills/.index/jpa-patterns.json` | [ ] |
| 91 | knowledge-ops | `skills/.index/knowledge-ops.json` | [ ] |
| 92 | kotlin-coroutines-flows | `skills/.index/kotlin-coroutines-flows.json` | [ ] |
| 93 | kotlin-exposed-patterns | `skills/.index/kotlin-exposed-patterns.json` | [ ] |
| 94 | kotlin-patterns | `skills/.index/kotlin-patterns.json` | [ ] |
| 95 | kubernetes-patterns | `skills/.index/kubernetes-patterns.json` | [ ] |
| 96 | laravel-plugin-discovery | `skills/.index/laravel-plugin-discovery.json` | [ ] |
| 97 | large-file-refactor | `skills/.index/large-file-refactor.json` | [ ] |
| 98 | lead-intelligence | `skills/.index/lead-intelligence.json` | [ ] |
| 99 | liquid-glass-design | `skills/.index/liquid-glass-design.json` | [ ] |
| 100 | logistics-exception-management | `skills/.index/logistics-exception-management.json` | [ ] |
| 101 | market-research | `skills/.index/market-research.json` | [ ] |
| 102 | mcp-server-patterns | `skills/.index/mcp-server-patterns.json` | [ ] |
| 103 | media-doc-processing | `skills/.index/media-doc-processing.json` | [ ] |
| 104 | messages-ops | `skills/.index/messages-ops.json` | [ ] |
| 105 | mle-workflow | `skills/.index/mle-workflow.json` | [ ] |
| 106 | motion-design | `skills/.index/motion-design.json` | [ ] |
| 107 | nanoclaw-repl | `skills/.index/nanoclaw-repl.json` | [ ] |
| 108 | netmiko-ssh-automation | `skills/.index/netmiko-ssh-automation.json` | [ ] |
| 109 | network-bgp-diagnostics | `skills/.index/network-bgp-diagnostics.json` | [ ] |
| 110 | network-config-validation | `skills/.index/network-config-validation.json` | [ ] |
| 111 | network-interface-health | `skills/.index/network-interface-health.json` | [ ] |
| 112 | observability-and-instrumentation | `skills/.index/observability-and-instrumentation.json` | [ ] |
| 113 | openclaw-persona-forge | `skills/.index/openclaw-persona-forge.json` | [ ] |
| 114 | opensource-pipeline | `skills/.index/opensource-pipeline.json` | [ ] |
| 115 | performance-optimization | `skills/.index/performance-optimization.json` | [ ] |
| 116 | plankton-code-quality | `skills/.index/plankton-code-quality.json` | [ ] |
| 117 | planning-and-task-breakdown | `skills/.index/planning-and-task-breakdown.json` | [ ] |
| 118 | product-capability | `skills/.index/product-capability.json` | [ ] |
| 119 | production-audit | `skills/.index/production-audit.json` | [ ] |
| 120 | production-scheduling | `skills/.index/production-scheduling.json` | [ ] |
| 121 | product-lens | `skills/.index/product-lens.json` | [ ] |
| 122 | project-flow-ops | `skills/.index/project-flow-ops.json` | [ ] |
| 123 | prompt-optimizer | `skills/.index/prompt-optimizer.json` | [ ] |
| 124 | python-patterns | `skills/.index/python-patterns.json` | [ ] |
| 125 | python-testing | `skills/.index/python-testing.json` | [ ] |
| 126 | pytorch-patterns | `skills/.index/pytorch-patterns.json` | [ ] |
| 127 | quality-nonconformance | `skills/.index/quality-nonconformance.json` | [ ] |
| 128 | ralphinho-rfc-pipeline | `skills/.index/ralphinho-rfc-pipeline.json` | [ ] |
| 129 | react-patterns | `skills/.index/react-patterns.json` | [ ] |
| 130 | react-performance | `skills/.index/react-performance.json` | [ ] |
| 131 | react-testing | `skills/.index/react-testing.json` | [ ] |
| 132 | recsys-pipeline-architect | `skills/.index/recsys-pipeline-architect.json` | [ ] |
| 133 | recursive-decision-ledger | `skills/.index/recursive-decision-ledger.json` | [ ] |
| 134 | regex-vs-llm-structured-text | `skills/.index/regex-vs-llm-structured-text.json` | [ ] |
| 135 | returns-reverse-logistics | `skills/.index/returns-reverse-logistics.json` | [ ] |
| 136 | rules-distill | `skills/.index/rules-distill.json` | [ ] |
| 137 | rust-patterns | `skills/.index/rust-patterns.json` | [ ] |
| 138 | rust-testing | `skills/.index/rust-testing.json` | [ ] |
| 139 | santa-method | `skills/.index/santa-method.json` | [ ] |
| 140 | scientific-research | `skills/.index/scientific-research.json` | [ ] |
| 141 | search-first | `skills/.index/search-first.json` | [ ] |
| 142 | security-auditor | `skills/.index/security-auditor.json` | [ ] |
| 143 | security-review | `skills/.index/security-review.json` | [ ] |
| 144 | session-context-ops | `skills/.index/session-context-ops.json` | [ ] |
| 145 | session-schemas | `skills/.index/session-schemas.json` | [ ] |
| 146 | session-templates | `skills/.index/session-templates.json` | [ ] |
| 147 | shipping-and-launch | `skills/.index/shipping-and-launch.json` | [ ] |
| 148 | skill-comply | `skills/.index/skill-comply.json` | [ ] |
| 149 | skill-scout | `skills/.index/skill-scout.json` | [ ] |
| 150 | skill-stocktake | `skills/.index/skill-stocktake.json` | [ ] |
| 151 | source-driven-development | `skills/.index/source-driven-development.json` | [ ] |
| 152 | spec-driven-development | `skills/.index/spec-driven-development.json` | [ ] |
| 153 | springboot-patterns | `skills/.index/springboot-patterns.json` | [ ] |
| 154 | springboot-security | `skills/.index/springboot-security.json` | [ ] |
| 155 | springboot-tdd | `skills/.index/springboot-tdd.json` | [ ] |
| 156 | springboot-verification | `skills/.index/springboot-verification.json` | [ ] |
| 157 | standards-guide | `skills/.index/standards-guide.json` | [ ] |
| 158 | strategic-compact | `skills/.index/strategic-compact.json` | [ ] |
| 159 | swift-actor-persistence | `skills/.index/swift-actor-persistence.json` | [ ] |
| 160 | swift-concurrency-6-2 | `skills/.index/swift-concurrency-6-2.json` | [ ] |
| 161 | tdd-workflow | `skills/.index/tdd-workflow.json` | [ ] |
| 162 | team-builder | `skills/.index/team-builder.json` | [ ] |
| 163 | terminal-ops | `skills/.index/terminal-ops.json` | [ ] |
| 164 | test-engineer | `skills/.index/test-engineer.json` | [ ] |
| 165 | tools-cost-audit | `skills/.index/tools-cost-audit.json` | [ ] |
| 166 | ui-ux-pro-max | `skills/.index/ui-ux-pro-max.json` | [ ] |
| 167 | uncloud | `skills/.index/uncloud.json` | [ ] |
| 168 | unified-notifications-ops | `skills/.index/unified-notifications-ops.json` | [ ] |
| 169 | using-agent-skills | `skills/.index/using-agent-skills.json` | [ ] |
| 170 | verification-loop | `skills/.index/verification-loop.json` | [ ] |
| 171 | vue-patterns | `skills/.index/vue-patterns.json` | [ ] |
| 172 | web-performance-auditor | `skills/.index/web-performance-auditor.json` | [ ] |
| 173 | workflow-modes | `skills/.index/workflow-modes.json` | [ ] |
| 174 | workspace-surface-audit | `skills/.index/workspace-surface-audit.json` | [ ] |
