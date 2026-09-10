// Dashboard MCP System Logs & Diagnostics View
import { el } from '../dom.js';
import { fetchLogs, clearLogs } from '../api.js';
import { showConfirm } from '../dialog.js';
import { paginate, renderPagination, resetPage } from '../pagination.js';
import { t } from '../i18n/index.js';

let currentLevel = 'all';
let currentSearch = '';
let allLogs = [];
let expandedId = null;

export async function renderLogsView() {
  const container = el('view-logs');
  if (!container) return;

  container.innerHTML = `
    <div style="display: flex; flex-direction: column; gap: 10px;">

      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 10px;">
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 8px 12px;">
          <div style="font-size: 10.5px; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.5px;">${t('logs.crashes')}</div>
          <div id="stat-crashes" style="font-size: 18px; font-weight: 700; color: #ef4444; margin-top: 2px;">--</div>
          <div style="font-size: 9.5px; color: var(--text-muted); margin-top: 1px;">${t('logs.crashes_sub')}</div>
        </div>
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 8px 12px;">
          <div style="font-size: 10.5px; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.5px;">${t('logs.errors')}</div>
          <div id="stat-errors" style="font-size: 18px; font-weight: 700; color: #f97316; margin-top: 2px;">--</div>
          <div style="font-size: 9.5px; color: var(--text-muted); margin-top: 1px;">${t('logs.errors_sub')}</div>
        </div>
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 8px 12px;">
          <div style="font-size: 10.5px; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.5px;">${t('logs.warnings')}</div>
          <div id="stat-warnings" style="font-size: 18px; font-weight: 700; color: #f59e0b; margin-top: 2px;">--</div>
          <div style="font-size: 9.5px; color: var(--text-muted); margin-top: 1px;">${t('logs.warnings_sub')}</div>
        </div>
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 8px 12px;">
          <div style="font-size: 10.5px; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.5px;">${t('logs.total')}</div>
          <div id="stat-total" style="font-size: 18px; font-weight: 700; color: #00e5ff; margin-top: 2px;">--</div>
          <div style="font-size: 9.5px; color: var(--text-muted); margin-top: 1px;">${t('logs.total_sub')}</div>
        </div>
      </div>

      <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
        <div style="display: flex; gap: 6px; align-items: center;">
          <button id="btn-log-all" class="btn btn-sm btn-primary" style="padding: 3px 8px; font-size: 11px;">${t('logs.all')}</button>
          <button id="btn-log-crash" class="btn btn-sm btn-secondary" style="padding: 3px 8px; font-size: 11px; color: #ef4444;">${t('logs.filter_crashes')}</button>
          <button id="btn-log-error" class="btn btn-sm btn-secondary" style="padding: 3px 8px; font-size: 11px; color: #f97316;">${t('logs.filter_errors')}</button>
          <button id="btn-log-warn" class="btn btn-sm btn-secondary" style="padding: 3px 8px; font-size: 11px; color: #f59e0b;">${t('logs.filter_warnings')}</button>
        </div>

        <div style="display: flex; gap: 6px; align-items: center; flex: 1; max-width: 460px; justify-content: flex-end;">
          <div style="position: relative; display: flex; align-items: center; width: 100%; max-width: 320px;">
            <span style="position: absolute; left: 8px; font-size: 11px; color: var(--text-muted); pointer-events: none;">🔍</span>
            <input id="logs-search-input" type="text" value="${escapeHtml(currentSearch)}" placeholder="${t('logs.search_placeholder')}"
              style="width: 100%; padding: 4px 24px 4px 26px; border-radius: var(--border-radius-sm); background: var(--bg-tertiary); color: var(--text-primary); border: 1px solid var(--border-color); font-size: 11px; height: 28px;" />
            <button id="btn-search-clear" style="position: absolute; right: 6px; background: transparent; border: none; color: var(--text-muted); cursor: pointer; font-size: 11px; padding: 2px 4px; display: ${currentSearch ? 'block' : 'none'};" title="Clear">✕</button>
          </div>
          <button id="btn-refresh-logs" class="btn btn-sm btn-secondary" style="padding: 3px 8px; font-size: 11px; height: 28px;" title="Refresh">🔄</button>
          <button id="btn-clear-logs" class="btn btn-sm btn-secondary" style="padding: 3px 8px; font-size: 11px; height: 28px; color: #ef4444; border-color: rgba(239, 68, 68, 0.4);" title="${t('logs.clear')}">${t('logs.clear')}</button>
        </div>
      </div>

      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); overflow: hidden;">
        <table class="flat-table" style="width: 100%; margin: 0; font-family: var(--font-mono); font-size: 11px; border-collapse: collapse;">
          <thead>
            <tr style="border-bottom: 1px solid var(--border-color); background: var(--bg-primary);">
              <th scope="col" style="width: 85px; padding: 6px 10px; font-size: 10.5px; text-align: left;">LEVEL</th>
              <th scope="col" style="width: 110px; padding: 6px 10px; font-size: 10.5px; text-align: left;">SOURCE</th>
              <th scope="col" style="padding: 6px 10px; font-size: 10.5px; text-align: left;">MESSAGE</th>
              <th scope="col" style="width: 125px; padding: 6px 10px; font-size: 10.5px; text-align: left;">TIME</th>
            </tr>
          </thead>
          <tbody id="logs-table-body">
            <tr><td colspan="4" style="text-align: center; color: var(--text-secondary); padding: 24px;">${t('logs.loading')}</td></tr>
          </tbody>
        </table>
      </div>
      <div id="logs-pagination"></div>
    </div>
  `;

  setupFilterButtons();
  setupSearchAndActions();
  await loadLogs();
}

