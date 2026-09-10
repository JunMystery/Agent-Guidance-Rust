import { el, qsa, setText, emptyState } from '../dom.js';
import { fmtTokens, savingsBadge } from '../format.js';
import { filterRows, bindFilter, makeSortable } from '../interactions.js';
import { paginate, renderPagination, resetPage } from '../pagination.js';
import { t } from '../i18n/index.js';
import { drawStreamTable } from './actionsStream.js';

export function getToolMeta(name) {
  const TOOL_META = {
    project_context: { cat: 'context', label: t('actions.domain_context'), cls: 'badge-context' },
    task_pipeline: { cat: 'lifecycle', label: t('actions.domain_lifecycle'), cls: 'badge-lifecycle' },
    workflow_gate: { cat: 'governance', label: t('actions.domain_governance'), cls: 'badge-governance' },
    guidance: { cat: 'standards', label: t('actions.domain_standards'), cls: 'badge-standards' },
    select_skills: { cat: 'skills', label: t('actions.domain_skills'), cls: 'badge-skills' },
    session_continuity: { cat: 'lifecycle', label: t('actions.domain_lifecycle'), cls: 'badge-lifecycle' },
  };
  return TOOL_META[name] || { cat: 'other', label: t('actions.domain_custom'), cls: 'badge-other' };
}

let activeCategory = 'all';
let activeEff = 'all';
let viewMode = 'aggregated';
let expandedKey = null;

let store = {
  tool_breakdown: [],
  recent_actions: [],
};

export function renderActionsView(data) {
  store.tool_breakdown = (data.tool_breakdown || []).map(r => ({
    ...r,
    savings: (r.tokens_original || 0) - (r.tokens_optimized || 0),
    cnt: r.count || 0,
    tok_orig: r.tokens_original || 0,
    tok_opt: r.tokens_optimized || 0,
    meta: getToolMeta(r.tool_name),
  }));

  store.recent_actions = (data.recent_actions || []).map(r => ({
    ...r,
    savings: (r.tokens_original ?? r.tok_orig ?? 0) - (r.tokens_optimized ?? r.tok_opt ?? 0),
    tokens_original: r.tokens_original ?? r.tok_orig ?? 0,
    tokens_optimized: r.tokens_optimized ?? r.tok_opt ?? 0,
    meta: getToolMeta(r.tool_name),
  }));

  drawKpis(data.totals || {}, store.tool_breakdown);
  bindActionsControls();
  refreshCurrentView();
}

function drawKpis(totals, breakdown) {
  const calls = breakdown.reduce((sum, r) => sum + (r.cnt || 0), 0);
  const orig = breakdown.reduce((sum, r) => sum + (r.tok_orig || 0), 0);
  const opt = breakdown.reduce((sum, r) => sum + (r.tok_opt || 0), 0);
  const saved = orig - opt;
  const rate = orig > 0 ? ((saved / orig) * 100).toFixed(1) : '0.0';

  setText('actions-kpi-calls', calls.toLocaleString());
  setText('actions-kpi-tools', t('actions.kpi_active_tools', { count: new Set(breakdown.map(r => r.tool_name)).size }));
  setText('actions-kpi-orig', fmtTokens(orig));
  setText('actions-kpi-opt', fmtTokens(opt));
  setText('actions-kpi-saved', fmtTokens(saved));
  setText('actions-kpi-rate', t('actions.kpi_rate', { rate }));
}

function refreshCurrentView() {
  const isStream = viewMode === 'stream';
  el('actions-table-wrap')?.classList.toggle('hidden', isStream);
  el('actions-stream-wrap')?.classList.toggle('hidden', !isStream);
  if (isStream) {
    drawStreamTable(store, activeCategory, activeEff);
  } else {
    drawBreakdownTable();
  }
}

