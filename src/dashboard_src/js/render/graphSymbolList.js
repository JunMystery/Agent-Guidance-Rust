import { t } from '../i18n/index.js';

// Hierarchical File -> Function Symbol List Component (Parallel Graph Navigator)

export function initGraphSymbolList(container, nodes, onSelectNode) {
  if (!container) return;

  let query = '';
  let kindFilter = 'all';
  let activeId = null;

  container.innerHTML = `
    <div style="margin-top: 20px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 14px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; flex-wrap: wrap; gap: 8px;">
        <div style="display: flex; align-items: center; gap: 8px;">
          <span style="font-size: 13px; font-weight: 600; color: #f8fafc;">${t('symbol_list.title')}</span>
          <span id="symbol-list-count" style="font-size: 11px; background: #0284c720; color: #38bdf8; border: 1px solid #0284c740; padding: 2px 8px; border-radius: 12px; font-family: var(--font-mono); font-weight: 600;">
            ${t('symbol_list.symbols_count', { count: nodes.length })}
          </span>
        </div>
        <div style="display: flex; gap: 6px; align-items: center;">
          <button class="list-filter-btn active" data-kind="all" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: #0284c7; color: white; cursor: pointer;">${t('symbol_list.all')}</button>
          <button class="list-filter-btn" data-kind="function" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;">${t('symbol_list.fn')}</button>
          <button class="list-filter-btn" data-kind="struct" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;">${t('symbol_list.structs')}</button>
          <button class="list-filter-btn" data-kind="hubs" style="padding: 3px 8px; font-size: 11px; border-radius: 4px; border: 1px solid var(--border-color); background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;">${t('symbol_list.hubs')}</button>
        </div>
      </div>

      <div style="position: relative; margin-bottom: 12px;">
        <input type="text" id="symbol-search-input" placeholder="${t('symbol_list.search_placeholder')}" style="width: 100%; box-sizing: border-box; padding: 8px 12px; background: #080c14; border: 1px solid #334155; border-radius: 6px; color: #f1f5f9; font-size: 12px; font-family: var(--font-mono); outline: none;">
      </div>

      <div id="symbol-tree-wrap" style="max-height: 420px; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; padding-right: 4px;">
      </div>
    </div>
  `;

  const input = container.querySelector('#symbol-search-input');
  const treeWrap = container.querySelector('#symbol-tree-wrap');
  const countBadge = container.querySelector('#symbol-list-count');

  function renderList() {
    const rawQ = query.trim().toLowerCase();
    let fileQuery = '';
    let funcQuery = '';

    if (rawQ.includes('->')) {
      const parts = rawQ.split('->');
      fileQuery = parts[0].trim();
      funcQuery = parts[1].trim();
    } else if (rawQ.includes(' ')) {
      const parts = rawQ.split(/\s+/);
      fileQuery = parts[0].trim();
      funcQuery = parts.slice(1).join(' ').trim();
    } else {
      fileQuery = rawQ;
      funcQuery = rawQ;
    }

    // Filter nodes
    const matchedNodes = nodes.filter(n => {
      if (kindFilter === 'hubs' && !n.isHub) return false;
      if (kindFilter !== 'all' && kindFilter !== 'hubs' && n.kind !== kindFilter) return false;

      if (!rawQ) return true;

      const f = (n.file || '').toLowerCase();
      const l = (n.label || '').toLowerCase();
      const lang = (n.lang || '').toLowerCase();

      if (rawQ.includes('->') || rawQ.includes(' ')) {
        return f.includes(fileQuery) && l.includes(funcQuery);
      }
      return f.includes(rawQ) || l.includes(rawQ) || lang.includes(rawQ);
    });

    if (countBadge) {
      countBadge.textContent = t('symbol_list.count_badge', {
        count: matchedNodes.length,
        files: new Set(matchedNodes.map(n => n.file)).size
      });
    }

    if (!matchedNodes.length) {
      treeWrap.innerHTML = `
        <div style="padding: 24px; text-align: center; color: var(--text-secondary); font-size: 12px;">
          ${t('symbol_list.no_match', { query: escapeHtml(rawQ) })}
        </div>
      `;
      return;
    }

    // Group by file
    const groups = new Map();
    matchedNodes.forEach(n => {
      const f = n.file || 'unknown';
      if (!groups.has(f)) groups.set(f, []);
      groups.get(f).push(n);
    });

    let html = '';
    groups.forEach((fileNodes, file) => {
      const displayFile = file.replace(/\\/g, '/');
      html += `
        <div style="background: #0f172a; border: 1px solid #1e293b; border-radius: 6px; overflow: hidden; flex-shrink: 0; min-height: fit-content;">
          <div style="background: #1e293b; padding: 6px 10px; display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #334155;">
            <div style="display: flex; align-items: center; gap: 6px;">
              <span style="font-size: 13px;">📁</span>
              <span style="font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: #38bdf8;">${escapeHtml(displayFile)}</span>
            </div>
            <span style="font-size: 10px; color: #94a3b8; background: #0f172a; padding: 1px 6px; border-radius: 4px;">${t('symbol_list.symbols_count', { count: fileNodes.length })}</span>
          </div>
          <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 4px; padding: 6px;">
            ${fileNodes.map(n => renderSymbolCard(n, activeId === n.id)).join('')}
          </div>
        </div>
      `;
    });

    treeWrap.innerHTML = html;

    // Attach click handlers
    treeWrap.querySelectorAll('.symbol-item-card').forEach(card => {
      card.onclick = () => {
        const id = card.getAttribute('data-id');
        activeId = id;
        treeWrap.querySelectorAll('.symbol-item-card').forEach(c => {
          c.style.borderColor = c.getAttribute('data-id') === id ? '#00e5ff' : '#1e293b';
          c.style.background = c.getAttribute('data-id') === id ? '#0284c725' : '#080c14';
        });
        const selectedNode = nodes.find(n => n.id === id);
        if (selectedNode && onSelectNode) {
          onSelectNode(selectedNode);
        }
      };
    });
  }

  function renderSymbolCard(n, isSelected) {
    const kindColors = { function: '#10b981', struct: '#00e5ff', module: '#8b5cf6', trait: '#ec4899', enum: '#f59e0b' };
    const color = kindColors[n.kind] || '#94a3b8';
    return `
      <div class="symbol-item-card" data-id="${escapeHtml(n.id)}" style="background: ${isSelected ? '#0284c725' : '#080c14'}; border: 1px solid ${isSelected ? '#00e5ff' : '#1e293b'}; border-radius: 4px; padding: 6px 8px; cursor: pointer; display: flex; align-items: center; justify-content: space-between; gap: 6px; transition: all 0.15s ease;" onmouseover="if(this.getAttribute('data-id')!=='${activeId}')this.style.borderColor='#38bdf8'" onmouseout="if(this.getAttribute('data-id')!=='${activeId}')this.style.borderColor='#1e293b'">
        <div style="min-width: 0; flex: 1;">
          <div style="display: flex; align-items: center; gap: 4px; margin-bottom: 2px;">
            <span style="font-size: 9px; font-weight: 700; color: ${color}; background: ${color}15; border: 1px solid ${color}35; padding: 1px 4px; border-radius: 3px;">${n.kind || 'sym'}</span>
            ${n.lang ? `<span style="font-size: 8px; font-weight: 700; color: #38bdf8; background: #0284c718; border: 1px solid #0284c730; padding: 1px 3px; border-radius: 2px;">${n.lang}</span>` : ''}
            <span style="font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: #f1f5f9; text-overflow: ellipsis; overflow: hidden; white-space: nowrap;">${escapeHtml(n.label)}</span>
          </div>
          <div style="font-size: 9px; color: #64748b;">
            ${t('symbol_list.meta', { loc: n.loc || '--', deg: n.deg || 0, inDeg: n.inDeg || 0, outDeg: n.outDeg || 0 })}
          </div>
        </div>
        ${n.isHub ? `<span style="font-size: 9px; color: #f59e0b; background: #f59e0b20; border: 1px solid #f59e0b40; padding: 1px 4px; border-radius: 3px; white-space: nowrap; font-weight: 600;">${t('symbol_list.hub_badge')}</span>` : ''}
      </div>
    `;
  }

  input.addEventListener('input', (e) => {
    query = e.target.value;
    renderList();
  });

  container.querySelectorAll('.list-filter-btn').forEach(btn => {
    btn.onclick = () => {
      container.querySelectorAll('.list-filter-btn').forEach(b => {
        b.style.background = 'var(--bg-tertiary)';
        b.style.color = 'var(--text-secondary)';
      });
      btn.style.background = '#0284c7';
      btn.style.color = 'white';
      kindFilter = btn.getAttribute('data-kind');
      renderList();
    };
  });

  renderList();

  return {
    setActiveNode: (id) => {
      activeId = id;
      treeWrap.querySelectorAll('.symbol-item-card').forEach(c => {
        const match = c.getAttribute('data-id') === id;
        c.style.borderColor = match ? '#00e5ff' : '#1e293b';
        c.style.background = match ? '#0284c725' : '#080c14';
        if (match) c.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
    }
  };
}

function escapeHtml(str) {
  return String(str || '').replace(/[&<>"']/g, m => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[m]));
}
