# Phase 2: GraphRAG Community Engine & Blast Radius

## 1. Objectives & Scope
Leverage the newly connected `symbol_edges` to generate accurate hierarchical community clusters via the Leiden algorithm, compute dependency coupling, and traverse blast radii.

Key deliverables:
1. Leiden Community Clustering with Real Weights: Utilize edge degrees and weights to form Level 0 (Macro Subsystems), Level 1 (Feature Modules), and Level 2 (Micro Clusters).
2. Blast Radius Engine: Bidirectional traversal (upstream callers + downstream callees) with recursive depth (up to depth 3) and risk scoring.
3. Communities Persistence Schema Alignment: Ensure `communities.json` can be consumed consistently by backend APIs and frontend visualizers.

---

## 2. Technical Design

### 2.1 Leiden Community Clustering (`leiden.rs`)
- Input: `entities: &[GraphEntity]`, `edges: &[GraphEdge]`
- Edge Weighting:
  - `calls`: Weight $1.5 \times \text{call\_frequency}$
  - `imports`: Weight $1.0$
  - `implements`: Weight $2.0$
- Community Summarization: Synthesize high-level architectural responsibility, key entry points (hub symbols), and cohesion metric.

### 2.2 Blast Radius Calculation
Given target symbol $S$:
1. Upstream (Callers): BFS traversal along incoming edges `e.target_id == S`. Identifies components that break if $S$'s contract changes.
2. Downstream (Callees): BFS traversal along outgoing edges `e.source_id == S`. Identifies dependencies of $S$.
3. Risk Score:
   $$\text{Risk} = \min\left(10, \log_2(1 + |\text{Upstream}| \times 2 + |\text{Downstream}|)\right)$$

---

## 3. Implementation Blueprint (< 200 LOC per file)

### `src/context/graph_rag/leiden.rs` [MODIFIED]
- Inject true graph adjacency lists from `edges`.
- Aggregate internal vs external edge counts to compute modularity score.

### `src/context/graph_rag/blast_radius.rs` [NEW / EXTRACTED]
- Extract traversal logic from `mod.rs` to keep files < 200 LOC.
- Methods: `compute_blast_radius(symbol_id, depth) -> BlastRadiusResult`.
- Generates ASCII and Mermaid graph representations.
