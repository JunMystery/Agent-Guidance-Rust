import { el, emptyState } from '../dom.js';
import { timeAgo, savingsBadge } from '../format.js';
import { filterRows } from '../interactions.js';
import { paginate, renderPagination } from '../pagination.js';
import { t } from '../i18n/index.js';

export function drawStreamTable(store, activeCategory, activeEff) {
  const body = el('actions-stream-body');
  if (!body) return;
  const query = el('actions-filter')?.value || '';
  let rows = filterRows(store.recent_actions, query, ['tool_name', 'operation', 'target']);

  if (activeCategory !== 'all') {
    rows = rows.filter(r => r.meta.cat === activeCategory);
  }
  if (activeEff === 'high') {
    rows = rows.filter(r => r.tokens_original > 0 && ((r.savings / r.tokens_original) * 100) >= 50);
  } else if (activeEff === 'low') {
    rows = rows.filter(r => r.tokens_original > 0 && ((r.savings / r.tokens_original) * 100) < 50);
  }

  const paged = paginate('actions-stream-body', rows);
  body.innerHTML = '';

  if (!paged.pagedRows.length) {
    emptyState('actions-stream-body', 6, t('actions.empty_stream'));
    renderPagination('actions-stream-pagination', 'actions-stream-body', paged, () => drawStreamTable(store, activeCategory, activeEff));
    return;
  }

  paged.pagedRows.forEach(r => {
    const { pct, badgeClass } = savingsBadge(r.savings, r.tokens_original);
    const op = r.operation || ((r.tool_name === 'select_skills' || r.tool_name === 'select_skill') ? 'load' : 'default');
    const isErr = !!r.error_message;
    const targetText = r.target
      ? `<code class="target-badge" title="${r.target.replace(/"/g, '&quot;')}">${r.target.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</code>`
      : '<span class="text-muted">—</span>';
    body.innerHTML += `
      <tr>
        <td class="time-col font-mono">${timeAgo(r.started_at)}</td>
        <td>
          <div class="tool-cell">
            <span class="tool-tag ${r.meta.cls}">${r.meta.label}</span>
            <span class="tool-name">${r.tool_name}</span>
          </div>
        </td>
        <td><span class="badge op-badge">${op}</span></td>
        <td class="target-cell">${targetText}</td>
        <td class="num-cell"><span class="${badgeClass}">${pct}%</span></td>
        <td><span class="badge ${isErr ? 'red' : 'green'}" title="${(r.error_message || '').replace(/"/g, '&quot;')}">${isErr ? t('status.failed') : t('status.success')}</span></td>
      </tr>
    `;
  });

  renderPagination('actions-stream-pagination', 'actions-stream-body', paged, () => drawStreamTable(store, activeCategory, activeEff));
}
