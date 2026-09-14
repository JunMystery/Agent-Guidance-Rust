import { el } from '../dom.js';
import { fetchGraphData } from '../api.js';
import { t } from '../i18n/index.js';
import { renderGraphToolbar } from './graphToolbar.js';
import { initFileGraphCanvas } from './fileGraphCanvas.js';
import { renderFileInspector } from './fileInspectorView.js';
import { initFileContainerList } from './fileContainerList.js';
import { initFunctionDrilldownCanvas } from './functionDrilldownCanvas.js';
import { renderFunctionInspector } from './functionInspectorView.js';
import { initGraphCanvas } from './graphCanvas.js';
import { renderSymbolInspector } from './inspectorView.js';
import { initGraphSymbolList } from './graphSymbolList.js';
import { computeHubThreshold } from './graphLabels.js';

let activeCanvas = null;
let cachedFiles = [];
let currentSelectedFile = '';

export async function switchGraphMode(mode, targetFile = null) {
  let file = targetFile;
  if (mode === 'file_functions') {
    if (!file && cachedFiles.length === 0) {
      const fData = await fetchGraphData({ view: 'files' });
      if (fData && fData.nodes) {
        cachedFiles = fData.nodes.map(n => n.file || n.id).filter(Boolean);
      }
    }
    file = file || currentSelectedFile || 'all';
    if (file) currentSelectedFile = file;
  }
  const data = await fetchGraphData({ view: mode, file });
  renderGraphView(data);
}