async function loadLogs() {
  const table = el('logs-table-body');
  if (!table) return;

  try {
    const data = await fetchLogs({ level: currentLevel, search: currentSearch, limit: 500 });
    updateStats(data.stats || {});
    allLogs = data.logs || [];
    drawLogsTable();
  } catch (err) {
    table.innerHTML = `<tr><td colspan="4" style="color: #ef4444; padding: 20px; text-align: center;">${t('logs.load_failed', { error: err.message })}</td></tr>`;
  }
}

function drawLogsTable() {
  const table = el('logs-table-body');
  if (!table) return;
  const paged = paginate('logs-table', allLogs, 10);
  renderLogEntries(table, paged.pagedRows);
  renderPagination('logs-pagination', 'logs-table', paged, drawLogsTable);
}

function updateStats(stats) {
  if (el('stat-crashes')) el('stat-crashes').textContent = stats.crashes || 0;
  if (el('stat-errors')) el('stat-errors').textContent = stats.errors || 0;
  if (el('stat-warnings')) el('stat-warnings').textContent = stats.warnings || 0;
  if (el('stat-total')) el('stat-total').textContent = stats.total || 0;
}

function renderLogEntries(tbody, logs) {
  if (!logs.length) {
    tbody.innerHTML = `
      <tr>
        <td colspan="4" style="text-align: center; color: var(--text-muted); padding: 28px 16px;">
          <div style="font-size: 20px; margin-bottom: 4px;">✨</div>
          <div style="font-size: 12px; color: var(--text-secondary); font-weight: 500;">${t('logs.zero_found')}</div>
          <div style="font-size: 10px; margin-top: 2px;">${t('logs.healthy')}</div>
        </td>
      </tr>
    `;
    return;
  }

  let html = '';
  logs.forEach((log) => {
    const isCrash = log.level === 'CRASH';
    const isErr = log.level === 'ERROR';
    const isWarn = log.level === 'WARN';
    const badgeBg = isCrash ? '#ef444422' : (isErr ? '#f9731622' : (isWarn ? '#f59e0b22' : '#00e5ff22'));
    const badgeColor = isCrash ? '#ef4444' : (isErr ? '#f97316' : (isWarn ? '#f59e0b' : '#00e5ff'));
    const icon = isCrash ? '💥' : (isErr ? '❌' : (isWarn ? '⚠️' : 'ℹ️'));
    const isExp = expandedId === log.id;

    html += `
      <tr class="log-row" data-id="${log.id}" style="cursor: pointer; border-bottom: 1px solid var(--border-color); height: 32px; background: ${isExp ? 'rgba(0, 229, 255, 0.05)' : 'transparent'};" title="Click to expand details">
        <td style="padding: 5px 10px; white-space: nowrap;">
          <span style="background: ${badgeBg}; color: ${badgeColor}; border: 1px solid ${badgeColor}55; padding: 1px 6px; border-radius: 3px; font-size: 9.5px; font-weight: 700;">
            ${icon} ${log.level}
          </span>
        </td>
        <td style="padding: 5px 10px; color: #00e5ff; font-weight: 600; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 110px;">
          ${escapeHtml(log.source)}
        </td>
        <td style="padding: 5px 10px; max-width: 0; width: 100%;">
          <div style="display: flex; align-items: center; gap: 6px;">
            <span style="color: var(--text-primary); font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
              ${escapeHtml(log.message)}
            </span>
            ${log.project_path ? `<span style="color: var(--text-muted); font-size: 9.5px; white-space: nowrap;">[${escapeHtml(log.project_path)}]</span>` : ''}
            ${log.details ? `<span style="color: var(--accent, #00e5ff); font-size: 9.5px; margin-left: auto; white-space: nowrap;">${isExp ? '▲ less' : '▼ details'}</span>` : ''}
          </div>
        </td>
        <td style="padding: 5px 10px; color: var(--text-muted); font-size: 10.5px; white-space: nowrap;">
          ${log.time_str || ''}
        </td>
      </tr>
    `;

    if (isExp && (log.details || log.message)) {
      html += `
        <tr style="background: #080c14; border-bottom: 1px solid var(--border-color);">
          <td colspan="4" style="padding: 8px 12px;">
            <div style="color: var(--text-primary); font-size: 11.5px; line-height: 1.4; margin-bottom: 6px; word-break: break-word;">
              <strong>${t('logs.message') || 'Message'}:</strong> ${escapeHtml(log.message)}
            </div>
            ${log.project_path ? `<div style="color: var(--text-muted); font-size: 10px; margin-bottom: 6px;"><strong>Project:</strong> ${escapeHtml(log.project_path)}</div>` : ''}
            ${log.details ? `
              <div style="font-size: 10px; color: var(--text-secondary); margin-bottom: 2px;"><strong>${t('logs.stacktrace')}:</strong></div>
              <pre style="margin: 0; padding: 6px 10px; background: #04060a; border: 1px solid #1e293b; border-radius: 4px; color: #94a3b8; font-size: 10px; overflow-x: auto; max-height: 160px; white-space: pre-wrap;">${escapeHtml(log.details)}</pre>
            ` : ''}
          </td>
        </tr>
      `;
    }
  });

  tbody.innerHTML = html;

  tbody.querySelectorAll('.log-row').forEach(row => {
    row.onclick = () => {
      const id = parseInt(row.dataset.id, 10);
      expandedId = expandedId === id ? null : id;
      drawLogsTable();
    };
  });
}

