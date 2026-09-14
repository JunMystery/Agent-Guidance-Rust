import { el } from '../dom.js';
import { t } from '../i18n/index.js';
import { initFileCombobox } from './fileCombobox.js';

export function renderGraphToolbar(container, options = {}) {
  if (!container) return;

  const {
    activeMode = 'files',
    data = {},
    availableFiles = [],
    selectedFile = '',
    isIsolated = false,
    isolatedFileInfo = null,
    onSwitchMode = () => {},
    onSelectFile = () => {},
    onResetFilter = () => {},
    onFilterChange = () => {},
    onZoomIn = () => {},
    onZoomOut = () => {},
    onZoomFit = () => {},
    onTogglePulse = () => {},
    onToggleOrphans = () => {},
    onCopyDag = () => {}
  } = options;

  const nodesCount = data.total_nodes || (data.nodes ? data.nodes.length : 0);
  const edgesCount = data.total_edges || (data.edges ? data.edges.length : 0);

  const isFilesMode = activeMode === 'files';
  const isDrilldownMode = activeMode === 'file_functions';
  const isSymbolsMode = activeMode === 'symbols';

  let modeSpecificControls = '';

  if (isFilesMode) {
    const connectedCount = data.nodes ? data.nodes.filter(n => ((n.in_degree || 0) + (n.out_degree || 0)) > 0).length : 0;
    const hideOrph = options.hideOrphans !== false;
    modeSpecificControls = `
      <div style="display: flex; gap: 8px; align-items: center;">
        <button id="btn-toggle-file-orphans" class="btn btn-sm ${hideOrph ? 'btn-primary' : 'btn-secondary'}" style="padding: 4px 10px; font-size: 11px;">
          ${hideOrph ? `🔗 ${t('graph.connected')} (${connectedCount})` : `🌐 ${t('graph.all')} (${nodesCount})`}
        </button>
        ${isIsolated && isolatedFileInfo ? `
          <div style="display: inline-flex; align-items: center; gap: 8px; background: rgba(56, 189, 248, 0.12); border: 1px solid rgba(56, 189, 248, 0.4); padding: 3px 10px; border-radius: var(--border-radius-sm); font-size: 11px; color: #38bdf8;">
            <span>🛡️ <strong>${isolatedFileInfo.file ? isolatedFileInfo.file.split(/[/\\]/).pop() : ''}</strong> (${isolatedFileInfo.depCount || 0} ${t('graph.deps_label')}, ${isolatedFileInfo.revCount || 0} ${t('graph.callers_label')})</span>
            <button id="btn-reset-isolate" class="btn btn-sm btn-secondary" style="padding: 2px 6px; font-size: 10px; margin-left: 4px;">
              ${t('graph.reset_filter')}
            </button>
          </div>
        ` : ''}
      </div>
    `;
  } else if (isDrilldownMode) {
    modeSpecificControls = `
      <div style="display: flex; gap: 8px; align-items: center;">
        <label style="font-size: 11px; color: var(--text-secondary); font-weight: 600;">${t('graph.file_label')}:</label>
        <div id="file-combobox-mount"></div>
      </div>
    `;
  } else {
    // Mode 3: Symbols controls
    modeSpecificControls = `
      <div style="display: flex; gap: 6px; align-items: center;">
        <button id="btn-filter-all" class="btn btn-sm btn-primary" style="padding: 4px 10px; font-size: 11px;">${t('graph.all')}</button>
        <button id="btn-filter-func" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.functions')}</button>
        <button id="btn-filter-struct" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.structs')}</button>
        <button id="btn-filter-mod" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.modules')}</button>
        <button id="btn-filter-hubs" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">${t('graph.hubs')}</button>
        <button id="btn-toggle-orphans" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; margin-left: 6px; border-color: #334155;">${t('graph.connected_only')}</button>
      </div>
    `;
  }

  container.innerHTML = `
    <div style="display: flex; flex-direction: column; gap: 10px; margin-bottom: 12px;">
      <!-- Row 1: Mode Switcher Tabs + Stats Badges -->
      <div style="display: flex; gap: 12px; align-items: center; justify-content: space-between; flex-wrap: wrap;">
        <div style="display: flex; gap: 4px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-sm); padding: 3px;">
          <button id="tab-mode-files" class="btn btn-sm ${isFilesMode ? 'btn-primary' : 'btn-secondary'}" style="padding: 5px 12px; font-size: 11px; font-weight: 600;" data-mode="files">
            ${t('graph.mode_files')}
          </button>
          <button id="tab-mode-drilldown" class="btn btn-sm ${isDrilldownMode ? 'btn-primary' : 'btn-secondary'}" style="padding: 5px 12px; font-size: 11px; font-weight: 600;" data-mode="file_functions">
            ${t('graph.mode_drilldown')}
          </button>
          <button id="tab-mode-symbols" class="btn btn-sm ${isSymbolsMode ? 'btn-primary' : 'btn-secondary'}" style="padding: 5px 12px; font-size: 11px; font-weight: 600;" data-mode="symbols">
            ${t('graph.mode_symbols')}
          </button>
        </div>

        <div style="display: flex; gap: 8px; align-items: center;">
          <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 5px 12px; border-radius: var(--border-radius-sm);">
            <span style="color: var(--text-secondary); font-size: 11px;">${isFilesMode ? t('graph.files') : t('graph.symbols')}</span>
            <strong style="margin-left: 6px; color: var(--text-primary); font-size: 12px;">${nodesCount}</strong>
          </div>
          <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); padding: 5px 12px; border-radius: var(--border-radius-sm);">
            <span style="color: var(--text-secondary); font-size: 11px;">${t('graph.edges')}</span>
            <strong style="margin-left: 6px; color: #00e5ff; font-size: 12px;">${edgesCount}</strong>
          </div>
        </div>
      </div>

      <!-- Row 2: Mode Specific Filter & Navigation Controls -->
      <div style="display: flex; gap: 12px; align-items: center; justify-content: space-between; flex-wrap: wrap;">
        <div style="display: flex; gap: 8px; align-items: center; flex-wrap: wrap;">
          ${modeSpecificControls}
        </div>

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
    </div>
  `;

  // Attach event listeners
  const btnFiles = container.querySelector('#tab-mode-files');
  const btnDrilldown = container.querySelector('#tab-mode-drilldown');
  const btnSymbols = container.querySelector('#tab-mode-symbols');

  if (btnFiles) btnFiles.onclick = () => onSwitchMode('files');
  if (btnDrilldown) btnDrilldown.onclick = () => onSwitchMode('file_functions', selectedFile);
  if (btnSymbols) btnSymbols.onclick = () => onSwitchMode('symbols');

  const comboboxMount = container.querySelector('#file-combobox-mount');
  if (comboboxMount) {
    initFileCombobox(comboboxMount, {
      files: availableFiles,
      selectedFile,
      onSelect: onSelectFile
    });
  }

  const resetIsolateBtn = container.querySelector('#btn-reset-isolate');
  if (resetIsolateBtn) resetIsolateBtn.onclick = onResetFilter;

  const btnToggleFileOrphans = container.querySelector('#btn-toggle-file-orphans');
  if (btnToggleFileOrphans) btnToggleFileOrphans.onclick = onToggleOrphans;

  const btnZoomInEl = container.querySelector('#btn-zoom-in');
  const btnZoomOutEl = container.querySelector('#btn-zoom-out');
  const btnZoomFitEl = container.querySelector('#btn-zoom-fit');
  const togglePulseEl = container.querySelector('#toggle-pulse');
  const copyBtnEl = container.querySelector('#btn-copy-mermaid');

  if (btnZoomInEl) btnZoomInEl.onclick = onZoomIn;
  if (btnZoomOutEl) btnZoomOutEl.onclick = onZoomOut;
  if (btnZoomFitEl) btnZoomFitEl.onclick = onZoomFit;
  if (togglePulseEl) togglePulseEl.onchange = (e) => onTogglePulse(e.target.checked);
  if (copyBtnEl) copyBtnEl.onclick = onCopyDag;

  if (isSymbolsMode) {
    const setupSymFilter = (id, kind) => {
      const b = container.querySelector('#' + id);
      if (b) {
        b.onclick = () => {
          ['btn-filter-all', 'btn-filter-func', 'btn-filter-struct', 'btn-filter-mod', 'btn-filter-hubs'].forEach(item => {
            const elBtn = container.querySelector('#' + item);
            if (elBtn) elBtn.className = 'btn btn-sm ' + (item === id ? 'btn-primary' : 'btn-secondary');
          });
          onFilterChange(kind, 'all');
        };
      }
    };
    setupSymFilter('btn-filter-all', 'all');
    setupSymFilter('btn-filter-func', 'function');
    setupSymFilter('btn-filter-struct', 'struct');
    setupSymFilter('btn-filter-mod', 'module');
    setupSymFilter('btn-filter-hubs', 'hubs');

    const toggleOrph = container.querySelector('#btn-toggle-orphans');
    let hideOrphans = false;
    if (toggleOrph) {
      toggleOrph.onclick = () => {
        hideOrphans = !hideOrphans;
        onToggleOrphans(hideOrphans);
        toggleOrph.className = 'btn btn-sm ' + (hideOrphans ? 'btn-primary' : 'btn-secondary');
      };
    }
  }
}
