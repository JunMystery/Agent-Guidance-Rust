import { el } from '../dom.js';
import { initGraphCanvas } from './graphCanvas.js';

let activeCanvasInstance = null;

export function renderGraphView(data) {
  const container = el('view-graph');
  if (!container) return;

  if (activeCanvasInstance && activeCanvasInstance.destroy) {
    activeCanvasInstance.destroy();
    activeCanvasInstance = null;
  }

  if (!data || !data.graph_available) {
    container.innerHTML = `
      <div style="padding: 32px; text-align: center; color: var(--text-secondary);">
        <div style="font-size: 36px; margin-bottom: 12px;">🕸️</div>
        <h3 style="color: var(--text-primary); margin-bottom: 8px;">Architecture Graph Not Available</h3>
        <p style="max-width: 500px; margin: 0 auto 16px auto; font-size: 13px;">
          ${data?.message || 'No indexed code_graph.db found for this project. Index the codebase using project_context to generate dependency graphs.'}
        </p>
        <div style="font-family: var(--font-mono); font-size: 11px; background: var(--bg-tertiary); padding: 8px 12px; display: inline-block; border-radius: var(--border-radius-sm);">
          Project: ${data?.project_path || '--'}
        </div>
      </div>
    `;
    return;
  }

  const nodes = data.nodes || [];
  const edges = data.edges || [];
  const communities = Array.isArray(data.communities) ? data.communities : (data.communities?.communities || []);
  const mermaidDag = data.mermaid_dag || '';

  // Calculate degrees & hub status
  const inDegree = new Map();
  const outDegree = new Map();
  edges.forEach(e => {
    outDegree.set(e.source, (outDegree.get(e.source) || 0) + 1);
    inDegree.set(e.target, (inDegree.get(e.target) || 0) + 1);
  });
  nodes.forEach(n => {
    n.inDeg = inDegree.get(n.id) || 0;
    n.outDeg = outDegree.get(n.id) || 0;
    n.deg = n.inDeg + n.outDeg;
    n.isHub = n.deg >= 3;
  });

  container.innerHTML = `
    <div style="display: flex; gap: 12px; align-items: center; margin-bottom: 12px; flex-wrap: wrap;">
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">SYMBOLS:</span>
        <strong style="margin-left: 6px; color: var(--text-primary); font-size: 13px;">${data.total_nodes || nodes.length}</strong>
      </div>
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">EDGES:</span>
        <strong style="margin-left: 6px; color: #00e5ff; font-size: 13px;">${data.total_edges || edges.length}</strong>
      </div>
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">CLUSTERS:</span>
        <strong style="margin-left: 6px; color: #a78bfa; font-size: 13px;">${communities.length}</strong>
      </div>

      <div style="display: flex; gap: 6px; align-items: center; margin-left: 8px;">
        <button id="btn-filter-all" class="btn btn-sm btn-primary" style="padding: 4px 10px; font-size: 11px;">All</button>
        <button id="btn-filter-func" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">Functions</button>
        <button id="btn-filter-struct" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">Structs</button>
        <button id="btn-filter-mod" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">Modules</button>
        <button id="btn-filter-hubs" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">🔥 Hubs</button>
        <button id="btn-toggle-orphans" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; margin-left: 6px; border-color: #334155;" title="Hide disconnected isolated nodes">🛡️ Connected Only</button>
      </div>

      <div style="flex: 1;"></div>

      <div style="display: flex; gap: 6px; align-items: center;">
        <label style="display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--text-secondary); cursor: pointer; margin-right: 8px;">
          <input type="checkbox" id="toggle-pulse" checked style="accent-color: #00e5ff; cursor: pointer;">
          <span>⚡ Pulses</span>
        </label>
        <div style="display: flex; gap: 2px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-sm); padding: 2px;">
          <button id="btn-zoom-out" class="btn btn-sm btn-secondary" style="padding: 2px 8px; font-size: 12px; font-weight: bold;" title="Zoom Out">−</button>
          <span id="zoom-val" style="font-family: var(--font-mono); font-size: 11px; color: #00e5ff; min-width: 42px; text-align: center; display: inline-flex; align-items: center; justify-content: center;">82%</span>
          <button id="btn-zoom-in" class="btn btn-sm btn-secondary" style="padding: 2px 8px; font-size: 12px; font-weight: bold;" title="Zoom In">+</button>
          <button id="btn-zoom-fit" class="btn btn-sm btn-secondary" style="padding: 2px 8px; font-size: 11px;" title="Reset Fit">Fit</button>
        </div>
        <button id="btn-copy-mermaid" class="btn btn-sm" style="background: #1e293b; color: #38bdf8; border: 1px solid #0284c7; padding: 4px 12px; font-size: 11px; cursor: pointer;">
          📋 Copy Mermaid DAG
        </button>
      </div>
    </div>

    <div style="display: flex; gap: 16px; height: 560px;">
      <div style="flex: 1; background: #080c14; border: 1px solid var(--border-color); border-radius: var(--border-radius-md); position: relative; overflow: hidden;">
        <canvas id="graph-canvas" style="width: 100%; height: 100%; display: block;"></canvas>
        <div style="position: absolute; bottom: 8px; left: 12px; font-size: 11px; color: #64748b; pointer-events: none;">
          Scroll to Zoom • Drag to Pan • Click node to inspect • Community Auras denote architectural modules
        </div>
      </div>
      <div id="graph-inspector" style="width: 300px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 14px; overflow-y: auto;">
        <div style="font-size: 12px; font-weight: 600; color: var(--text-primary); margin-bottom: 8px;">Symbol Inspector</div>
        <div id="inspector-body" style="font-size: 12px; color: var(--text-secondary); line-height: 1.6;">
          Select any symbol on the graph to inspect AST callers, dependencies, and blast radius.
        </div>
      </div>
    </div>
  `;

  const copyBtn = el('btn-copy-mermaid');
  if (copyBtn && mermaidDag) {
    copyBtn.onclick = () => {
      navigator.clipboard.writeText(mermaidDag).then(() => {
        copyBtn.textContent = '✅ Copied!';
        setTimeout(() => { copyBtn.textContent = '📋 Copy Mermaid DAG'; }, 2000);
      });
    };
  }

  const zoomLabel = el('zoom-val');
  const updateZoomText = (scale) => {
    if (zoomLabel) zoomLabel.textContent = `${Math.round(scale * 100)}%`;
  };

  activeCanvasInstance = initGraphCanvas(
    'graph-canvas',
    nodes,
    edges,
    communities,
    (selectedNode) => updateInspector(selectedNode, edges, nodes),
    updateZoomText
  );

  // Zoom & Filter Controls Wiring
  const btnZoomIn = el('btn-zoom-in');
  const btnZoomOut = el('btn-zoom-out');
  const btnZoomFit = el('btn-zoom-fit');
  const togglePulse = el('toggle-pulse');
  const btnToggleOrphans = el('btn-toggle-orphans');

  if (btnZoomIn) btnZoomIn.onclick = () => activeCanvasInstance.zoomIn();
  if (btnZoomOut) btnZoomOut.onclick = () => activeCanvasInstance.zoomOut();
  if (btnZoomFit) btnZoomFit.onclick = () => activeCanvasInstance.resetFit();
  if (togglePulse) togglePulse.onchange = (e) => activeCanvasInstance.setPulse(e.target.checked);

  let hideOrphans = false;
  if (btnToggleOrphans) {
    btnToggleOrphans.onclick = () => {
      hideOrphans = !hideOrphans;
      activeCanvasInstance.toggleOrphans(hideOrphans);
      btnToggleOrphans.className = 'btn btn-sm ' + (hideOrphans ? 'btn-primary' : 'btn-secondary');
      btnToggleOrphans.textContent = hideOrphans ? '🛡️ Connected (Only)' : '🌐 All Symbols';
    };
  }

  const setupFilter = (id, filter) => {
    const btn = el(id);
    if (!btn) return;
    btn.onclick = () => {
      ['btn-filter-all', 'btn-filter-func', 'btn-filter-struct', 'btn-filter-mod', 'btn-filter-hubs'].forEach(bId => {
        const b = el(bId);
        if (b) b.className = 'btn btn-sm ' + (bId === id ? 'btn-primary' : 'btn-secondary');
      });
      activeCanvasInstance.setFilter(filter);
    };
  };

  setupFilter('btn-filter-all', 'all');
  setupFilter('btn-filter-func', 'function');
  setupFilter('btn-filter-struct', 'struct');
  setupFilter('btn-filter-mod', 'module');
  setupFilter('btn-filter-hubs', 'hubs');
}