function drawBreakdownTable() {
  const body = el('actions-body');
  if (!body) return;
  const query = el('actions-filter')?.value || '';
  let rows = filterRows(store.tool_breakdown, query, ['tool_name', 'operation']);

  if (activeCategory !== 'all') {
    rows = rows.filter(r => r.meta.cat === activeCategory);
  }
  if (activeEff === 'high') {
    rows = rows.filter(r => r.tok_orig > 0 && ((r.savings / r.tok_orig) * 100) >= 50);
  } else if (activeEff === 'low') {
    rows = rows.filter(r => r.tok_orig > 0 && ((r.savings / r.tok_orig) * 100) < 50);
  }

  const paged = paginate('actions-body', rows);
  body.innerHTML = '';
  if (!paged.pagedRows.length) {
    emptyState('actions-body', 6, t('actions.empty_breakdown'));
    renderPagination('actions-pagination', 'actions-body', paged, drawBreakdownTable);
    return;
  }

  paged.pagedRows.forEach(r => {
    const rowKey = `${r.tool_name}::${r.operation || ''}`;
    const isExp = expandedKey === rowKey;
    const { pct, badgeClass } = savingsBadge(r.savings, r.tok_orig);
    const op = r.operation || ((r.tool_name === 'select_skills' || r.tool_name === 'select_skill') ? 'load' : 'default');
    const pctNum = Math.min(100, Math.max(0, parseFloat(pct) || 0));

    const tr = document.createElement('tr');
    tr.className = `action-row ${isExp ? 'is-expanded' : ''}`;
    tr.tabIndex = 0;
    tr.setAttribute('aria-expanded', isExp ? 'true' : 'false');
    tr.innerHTML = `
      <td>
        <div class="tool-cell">
          <span class="tool-tag ${r.meta.cls}">${r.meta.label}</span>
          <span class="tool-name">${r.tool_name}</span>
        </div>
      </td>
      <td><span class="badge op-badge">${op}</span></td>
      <td class="num-cell"><strong>${r.cnt}</strong></td>
      <td class="num-cell muted-cell">${fmtTokens(r.tok_orig)}</td>
      <td class="num-cell font-mono">${fmtTokens(r.tok_opt)}</td>
      <td>
        <div class="eff-cell">
          <div class="eff-bar-bg" title="${t('actions.compression_title', { pct })}">
            <div class="eff-bar-fill" style="width:${pctNum}%"></div>
          </div>
          <span class="${badgeClass}">${pct}%</span>
          <button class="btn-inspect" title="${t('actions.inspect_traces_title')}">${isExp ? '▲' : '▼'}</button>
        </div>
      </td>
    `;

    tr.onclick = (e) => {
      e.stopPropagation();
      expandedKey = isExp ? null : rowKey;
      drawBreakdownTable();
    };

    tr.onkeydown = (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        expandedKey = isExp ? null : rowKey;
        drawBreakdownTable();
      }
    };

    body.appendChild(tr);

    if (isExp) {
      const traceMatches = store.recent_actions.filter(a =>
        a.tool_name === r.tool_name && (a.operation || '') === (r.operation || '')
      ).slice(0, 10);

      const expTr = document.createElement('tr');
      expTr.className = 'action-expanded-row';
      const tracesHtml = traceMatches.length
        ? traceMatches.map(tItem => `
            <div class="trace-chip">
              <span class="font-mono">${fmtTokens(tItem.tokens_original)} &rarr; ${fmtTokens(tItem.tokens_optimized)}</span>
              ${tItem.target ? `<span class="target-inline" title="${tItem.target.replace(/"/g, '&quot;')}">${tItem.target.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</span>` : ''}
              <span class="badge ${tItem.error_message ? 'red' : 'green'}">${tItem.error_message ? t('status.failed') : t('status.success')}</span>
            </div>
          `).join('')
        : `<span class="text-muted">${t('actions.no_live_samples')}</span>`;

      expTr.innerHTML = `
        <td colspan="6">
          <div class="expanded-panel">
            <div class="panel-header">
              <strong>${t('actions.traces_breakdown', { tool: r.tool_name, op })}</strong>
            </div>
            <div class="traces-list">${tracesHtml}</div>
          </div>
        </td>
      `;
      body.appendChild(expTr);
    }
  });

  renderPagination('actions-pagination', 'actions-body', paged, drawBreakdownTable);
}

function bindActionsControls() {
  const wrap = el('view-actions');
  if (!wrap || wrap.dataset.bound) return;
  wrap.dataset.bound = 'true';

  bindFilter('actions-filter', () => {
    resetPage('actions-body');
    resetPage('actions-stream-body');
    expandedKey = null;
    refreshCurrentView();
  });

  makeSortable('actions-body', store.tool_breakdown, drawBreakdownTable);
  makeSortable('actions-stream-body', store.recent_actions, () => drawStreamTable(store, activeCategory, activeEff));

  qsa('.actions-chip').forEach(btn => {
    btn.onclick = () => {
      qsa('.actions-chip').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      activeCategory = btn.dataset.cat || 'all';
      resetPage('actions-body');
      resetPage('actions-stream-body');
      expandedKey = null;
      refreshCurrentView();
    };
  });

  qsa('.eff-pill').forEach(btn => {
    btn.onclick = () => {
      qsa('.eff-pill').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      activeEff = btn.dataset.eff || 'all';
      resetPage('actions-body');
      resetPage('actions-stream-body');
      expandedKey = null;
      refreshCurrentView();
    };
  });

  qsa('.view-mode-btn').forEach(btn => {
    btn.onclick = () => {
      qsa('.view-mode-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      viewMode = btn.dataset.mode || 'aggregated';
      resetPage('actions-body');
      resetPage('actions-stream-body');
      expandedKey = null;
      refreshCurrentView();
    };
  });

  el('btn-export-actions')?.addEventListener('click', () => {
    const data = viewMode === 'stream' ? store.recent_actions : store.tool_breakdown;
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `actions-telemetry-${viewMode}-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  });
}
