import { el } from '../dom.js';
import { initGraphCanvas } from './graphCanvas.js';
import { computeHubThreshold } from './graphLabels.js';
import { renderSymbolInspector } from './inspectorView.js';
import { initGraphSymbolList } from './graphSymbolList.js';
import { t } from '../i18n/index.js';

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
        <h3 style="color: var(--text-primary); margin-bottom: 8px;">${t('graph.not_available')}</h3>
        <p style="max-width: 500px; margin: 0 auto 16px auto; font-size: 13px;">
          ${data?.message || t('graph.not_indexed_msg')}
        </p>
        <div style="font-family: var(--font-mono); font-size: 11px; background: var(--bg-tertiary); padding: 8px 12px; display: inline-block; border-radius: var(--border-radius-sm);">
          ${t('graph.project_label', { project: data?.project_path || '--' })}
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
  });
  const hubThreshold = computeHubThreshold(nodes);
  nodes.forEach(n => {
    n.isHub = n.deg >= hubThreshold;
  });

  container.innerHTML = `
    <div style="display: flex; gap: 12px; align-items: center; margin-bottom: 12px; flex-wrap: wrap;">
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">${t('graph.symbols')}</span>
        <strong style="margin-left: 6px; color: var(--text-primary); font-size: 13px;">${data.total_nodes || nodes.length}</strong>
      </div>
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">${t('graph.edges')}</span>
        <strong style="margin-left: 6px; color: #00e5ff; font-size: 13px;">${data.total_edges || edges.length}</strong>
      </div>
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 6px 14px; border-radius: var(--border-radius-sm);">
        <span style="color: var(--text-secondary); font-size: 11px;">${t('graph.clusters')}</span>
        <strong style="margin-left: 6px; color: #a78bfa; font-size: 13px;">${communities.length}</strong>
      </div>

      <div style="display: flex; gap: 6px; align-items: center; margin-left: 8px;">
        <button id="btn-filter-all" class="btn btn-sm btn-primary" style="padding: 4px 10px; font-size: 11px;">${t('graph.all')}</button>
        <button id="btn-filter-func" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.functions')}</button>
        <button id="btn-filter-struct" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.structs')}</button>
        <button id="btn-filter-mod" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.modules')}</button>
        <button id="btn-filter-hubs" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.hubs')}</button>
        <button id="btn-toggle-orphans" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; margin-left: 6px; border-color: #334155;" title="${t('graph.connected_only')}">${t('graph.connected_only')}</button>
      </div>

      <div style="flex: 1;"></div>

      <div style="display: flex; gap: 6px; align-items: center;">
        <label style="display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--text-secondary); cursor: pointer; margin-right: 8px;">
          <input type="checkbox" id="toggle-pulse" checked style="accent-color: #00e5ff; cursor: pointer;">
          <span>${t('graph.pulses')}</span>
        </label>
        <div style="display: flex; gap: 2px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-sm); padding: 2px;">
          <button id="btn-zoom-out" class="btn btn-sm btn-secondary" style="padding: 2px 8px; font-size: 12px; font-weight: bold;" title="${t('graph.zoom_out')}">−</button>
          <span id="zoom-val" style="font-family: var(--font-mono); font-size: 11px; color: #00e5ff; min-width: 42px; text-align: center; display: inline-flex; align-items: center; justify-content: center;">82%</span>
          <button id="btn-zoom-in" class="btn btn-sm btn-secondary" style="padding: 2px 8px; font-size: 12px; font-weight: bold;" title="${t('graph.zoom_in')}">+</button>
          <button id="btn-zoom-fit" class="btn btn-sm btn-secondary" style="padding: 2px 8px; font-size: 11px;" title="${t('graph.reset_fit')}">${t('graph.zoom_fit')}</button>
        </div>
        <button id="btn-copy-mermaid" class="btn btn-sm" style="background: #1e293b; color: #38bdf8; border: 1px solid #0284c7; padding: 4px 12px; font-size: 11px; cursor: pointer;">
          ${t('graph.copy_dag')}
        </button>
      </div>
    </div>

    <div style="display: flex; gap: 16px; height: 560px;">
      <div style="flex: 1; background: #080c14; border: 1px solid var(--border-color); border-radius: var(--border-radius-md); position: relative; overflow: hidden;">
        <canvas id="graph-canvas" style="width: 100%; height: 100%; display: block;"></canvas>
        <div style="position: absolute; bottom: 8px; left: 12px; font-size: 11px; color: #64748b; pointer-events: none;">
          ${t('graph.canvas_hint')}
        </div>
      </div>
      <div id="graph-inspector" style="width: 340px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 14px; overflow-y: auto;">
        <div id="inspector-body">
          <div style="font-size: 12px; font-weight: 600; color: var(--text-primary); margin-bottom: 8px;">${t('inspector.title')}</div>
          <div style="font-size: 12px; color: var(--text-secondary); line-height: 1.6;">
            ${t('inspector.hint')}
          </div>
        </div>
      </div>
    </div>
    <div id="graph-symbol-list-container"></div>
  `;

  const copyBtn = el('btn-copy-mermaid');
  if (copyBtn && mermaidDag) {
    copyBtn.onclick = () => {
      navigator.clipboard.writeText(mermaidDag).then(() => {
        copyBtn.textContent = t('graph.copied');
        setTimeout(() => { copyBtn.textContent = t('graph.copy_dag'); }, 2000);
      });
    };
  }

  const zoomLabel = el('zoom-val');
  const updateZoomText = (scale) => {
    if (zoomLabel) zoomLabel.textContent = `${Math.round(scale * 100)}%`;
  };

  let symbolList = null;

  const handleSelectNode = (selectedNode, syncList = true) => {
    const insp = el('inspector-body');
    renderSymbolInspector(insp, selectedNode, edges, nodes, (jumpNode) => {
      if (activeCanvasInstance) activeCanvasInstance.selectNode(jumpNode.id);
      handleSelectNode(jumpNode, true);
    });
    if (syncList && symbolList) {
      symbolList.setActiveNode(selectedNode.id);
    }
  };

  activeCanvasInstance = initGraphCanvas(
    'graph-canvas',
    nodes,
    edges,
    communities,
    (node) => handleSelectNode(node, true),
    updateZoomText
  );

  symbolList = initGraphSymbolList(
    el('graph-symbol-list-container'),
    nodes,
    (node) => {
      if (activeCanvasInstance) activeCanvasInstance.selectNode(node.id);
      handleSelectNode(node, false);
    }
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
      btnToggleOrphans.textContent = hideOrphans ? t('graph.connected_only') : t('graph.all_symbols');
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