function updateInspector(node, edges, allNodes) {
  const insp = el('inspector-body');
  if (!insp || !node) return;
  const nodeMap = new Map(allNodes.map(n => [n.id, n.label]));
  const callers = edges.filter(e => e.target === node.id).map(e => nodeMap.get(e.source) || e.source);
  const callees = edges.filter(e => e.source === node.id).map(e => nodeMap.get(e.target) || e.target);

  insp.innerHTML = `
    <div style="margin-bottom: 8px;"><strong style="color: #00e5ff; font-size: 14px;">${node.label}</strong></div>
    <div><strong>Kind:</strong> <span style="background: var(--bg-tertiary); padding: 2px 6px; border-radius: 4px; font-size: 11px;">${node.kind}</span></div>
    <div style="margin-top: 4px;"><strong>Span:</strong> ${node.loc} lines</div>
    <div style="margin-top: 4px;"><strong>Impact Degree:</strong> ${node.deg} (Callers: ${node.inDeg} | Dependencies: ${node.outDeg})</div>
    ${node.isHub ? '<div style="margin-top: 6px; color: #f59e0b; font-size: 11px; font-weight: 600;">⚡ Critical Architectural Hub</div>' : ''}
    <div style="margin-top: 8px; word-break: break-all;"><strong>File:</strong> <div style="font-family: var(--font-mono); font-size: 10px; color: #94a3b8; margin-top: 2px;">${node.file}</div></div>
    <div style="margin-top: 10px; border-top: 1px solid var(--border-color); padding-top: 8px;">
      <strong style="font-size: 11px; color: #38bdf8;">Incoming Callers (${callers.length}):</strong>
      <div style="font-size: 11px; color: #93c5fd; margin-top: 2px; max-height: 90px; overflow-y: auto;">
        ${callers.length ? callers.join(', ') : '<em>None</em>'}
      </div>
    </div>
    <div style="margin-top: 8px; border-top: 1px solid var(--border-color); padding-top: 8px;">
      <strong style="font-size: 11px; color: #10b981;">Outgoing Dependencies (${callees.length}):</strong>
      <div style="font-size: 11px; color: #86efac; margin-top: 2px; max-height: 90px; overflow-y: auto;">
        ${callees.length ? callees.join(', ') : '<em>None</em>'}
      </div>
    </div>
  `;
}