export function renderGraphView(data) {
  const container = el('view-graph');
  if (!container) return;

  if (activeCanvas && activeCanvas.destroy) {
    activeCanvas.destroy();
    activeCanvas = null;
  }

  if (!data || !data.graph_available) {
    const isProjRequired = data?.reason === 'PROJECT_REQUIRED';
    container.innerHTML = `
      <div style="padding: 48px 32px; text-align: center; color: var(--text-secondary);">
        <div style="font-size: 40px; margin-bottom: 12px;">${isProjRequired ? '📂' : '🕸️'}</div>
        <h3 style="color: var(--text-primary); margin-bottom: 8px;">${isProjRequired ? t('graph.select_project_title') : t('graph.not_available')}</h3>
        <p style="max-width: 520px; margin: 0 auto 16px auto; font-size: 13px; line-height: 1.5;">${isProjRequired ? t('graph.select_project_prompt') : (data?.message || t('graph.not_indexed_msg'))}</p>
        ${data?.project_path ? `
        <div style="font-family: var(--font-mono); font-size: 11px; background: var(--bg-tertiary); padding: 8px 12px; display: inline-block; border-radius: var(--border-radius-sm);">
          ${t('graph.project_label', { project: data?.project_path || '--' })}
        </div>` : ''}
      </div>
    `;
    return;
  }

  const activeMode = data.view || 'files';
  const nodes = data.nodes || [];
  const edges = data.edges || [];

  if (activeMode === 'files') {
    cachedFiles = nodes.map(n => n.file || n.id).filter(Boolean);
    if (!currentSelectedFile && cachedFiles.length > 0) currentSelectedFile = cachedFiles[0];
  } else if (activeMode === 'file_functions' && data.file) {
    currentSelectedFile = data.file;
    if (!cachedFiles.includes(data.file)) cachedFiles.unshift(data.file);
  }

  container.innerHTML = `
    <div id="graph-toolbar-wrap"></div>
    <div style="display: flex; gap: 16px; height: 560px; min-height: 480px; align-items: stretch; width: 100%; box-sizing: border-box;">
      <div style="flex: 1 1 auto; min-width: 0; background: #080c14; border: 1px solid var(--border-color); border-radius: var(--border-radius-md); position: relative; overflow: hidden;">
        <canvas id="graph-canvas" style="width: 100%; height: 100%; display: block;"></canvas>
        <div id="graph-canvas-hint" style="position: absolute; bottom: 8px; left: 12px; font-size: 11px; color: #64748b; pointer-events: none;"></div>
      </div>
      <div id="graph-inspector" style="width: 360px; min-width: 320px; max-width: 400px; flex-shrink: 0; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 14px; overflow-y: auto; box-sizing: border-box;">
        <div id="inspector-body"></div>
      </div>
    </div>
    <div id="graph-symbol-list-container"></div>
  `;

  const toolbarWrap = el('graph-toolbar-wrap');
  const inspBody = el('inspector-body');
  const hintEl = el('graph-canvas-hint');
  const updateZoomText = (scale) => {
    const zl = el('zoom-val');
    if (zl) zl.textContent = `${Math.round(scale * 100)}%`;
  };

  const copyDag = () => {
    const dag = data.mermaid_dag || `graph TD\n${edges.map(e => `  ${e.source.replace(/[^a-zA-Z0-9_]/g, '_')} --> ${e.target.replace(/[^a-zA-Z0-9_]/g, '_')}`).join('\n')}`;
    navigator.clipboard.writeText(dag).then(() => {
      const b = el('btn-copy-mermaid');
      if (b) { b.textContent = t('graph.copied'); setTimeout(() => { b.textContent = t('graph.copy_dag'); }, 2000); }
    });
  };

  if (activeMode === 'files') {
    let isIsolated = false;
    let isolatedInfo = null;
    let hideOrphans = true;
    let fileList = null;

    const refreshToolbar = () => {
      renderGraphToolbar(toolbarWrap, {
        activeMode: 'files',
        data,
        isIsolated,
        hideOrphans,
        isolatedFileInfo: isolatedInfo,
        onSwitchMode: switchGraphMode,
        onToggleOrphans: () => {
          hideOrphans = !hideOrphans;
          if (activeCanvas) activeCanvas.toggleOrphans(hideOrphans);
          refreshToolbar();
        },
        onResetFilter: () => {
          isIsolated = false;
          isolatedInfo = null;
          if (activeCanvas) activeCanvas.clearIsolate();
          renderFileInspector(inspBody, null, edges, nodes, onJumpFile, onDrilldown);
          if (fileList) fileList.setActiveFile(null);
          refreshToolbar();
        },
        onZoomIn: () => activeCanvas && activeCanvas.zoomIn(),
        onZoomOut: () => activeCanvas && activeCanvas.zoomOut(),
        onZoomFit: () => activeCanvas && activeCanvas.resetFit(),
        onTogglePulse: (p) => activeCanvas && activeCanvas.setPulse(p),
        onCopyDag: copyDag
      });
    };

    const onDrilldown = (targetFilePath) => switchGraphMode('file_functions', targetFilePath);
    const onJumpFile = (targetId) => {
      if (activeCanvas) activeCanvas.selectNode(targetId, true);
      const targetNode = nodes.find(n => n.id === targetId || n.file === targetId);
      if (targetNode) onSelectFileNode(targetNode, true);
    };

    const onSelectFileNode = (node, isolated, fromCanvas = false) => {
      isIsolated = isolated;
      if (node) {
        currentSelectedFile = node.file || node.path || node.id;
        if (fileList) fileList.setActiveFile(node.id);
        const depCount = edges.filter(e => e.source === node.id).length;
        const revCount = edges.filter(e => e.target === node.id).length;
        isolatedInfo = { file: currentSelectedFile, depCount, revCount };
        if (!fromCanvas && activeCanvas && activeCanvas.selectNode) {
          activeCanvas.selectNode(node.id, isolated !== false);
        }
      } else {
        isolatedInfo = null;
        if (fileList) fileList.setActiveFile(null);
        if (!fromCanvas && activeCanvas && activeCanvas.clearIsolate) {
          activeCanvas.clearIsolate();
        }
      }
      refreshToolbar();
      renderFileInspector(inspBody, node, edges, nodes, onJumpFile, onDrilldown);
    };

    refreshToolbar();
    renderFileInspector(inspBody, null, edges, nodes, onJumpFile, onDrilldown);
    if (hintEl) hintEl.textContent = t('graph.file_canvas_hint');
    activeCanvas = initFileGraphCanvas('graph-canvas', nodes, edges, (n, iso) => onSelectFileNode(n, iso, true), updateZoomText);
    fileList = initFileContainerList(el('graph-symbol-list-container'), nodes, onSelectFileNode, onDrilldown);

  } else if (activeMode === 'file_functions') {
    let symbolList = null;
    const onJumpFn = (targetId) => {
      if (activeCanvas) activeCanvas.selectNode(targetId);
      const n = nodes.find(item => item.id === targetId);
      if (n) {
        renderFunctionInspector(inspBody, n, edges, nodes, onJumpFn);
        if (symbolList) symbolList.setActiveNode(n.id);
      }
    };

    renderGraphToolbar(toolbarWrap, {
      activeMode: 'file_functions',
      data,
      availableFiles: cachedFiles,
      selectedFile: currentSelectedFile,
      onSwitchMode: switchGraphMode,
      onSelectFile: (file) => switchGraphMode('file_functions', file),
      onZoomIn: () => activeCanvas && activeCanvas.zoomIn(),
      onZoomOut: () => activeCanvas && activeCanvas.zoomOut(),
      onZoomFit: () => activeCanvas && activeCanvas.resetFit(),
      onTogglePulse: (p) => activeCanvas && activeCanvas.setPulse(p),
      onCopyDag: copyDag
    });

    renderFunctionInspector(inspBody, null, edges, nodes, onJumpFn);
    if (hintEl) hintEl.textContent = t('graph.function_canvas_hint');
    activeCanvas = initFunctionDrilldownCanvas('graph-canvas', currentSelectedFile, nodes, edges, (n) => {
      renderFunctionInspector(inspBody, n, edges, nodes, onJumpFn);
      if (n && symbolList) symbolList.setActiveNode(n.id);
    }, updateZoomText);
    symbolList = initGraphSymbolList(el('graph-symbol-list-container'), nodes, (n) => {
      onJumpFn(n.id);
    });

  } else {
    // Mode 3: Legacy Symbol Graph
    const inDeg = new Map(), outDeg = new Map();
    edges.forEach(e => {
      outDeg.set(e.source, (outDeg.get(e.source) || 0) + 1);
      inDeg.set(e.target, (inDeg.get(e.target) || 0) + 1);
    });
    nodes.forEach(n => {
      n.inDeg = inDeg.get(n.id) || 0;
      n.outDeg = outDeg.get(n.id) || 0;
      n.deg = n.inDeg + n.outDeg;
    });
    const hubThreshold = computeHubThreshold(nodes);
    nodes.forEach(n => { n.isHub = n.deg >= hubThreshold; });

    let symbolList = null;
    const handleSelect = (sel, sync = true) => {
      renderSymbolInspector(inspBody, sel, edges, nodes, (jump) => {
        if (activeCanvas) activeCanvas.selectNode(jump.id);
        handleSelect(jump, true);
      });
      if (sync && symbolList) symbolList.setActiveNode(sel.id);
    };

    renderGraphToolbar(toolbarWrap, {
      activeMode: 'symbols',
      data,
      onSwitchMode: switchGraphMode,
      onFilterChange: (kind, lang) => activeCanvas && activeCanvas.setFilter(kind, lang),
      onToggleOrphans: (hide) => activeCanvas && activeCanvas.toggleOrphans(hide),
      onZoomIn: () => activeCanvas && activeCanvas.zoomIn(),
      onZoomOut: () => activeCanvas && activeCanvas.zoomOut(),
      onZoomFit: () => activeCanvas && activeCanvas.resetFit(),
      onTogglePulse: (p) => activeCanvas && activeCanvas.setPulse(p),
      onCopyDag: copyDag
    });

    handleSelect(null, false);
    if (hintEl) hintEl.textContent = t('graph.canvas_hint');
    activeCanvas = initGraphCanvas('graph-canvas', nodes, edges, data.communities || [], (n) => handleSelect(n, true), updateZoomText);
    symbolList = initGraphSymbolList(el('graph-symbol-list-container'), nodes, (n) => {
      if (activeCanvas) activeCanvas.selectNode(n.id);
      handleSelect(n, false);
    });
  }
}
