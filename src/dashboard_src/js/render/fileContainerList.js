import { t } from '../i18n/index.js';

function escapeHtml(str) {
  return String(str || '').replace(/[&<>"']/g, m => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[m]));
}

export function initFileContainerList(container, nodes = [], onSelectFile, onDrilldown) {
  if (!container) return { setActiveFile: () => {} };

  let query = '';
  let filterMode = 'connected'; // 'connected' | 'all' | 'hubs'
  let activeId = null;

  container.innerHTML = `
    <div style="margin-top: 20px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 14px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; flex-wrap: wrap; gap: 8px;">
        <div style="display: flex; align-items: center; gap: 8px;">
          <span style="font-size: 13px; font-weight: 600; color: #f8fafc;">📁 ${t('graph.file_navigator_title')}</span>
          <span id="file-list-count" style="font-size: 11px; background: #0284c720; color: #38bdf8; border: 1px solid #0284c740; padding: 2px 8px; border-radius: 12px; font-family: var(--font-mono); font-weight: 600;"></span>
        </div>
        <div style="display: flex; gap: 6px; align-items: center;">
          <button class="file-filter-btn active" data-mode="connected" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: #0284c7; color: white; cursor: pointer;">${t('graph.nav_connected_only')}</button>
          <button class="file-filter-btn" data-mode="all" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;">${t('graph.nav_all_files')}</button>
          <button class="file-filter-btn" data-mode="hubs" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;">${t('graph.nav_top_hubs')}</button>
        </div>
      </div>
      <div style="position: relative; margin-bottom: 12px;">
        <input type="text" id="file-search-input" placeholder="${t('graph.nav_search_placeholder')}" style="width: 100%; box-sizing: border-box; padding: 8px 12px; background: #080c14; border: 1px solid #334155; border-radius: 6px; color: #f1f5f9; font-size: 12px; font-family: var(--font-mono); outline: none;">
      </div>
      <div id="file-tree-wrap" style="max-height: 440px; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; padding-right: 4px;"></div>
    </div>
  `;

  const input = container.querySelector('#file-search-input');
  const treeWrap = container.querySelector('#file-tree-wrap');
  const countBadge = container.querySelector('#file-list-count');

  function renderList() {
    const rawQ = query.trim().toLowerCase();
    const filtered = nodes.filter(n => {
      const deg = (n.in_degree || 0) + (n.out_degree || 0);
      if (filterMode === 'connected' && deg === 0) return false;
      if (filterMode === 'hubs' && deg < 4) return false;
      if (!rawQ) return true;
      const p = (n.file || n.path || n.id || '').toLowerCase();
      return p.includes(rawQ);
    });

    if (countBadge) {
      countBadge.textContent = `${filtered.length} ${t('graph.files_count_suffix')}`;
    }

    if (!filtered.length) {
      treeWrap.innerHTML = `<div style="padding: 24px; text-align: center; color: var(--text-secondary); font-size: 12px;">${t('graph.nav_no_match')}</div>`;
      return;
    }

    const groups = new Map();
    filtered.forEach(n => {
      const pathStr = (n.file || n.path || n.id).replace(/\\/g, '/');
      const idx = pathStr.lastIndexOf('/');
      const dir = idx >= 0 ? pathStr.slice(0, idx) : '.';
      if (!groups.has(dir)) groups.set(dir, []);
      groups.get(dir).push(n);
    });

    let html = '';
    groups.forEach((fileList, dir) => {
      html += `
        <div style="background: #0f172a; border: 1px solid #1e293b; border-radius: 6px; overflow: hidden; flex-shrink: 0;">
          <div style="background: #1e293b; padding: 6px 10px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #334155;">
            <div style="display: flex; align-items: center; gap: 6px;">
              <span style="font-size: 13px;">📁</span>
              <span style="font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: #38bdf8;">${escapeHtml(dir)}</span>
            </div>
            <span style="font-size: 10px; color: #94a3b8; background: #0f172a; padding: 1px 6px; border-radius: 4px;">${fileList.length} ${t('graph.files_count_suffix')}</span>
          </div>
          <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 6px; padding: 8px;">
            ${fileList.map(n => {
              const p = (n.file || n.path || n.id).replace(/\\/g, '/');
              const fname = p.split('/').pop();
              const isSel = activeId === n.id;
              const inD = n.in_degree || 0, outD = n.out_degree || 0;
              return `
                <div class="file-item-card" data-id="${escapeHtml(n.id)}" style="background: ${isSel ? '#0284c725' : '#080c14'}; border: 1px solid ${isSel ? '#00e5ff' : '#1e293b'}; border-radius: 4px; padding: 6px 8px; cursor: pointer; display: flex; align-items: center; justify-content: space-between; gap: 6px;">
                  <div style="min-width: 0; flex: 1;">
                    <div style="display: flex; align-items: center; gap: 4px;">
                      <span style="font-size: 11px;">📄</span>
                      <span style="font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: #f1f5f9; text-overflow: ellipsis; overflow: hidden; white-space: nowrap;" title="${escapeHtml(p)}">${escapeHtml(fname)}</span>
                    </div>
                    <div style="font-size: 9px; color: #64748b; margin-top: 2px;">
                      ${n.loc || 0} LOC • ${n.total_symbols || 0} ${t('graph.syms_abbr')} • <span style="color:#38bdf8">${t('graph.in_abbr')}: ${inD}</span> / <span style="color:#10b981">${t('graph.out_abbr')}: ${outD}</span>
                    </div>
                  </div>
                  <button class="btn-card-drilldown" data-id="${escapeHtml(n.id)}" title="${t('graph.drilldown_title')}" style="padding: 2px 6px; font-size: 10px; background: #0284c718; border: 1px solid #0284c740; color: #38bdf8; border-radius: 3px; cursor: pointer; white-space: nowrap;">⚡ ${t('graph.drill_btn')}</button>
                </div>
              `;
            }).join('')}
          </div>
        </div>
      `;
    });

    treeWrap.innerHTML = html;

    treeWrap.querySelectorAll('.file-item-card').forEach(card => {
      card.onclick = (e) => {
        if (e.target.classList.contains('btn-card-drilldown')) return;
        const id = card.getAttribute('data-id');
        activeId = id;
        renderActiveStyles();
        const found = nodes.find(n => n.id === id);
        if (found && onSelectFile) onSelectFile(found, true);
      };
    });

    treeWrap.querySelectorAll('.btn-card-drilldown').forEach(btn => {
      btn.onclick = (e) => {
        e.stopPropagation();
        const id = btn.getAttribute('data-id');
        if (id && onDrilldown) onDrilldown(id);
      };
    });
  }

  function renderActiveStyles() {
    treeWrap.querySelectorAll('.file-item-card').forEach(c => {
      const match = c.getAttribute('data-id') === activeId;
      c.style.borderColor = match ? '#00e5ff' : '#1e293b';
      c.style.background = match ? '#0284c725' : '#080c14';
      if (match) c.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    });
  }

  input.addEventListener('input', (e) => { query = e.target.value; renderList(); });

  container.querySelectorAll('.file-filter-btn').forEach(btn => {
    btn.onclick = () => {
      container.querySelectorAll('.file-filter-btn').forEach(b => {
        b.style.background = 'var(--bg-tertiary)';
        b.style.color = 'var(--text-secondary)';
      });
      btn.style.background = '#0284c7';
      btn.style.color = 'white';
      filterMode = btn.getAttribute('data-mode');
      renderList();
    };
  });

  renderList();

  return {
    setActiveFile: (id) => {
      activeId = id;
      renderActiveStyles();
    }
  };
}
