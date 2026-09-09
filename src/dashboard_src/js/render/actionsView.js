// Rebuilt Actions Telemetry presentation coordinator with universal pagination.
import { el, qsa, setText, emptyState } from '../dom.js';
import { fmtTokens, fmtPct, timeAgo, fmtDurationMs, savingsBadge } from '../format.js';
import { filterRows, makeSortable, bindFilter } from '../interactions.js';
import { paginate, renderPagination, resetPage } from '../pagination.js';
import { store } from '../state.js';
import { t } from '../i18n/index.js';

let activeCategory = 'all';
let activeEff = 'all';
let viewMode = 'aggregated';
let expandedKey = null;

function getToolMeta(name) {
  const TOOL_META = {
    project_context: { cat: 'context', label: t('actions.domain_context'), cls: 'badge-context' },
    task_pipeline: { cat: 'lifecycle', label: t('actions.domain_lifecycle'), cls: 'badge-lifecycle' },
    workflow_gate: { cat: 'governance', label: t('actions.domain_governance'), cls: 'badge-governance' },
    guidance: { cat: 'standards', label: t('actions.domain_standards'), cls: 'badge-standards' },
    select_skills: { cat: 'skills', label: t('actions.domain_skills'), cls: 'badge-skills' },
  };
  return TOOL_META[name] || { cat: 'other', label: t('actions.domain_custom'), cls: 'badge-other' };
}

export function renderActionsView(data) {
  store.tool_breakdown = (data.tool_breakdown || []).map(r => ({
    ...r,
    savings: (r.tok_orig || 0) - (r.tok_opt || 0),
    meta: getToolMeta(r.tool_name),
  }));
  store.recent_actions = (data.recent_actions || []).map(r => ({
    ...r,
    savings: (r.tokens_original || r.tok_orig || 0) - (r.tokens_optimized || r.tok_opt || 0),
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
    drawStreamTable();
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
    const op = r.operation || (r.tool_name === 'select_skills' ? 'load' : 'default');
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
    body.appendChild(tr);

    if (isExp) {
      const expTr = document.createElement('tr');
      expTr.className = 'drilldown-row';
      expTr.innerHTML = `<td colspan="6">${buildDrilldown(r)}</td>`;
      body.appendChild(expTr);
    }
  });

  renderPagination('actions-pagination', 'actions-body', paged, drawBreakdownTable);
}

function buildDrilldown(r) {
  const matches = store.recent_actions.filter(a => a.tool_name === r.tool_name && (a.operation || '') === (r.operation || ''));
  const recent = matches.slice(0, 5);
  const avgSaved = r.cnt ? Math.round(r.savings / r.cnt) : 0;

  let items = recent.map(m => `
    <div class="drilldown-item">
      <span class="time-col">${timeAgo(m.started_at)}</span>
      <span class="font-mono">${fmtDurationMs(m.duration_ms)}</span>
      <span class="tok-stat">${t('actions.tok_stat', { orig: fmtTokens(m.tokens_original), opt: fmtTokens(m.tokens_optimized) })}</span>
      <span class="badge ${m.error_message ? 'red' : 'green'}">${m.error_message ? t('status.error') : t('status.ok')}</span>
    </div>
  `).join('');

  if (!items) items = `<div class="drilldown-empty">${t('actions.drilldown_empty')}</div>`;

  return `
    <div class="drilldown-panel">
      <div class="drilldown-header">
        <strong>${t('actions.drilldown_inspector', { tool: r.tool_name, op: r.operation || 'default' })}</strong>
        <span>${t('actions.drilldown_avg_saved', { saved: fmtTokens(avgSaved) })}</span>
      </div>
      <div class="drilldown-list">${items}</div>
    </div>
  `;
}

function drawStreamTable() {
  const body = el('actions-stream-body');
  if (!body) return;
  const query = el('actions-filter')?.value || '';
  let rows = filterRows(store.recent_actions, query, ['tool_name', 'operation', 'status']);

  if (activeCategory !== 'all') {
    rows = rows.filter(r => r.meta?.cat === activeCategory);
  }
  if (activeEff === 'high') {
    rows = rows.filter(r => (r.tokens_original || 0) > 0 && ((r.savings / r.tokens_original) * 100) >= 50);
  } else if (activeEff === 'low') {
    rows = rows.filter(r => (r.tokens_original || 0) > 0 && ((r.savings / r.tokens_original) * 100) < 50);
  }

  const paged = paginate('actions-stream-body', rows);
  body.innerHTML = '';

  if (!paged.pagedRows.length) {
    emptyState('actions-stream-body', 6, t('actions.empty_stream'));
    renderPagination('actions-stream-pagination', 'actions-stream-body', paged, drawStreamTable);
    return;
  }

  paged.pagedRows.forEach(r => {
    const { pct, badgeClass } = savingsBadge(r.savings, r.tokens_original);
    const op = r.operation || (r.tool_name === 'select_skills' ? 'load' : 'default');
    const isErr = !!r.error_message;
    body.innerHTML += `
      <tr>
        <td class="time-col">${timeAgo(r.started_at)}</td>
        <td>
          <div class="tool-cell">
            <span class="tool-tag ${r.meta.cls}">${r.meta.label}</span>
            <span class="tool-name">${r.tool_name}</span>
          </div>
        </td>
        <td><span class="badge op-badge">${op}</span></td>
        <td class="font-mono">${fmtDurationMs(r.duration_ms)}</td>
        <td><span class="${badgeClass}">${pct}%</span></td>
        <td><span class="badge ${isErr ? 'red' : 'green'}" title="${(r.error_message || '').replace(/"/g, '&quot;')}">${isErr ? t('status.failed') : t('status.success')}</span></td>
      </tr>
    `;
  });

  renderPagination('actions-stream-pagination', 'actions-stream-body', paged, drawStreamTable);
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
  makeSortable('actions-stream-body', store.recent_actions, drawStreamTable);

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
