# Phase 4: Dashboard HUD Canvas Visualizer

## 1. Objectives & Scope
Transform the graph canvas from a static node scatter plot into a high-performance, interactive Force-Directed Architectural Graph.

Key deliverables:
1. Accurate HUD Counter Metrics: `SYMBOLS`, `EDGES`, `CLUSTERS` reflect authentic data.
2. Clustered Layout: Group nodes organically by Community / Module subsystem using physics-based force simulation or orbital clustering.
3. Directional Bezier Links: Render smooth bezier curves with arrowheads distinguishing call invocations from module imports.
4. Interactive Inspector: Clicking a node isolates incoming callers (cyan), outgoing callees (orange), and provides quick navigation.

---

## 2. Technical Design

### 2.1 Visual Aesthetics (Obsidian Cyber-HUD)
- Canvas Background: Deep obsidian gradient `#0d1117 -> #161b22`.
- Nodes:
  - Functions: Emerald Glow (`#10b981`)
  - Structs / Classes: Cyan Neon (`#06b6d4`)
  - Traits / Interfaces: Violet Glow (`#8b5cf6`)
  - Hubs ($> 3$ connections): Pulsing Amber Rings (`#f59e0b`)
- Links / Edges:
  - Calls: Gradient Cyan-to-Emerald with directional arrowhead.
  - Imports: Semi-transparent Violet dashed lines.
  - Hover / Focus State: Active links light up with 100% opacity; non-connected elements fade to 10% opacity.

### 2.2 Force Simulation Algorithm
A lightweight, dependency-free 2D force relaxation simulation runs for 60 iterations on data load:
$$F = F_{\text{repulsion}} + F_{\text{spring}} + F_{\text{center}}$$
- $F_{\text{repulsion}} = \frac{k^2}{d}$ (keeps symbols distinct)
- $F_{\text{spring}} = \frac{d^2}{k} \times \text{weight}$ (attracts connected caller/callee symbols)
- $F_{\text{center}}$ gently tethers clusters toward screen center.

---

## 3. Implementation Blueprint (< 200 LOC per file)

### `src/dashboard_src/js/render/graphView.js` [REFACTORED]
- Modularized into:
  - `graphView.js`: Main coordinator (< 150 LOC)
  - `graphLayout.js`: Physics & community cluster simulation (< 120 LOC)
  - `graphRenderer.js`: Canvas drawing loops for nodes, bezier arrows, and glows (< 140 LOC)
  - `graphInspector.js`: Sidebar metadata, blast radius viewer, and copy actions (< 120 LOC)
