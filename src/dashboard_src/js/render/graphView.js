import { el } from '../dom.js';

export function renderGraphView(data) {
  const container = el('view-graph');
  if (!container) return;

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
  const communities = data.communities || [];
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
        <strong style="margin-left: 6px; color: var(--text-primary); font-size: 13px;">${data.total_edges || edges.length}</strong>
      </div>
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">CLUSTERS:</span>
        <strong style="margin-left: 6px; color: var(--text-primary); font-size: 13px;">${Array.isArray(communities) ? communities.length : 0}</strong>
      </div>
      <div style="display: flex; gap: 6px; align-items: center; margin-left: 8px;">
        <button id="btn-filter-all" class="btn btn-sm btn-primary" style="padding: 4px 10px; font-size: 11px;">All</button>
        <button id="btn-filter-func" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">Functions</button>
        <button id="btn-filter-struct" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">Structs</button>
        <button id="btn-filter-hubs" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">🔥 Hubs</button>
      </div>
      <div style="flex: 1;"></div>
      <button id="btn-copy-mermaid" class="btn btn-sm" style="background: #2b3a4a; color: #64b5f6; border: 1px solid #1e88e5; padding: 4px 12px; font-size: 11px; cursor: pointer;">
        📋 Copy Mermaid DAG
      </button>
    </div>

    <div style="display: flex; gap: 16px; height: 530px;">
      <div style="flex: 1; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); position: relative; overflow: hidden;">
        <canvas id="graph-canvas" style="width: 100%; height: 100%; display: block;"></canvas>
        <div style="position: absolute; bottom: 8px; left: 12px; font-size: 11px; color: var(--text-muted); pointer-events: none;">
          Click node to inspect • Glowing border indicates critical architectural hub
        </div>
      </div>
      <div id="graph-inspector" style="width: 280px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 14px; overflow-y: auto;">
        <div style="font-size: 12px; font-weight: 600; color: var(--text-primary); margin-bottom: 8px;">Symbol Inspector</div>
        <div id="inspector-body" style="font-size: 12px; color: var(--text-secondary); line-height: 1.6;">
          Select any symbol on the graph to inspect AST metadata, blast radius, and dependency connections.
        </div>
      </div>
    </div>
  `;

  // Wire Copy Mermaid Button
  const copyBtn = el('btn-copy-mermaid');
  if (copyBtn && mermaidDag) {
    copyBtn.onclick = () => {
      navigator.clipboard.writeText(mermaidDag).then(() => {
        copyBtn.textContent = '✅ Copied!';
        setTimeout(() => { copyBtn.textContent = '📋 Copy Mermaid DAG'; }, 2000);
      });
    };
  }

  let activeFilter = 'all';
  const network = initNetwork('graph-canvas', nodes, edges, (selectedNode) => {
    updateInspector(selectedNode, edges, nodes);
  });

  const setupFilter = (id, filter) => {
    const btn = el(id);
    if (!btn) return;
    btn.onclick = () => {
      ['btn-filter-all', 'btn-filter-func', 'btn-filter-struct', 'btn-filter-hubs'].forEach(bId => {
        const b = el(bId);
        if (b) {
          b.className = 'btn btn-sm ' + (bId === id ? 'btn-primary' : 'btn-secondary');
        }
      });
      activeFilter = filter;
      network.setFilter(activeFilter);
    };
  };

  setupFilter('btn-filter-all', 'all');
  setupFilter('btn-filter-func', 'function');
  setupFilter('btn-filter-struct', 'struct');
  setupFilter('btn-filter-hubs', 'hubs');
}

function updateInspector(node, edges, allNodes) {
  const insp = el('inspector-body');
  if (!insp || !node) return;
  const nodeMap = new Map(allNodes.map(n => [n.id, n.label]));
  const callers = edges.filter(e => e.target === node.id).map(e => nodeMap.get(e.source) || e.source);
  const callees = edges.filter(e => e.source === node.id).map(e => nodeMap.get(e.target) || e.target);

  insp.innerHTML = `
    <div style="margin-bottom: 8px;"><strong style="color: var(--accent); font-size: 14px;">${node.label}</strong></div>
    <div><strong>Kind:</strong> <span style="background: var(--bg-tertiary); padding: 2px 6px; border-radius: 4px; font-size: 11px;">${node.kind}</span></div>
    <div style="margin-top: 4px;"><strong>LOC:</strong> ${node.loc} lines</div>
    <div style="margin-top: 4px;"><strong>Blast Radius:</strong> ${node.deg} (In: ${node.inDeg} | Out: ${node.outDeg})</div>
    ${node.isHub ? '<div style="margin-top: 6px; color: #ff9800; font-size: 11px; font-weight: 600;">⚠️ Critical Architectural Hub</div>' : ''}
    <div style="margin-top: 8px; word-break: break-all;"><strong>File:</strong> <div style="font-family: var(--font-mono); font-size: 10px; color: var(--text-muted); margin-top: 2px;">${node.file}</div></div>
    <div style="margin-top: 10px; border-top: 1px solid var(--border-color); padding-top: 8px;">
      <strong style="font-size: 11px; color: var(--text-primary);">Incoming Callers (${callers.length}):</strong>
      <div style="font-size: 11px; color: #64b5f6; margin-top: 2px; max-height: 60px; overflow-y: auto;">
        ${callers.length ? callers.slice(0, 6).join(', ') : '<em>None</em>'}
      </div>
    </div>
    <div style="margin-top: 8px; border-top: 1px solid var(--border-color); padding-top: 8px;">
      <strong style="font-size: 11px; color: var(--text-primary);">Outgoing Dependencies (${callees.length}):</strong>
      <div style="font-size: 11px; color: #81c784; margin-top: 2px; max-height: 60px; overflow-y: auto;">
        ${callees.length ? callees.slice(0, 6).join(', ') : '<em>None</em>'}
      </div>
    </div>
  `;
}

function initNetwork(canvasId, nodes, edges, onSelect) {
  const canvas = el(canvasId);
  if (!canvas) return { setFilter: () => {} };
  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);

  const W = rect.width;
  const H = rect.height;
  const nodeMap = new Map();
  const count = Math.min(nodes.length, 75);

  for (let i = 0; i < count; i++) {
    const angle = (i / count) * 2 * Math.PI;
    const r = Math.min(W, H) * 0.35 + ((i % 4) * 22);
    nodeMap.set(nodes[i].id, {
      ...nodes[i],
      x: W / 2 + r * Math.cos(angle),
      y: H / 2 + r * Math.sin(angle),
      radius: Math.max(6, Math.min(16, 6 + (nodes[i].deg || 0) * 1.5))
    });
  }

  let filter = 'all';

  function render() {
    ctx.clearRect(0, 0, W, H);
    // Draw edges
    ctx.strokeStyle = 'rgba(100, 149, 237, 0.22)';
    ctx.lineWidth = 1;
    edges.forEach(e => {
      const src = nodeMap.get(e.source);
      const tgt = nodeMap.get(e.target);
      if (src && tgt) {
        ctx.beginPath();
        ctx.moveTo(src.x, src.y);
        ctx.lineTo(tgt.x, tgt.y);
        ctx.stroke();
      }
    });

    // Draw nodes
    nodeMap.forEach(n => {
      const match = filter === 'all' || (filter === 'hubs' ? n.isHub : n.kind === filter);
      const alpha = match ? 1.0 : 0.18;
      ctx.globalAlpha = alpha;

      if (n.isHub && match) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 3, 0, 2 * Math.PI);
        ctx.strokeStyle = '#ff9800';
        ctx.lineWidth = 2;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, 2 * Math.PI);
      ctx.fillStyle = n.kind === 'function' ? '#4CAF50' : (n.kind === 'struct' ? '#2196F3' : '#9C27B0');
      ctx.fill();
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 1.2;
      ctx.stroke();

      if (match) {
        ctx.fillStyle = '#d0d0d0';
        ctx.font = '10px sans-serif';
        ctx.fillText(n.label, n.x + n.radius + 4, n.y + 3);
      }
    });
    ctx.globalAlpha = 1.0;
  }

  render();

  canvas.addEventListener('click', (ev) => {
    const cr = canvas.getBoundingClientRect();
    const mx = ev.clientX - cr.left;
    const my = ev.clientY - cr.top;
    for (const n of nodeMap.values()) {
      const dist = Math.hypot(n.x - mx, n.y - my);
      if (dist <= n.radius + 6) {
        onSelect(n);
        break;
      }
    }
  });

  return {
    setFilter: (f) => {
      filter = f;
      render();
    }
  };
}
