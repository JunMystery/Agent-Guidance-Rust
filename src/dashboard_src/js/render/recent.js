import { el, emptyState } from '../dom.js';
import { fmtTokens, savingsBadge, fmtDurationMs, timeAgo } from '../format.js';
import { makeSortable, filterRows, bindFilter } from '../interactions.js';
import { paginate, renderPagination, resetPage } from '../pagination.js';
import { store } from '../state.js';

export function renderRecentCalls(recentActions) {
  store.recent_actions = (recentActions || []).map(r => {
    const saved = (r.tokens_original || 0) - (r.tokens_optimized || 0);
    return { ...r, savings: saved, status: r.error_message ? 'error' : 'ok' };
  });
  bindFilter('recent-filter', () => {
    resetPage('recent-calls-body');
    drawRecentCalls();
  });
  makeSortable('recent-calls-body', store.recent_actions, drawRecentCalls);
  drawRecentCalls();
}

function drawRecentCalls() {
  const body = el('recent-calls-body');
  if (!body) return;
  const query = el('recent-filter')?.value || '';
  const rows = filterRows(store.recent_actions, query, ['tool_name', 'operation', 'target', 'status']);
  const paged = paginate('recent-calls-body', rows);
  body.innerHTML = '';
  if (paged.pagedRows.length) {
    paged.pagedRows.forEach(r => {
      const saved = r.savings;
      const { pct, badgeClass } = savingsBadge(saved, r.tokens_original);
      const duration = fmtDurationMs(r.duration_ms);
      const statusClass = r.error_message ? 'badge red' : 'badge green';
      const statusText = r.error_message ? 'error' : 'ok';
      const statusTitle = r.error_message ? ' title="' + r.error_message.replace(/"/g, '&quot;') + '"' : '';
      const opText = r.operation || (r.tool_name === 'select_skills' ? 'load' : 'default');
      const targetText = r.target
        ? '<code class="target-badge" title="' + r.target.replace(/"/g, '&quot;') + '">' + r.target.replace(/</g, '&lt;').replace(/>/g, '&gt;') + '</code>'
        : '<span class="text-muted">—</span>';
      body.innerHTML += '<tr>' +
        '<td>' + timeAgo(r.started_at) + '</td>' +
        '<td><code>' + r.tool_name + '</code></td>' +
        '<td><span class="badge">' + opText + '</span></td>' +
        '<td class="target-cell">' + targetText + '</td>' +
        '<td>' + fmtTokens(r.tokens_original) + '</td>' +
        '<td>' + fmtTokens(r.tokens_optimized) + '</td>' +
        '<td><span class="' + badgeClass + '">' + pct + '%</span></td>' +
        '<td><span class="' + statusClass + '"' + statusTitle + '>' + statusText + '</span></td>' +
        '</tr>';
    });
  } else {
    emptyState('recent-calls-body', 8, 'No matching calls.');
  }
  renderPagination('recent-calls-pagination', 'recent-calls-body', paged, drawRecentCalls);
}