function setupFilterButtons() {
  const map = {
    'btn-log-all': 'all',
    'btn-log-crash': 'crash',
    'btn-log-error': 'error',
    'btn-log-warn': 'warn',
  };
  Object.entries(map).forEach(([id, level]) => {
    const btn = el(id);
    if (!btn) return;
    btn.onclick = () => {
      Object.keys(map).forEach(bId => {
        const b = el(bId);
        if (b) b.className = 'btn btn-sm ' + (bId === id ? 'btn-primary' : 'btn-secondary');
      });
      currentLevel = level;
      resetPage('logs-table');
      loadLogs();
    };
  });
}

function setupSearchAndActions() {
  const searchInput = el('logs-search-input');
  const clearSearchBtn = el('btn-search-clear');

  if (searchInput) {
    let debounceTimer;
    searchInput.oninput = (e) => {
      currentSearch = e.target.value;
      if (clearSearchBtn) clearSearchBtn.style.display = currentSearch ? 'block' : 'none';
      clearTimeout(debounceTimer);
      debounceTimer = setTimeout(() => {
        resetPage('logs-table');
        loadLogs();
      }, 250);
    };

    searchInput.onkeydown = (e) => {
      if (e.key === 'Enter') {
        clearTimeout(debounceTimer);
        currentSearch = searchInput.value;
        if (clearSearchBtn) clearSearchBtn.style.display = currentSearch ? 'block' : 'none';
        resetPage('logs-table');
        loadLogs();
      }
    };
  }

  if (clearSearchBtn) {
    clearSearchBtn.onclick = () => {
      currentSearch = '';
      if (searchInput) searchInput.value = '';
      clearSearchBtn.style.display = 'none';
      resetPage('logs-table');
      loadLogs();
    };
  }

  const refreshBtn = el('btn-refresh-logs');
  if (refreshBtn) refreshBtn.onclick = () => loadLogs();

  const clearBtn = el('btn-clear-logs');
  if (clearBtn) {
    clearBtn.onclick = async () => {
      const ok = await showConfirm({
        message: t('logs.clear_confirm', { level: currentLevel.toUpperCase() }),
      });
      if (ok) {
        await clearLogs(currentLevel);
        resetPage('logs-table');
        await loadLogs();
      }
    };
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
