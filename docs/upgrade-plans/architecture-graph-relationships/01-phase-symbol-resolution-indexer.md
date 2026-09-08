# Phase 1: Symbol Resolution & AST Edge Extraction

## 1. Objectives & Scope
Transform the flat, file-isolated edge extraction into a project-aware AST symbol resolution engine.

Key deliverables:
1. Scope Resolver: Map any line number in a source file to its enclosing symbol (`source_id`).
2. Local & Cross-File Symbol Lookup: Resolve function calls (`callee_name`) and imports to target symbol IDs (`target_id`).
3. Post-Indexing Edge Link Pass: Once all files are indexed in SQLite, run an edge resolution pass that reconciles cross-file references.

---

## 2. Technical Design

### 2.1 Scope Resolver (`find_enclosing_symbol`)
Given an AST call at line $L$ in file $F$:
$$\text{source\_symbol} = \arg\min_{s \in \text{Symbols}(F), s.\text{start} \le L \le s.\text{end}} (s.\text{end} - s.\text{start})$$
The smallest enclosing symbol span identifies the exact caller (e.g. nested function or method instead of parent struct).

### 2.2 Cross-File Edge Linker Pass
1. Build an in-memory Symbol Index `HashMap<String, Vec<String>>` mapping `symbol_name -> [symbol_id_1, symbol_id_2, ...]`.
2. For unresolved edges (e.g. `callee_name` without file prefix):
   - Check if current file has an import matching the callee module/namespace.
   - If unique match exists in project, link `source_id -> target_id`.
   - If multiple candidates exist, score by path proximity (same directory / parent module).
3. Insert resolved edges into `symbol_edges` with type `calls` or `imports`.

---

## 3. Implementation Blueprint (< 200 LOC per file)

### `src/context/indexer/resolver.rs` [NEW FILE]
- `struct SymbolResolver`:
  - `enclosing_symbol_id(symbols: &[ExtractedSymbol], line: usize) -> Option<String>`
  - `resolve_callee(callee_name: &str, current_file: &str, imports: &[String], all_symbols: &HashMap<String, Vec<String>>) -> Option<String>`
  - `resolve_import_target(import_stmt: &str, current_file: &str, file_list: &[String]) -> Option<String>`

### `src/context/indexer/parsers.rs` [MODIFIED]
- Refactor `extract_edges_from_content`:
  - Use `enclosing_symbol_id` to assign caller symbol `source_id`.
  - Fall back to file-level pseudo-symbol only if call is outside any function.

### `src/context/indexer/mod.rs` [MODIFIED]
- In `IncrementalIndexer::full_index` and `incremental_index`:
  - After file indexing completes, execute `self.resolve_cross_file_edges()`.
  - Batch insert resolved edges into `symbol_edges`.
