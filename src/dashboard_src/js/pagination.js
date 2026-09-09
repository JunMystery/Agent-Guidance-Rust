// Universal Pagination controller and UI renderer for dashboard tables.
import { el } from './dom.js';
import { t } from './i18n/index.js';

const paginationStates = {};

export function getPaginationState(tableId) {
  if (!paginationStates[tableId]) {
    paginationStates[tableId] = { page: 1, pageSize: 10 };
  }
  return paginationStates[tableId];
}

export function resetPage(tableId) {
  const state = getPaginationState(tableId);
  state.page = 1;
}

export function paginate(tableId, allRows) {
  const state = getPaginationState(tableId);
  const totalItems = allRows.length;
  const totalPages = Math.max(1, Math.ceil(totalItems / state.pageSize));

  if (state.page > totalPages) state.page = totalPages;
  if (state.page < 1) state.page = 1;

  const startIdx = (state.page - 1) * state.pageSize;
  const endIdx = Math.min(startIdx + state.pageSize, totalItems);
  const pagedRows = allRows.slice(startIdx, endIdx);

  return {
    pagedRows,
    page: state.page,
    pageSize: state.pageSize,
    totalPages,
    totalItems,
    startIdx,
    endIdx,
  };
}

export function renderPagination(containerId, tableId, paged, onPageChange) {
  const container = el(containerId);
  if (!container) return;

  const state = getPaginationState(tableId);
  const startNum = paged.totalItems === 0 ? 0 : paged.startIdx + 1;

  container.innerHTML = `
    <div class="pagination-bar" role="navigation" aria-label="${t('pagination.nav_aria', { table: tableId })}">
      <div class="pagination-info">
        ${t('pagination.showing', { start: startNum, end: paged.endIdx, total: paged.totalItems })}
      </div>
      <div class="pagination-controls">
        <div class="pagination-size">
          <label for="pagesize-${tableId}">${t('pagination.show')}</label>
          <select id="pagesize-${tableId}" class="pagination-select" aria-label="${t('pagination.rows_per_page')}">
            <option value="10" ${state.pageSize === 10 ? 'selected' : ''}>10</option>
            <option value="25" ${state.pageSize === 25 ? 'selected' : ''}>25</option>
            <option value="50" ${state.pageSize === 50 ? 'selected' : ''}>50</option>
            <option value="100" ${state.pageSize === 100 ? 'selected' : ''}>100</option>
          </select>
        </div>
        <div class="pagination-nav">
          <button class="pagination-btn pg-first" title="${t('pagination.first_page')}" ${paged.page <= 1 ? 'disabled' : ''}>⏮</button>
          <button class="pagination-btn pg-prev" title="${t('pagination.prev_page')}" ${paged.page <= 1 ? 'disabled' : ''}>◀</button>
          <span class="pagination-status">${t('pagination.page_status', { page: paged.page, totalPages: paged.totalPages })}</span>
          <button class="pagination-btn pg-next" title="${t('pagination.next_page')}" ${paged.page >= paged.totalPages ? 'disabled' : ''}>▶</button>
          <button class="pagination-btn pg-last" title="${t('pagination.last_page')}" ${paged.page >= paged.totalPages ? 'disabled' : ''}>⏭</button>
        </div>
      </div>
    </div>
  `;

  const sel = container.querySelector(`#pagesize-${tableId}`);
  if (sel) {
    sel.onchange = () => {
      state.pageSize = parseInt(sel.value, 10) || 10;
      state.page = 1;
      onPageChange();
    };
  }

  const btnFirst = container.querySelector('.pg-first');
  const btnPrev = container.querySelector('.pg-prev');
  const btnNext = container.querySelector('.pg-next');
  const btnLast = container.querySelector('.pg-last');

  if (btnFirst) btnFirst.onclick = () => { if (state.page > 1) { state.page = 1; onPageChange(); } };
  if (btnPrev) btnPrev.onclick = () => { if (state.page > 1) { state.page--; onPageChange(); } };
  if (btnNext) btnNext.onclick = () => { if (state.page < paged.totalPages) { state.page++; onPageChange(); } };
  if (btnLast) btnLast.onclick = () => { if (state.page < paged.totalPages) { state.page = paged.totalPages; onPageChange(); } };
}
