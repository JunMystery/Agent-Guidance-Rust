// Dashboard MCP System Logs & Diagnostics View
import { el } from '../dom.js';
import { fetchLogs, clearLogs } from '../api.js';
import { showConfirm } from '../dialog.js';
import { t } from '../i18n/index.js';

let currentLevel = 'all';
let currentSearch = '';

export async function renderLogsView() {
  const container = el('view-logs');
  if (!container) return;

  container.innerHTML = `
    <div style="display: flex; flex-direction: column; gap: 16px;">
      <!-- KPI Stats Row -->
      <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px;">
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 12px 16px;">
          <div style="font-size: 11px; color: var(--text-secondary); text-transform: uppercase;">${t('logs.crashes')}</div>
          <div id="stat-crashes" style="font-size: 24px; font-weight: 700; color: #ef4444; margin-top: 4px;">--</div>
          <div style="font-size: 10px; color: var(--text-muted); margin-top: 2px;">${t('logs.crashes_sub')}</div>
        </div>
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 12px 16px;">
          <div style="font-size: 11px; color: var(--text-secondary); text-transform: uppercase;">${t('logs.errors')}</div>
          <div id="stat-errors" style="font-size: 24px; font-weight: 700; color: #f97316; margin-top: 4px;">--</div>
          <div style="font-size: 10px; color: var(--text-muted); margin-top: 2px;">${t('logs.errors_sub')}</div>
        </div>
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 12px 16px;">
          <div style="font-size: 11px; color: var(--text-secondary); text-transform: uppercase;">${t('logs.warnings')}</div>
          <div id="stat-warnings" style="font-size: 24px; font-weight: 700; color: #f59e0b; margin-top: 4px;">--</div>
          <div style="font-size: 10px; color: var(--text-muted); margin-top: 2px;">${t('logs.warnings_sub')}</div>
        </div>
        <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); padding: 12px 16px;">
          <div style="font-size: 11px; color: var(--text-secondary); text-transform: uppercase;">${t('logs.total')}</div>
          <div id="stat-total" style="font-size: 24px; font-weight: 700; color: #00e5ff; margin-top: 4px;">--</div>
          <div style="font-size: 10px; color: var(--text-muted); margin-top: 2px;">${t('logs.total_sub')}</div>
        </div>
      </div>

      <!-- Controls Toolbar -->
      <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;">
        <div style="display: flex; gap: 6px; align-items: center;">
          <button id="btn-log-all" class="btn btn-sm btn-primary" style="padding: 4px 10px; font-size: 11px;">${t('logs.all')}</button>
          <button id="btn-log-crash" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; color: #ef4444;">${t('logs.filter_crashes')}</button>
          <button id="btn-log-error" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; color: #f97316;">${t('logs.filter_errors')}</button>
          <button id="btn-log-warn" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; color: #f59e0b;">${t('logs.filter_warnings')}</button>
        </div>

        <div style="display: flex; gap: 8px; align-items: center; flex: 1; max-width: 400px; justify-content: flex-end;">
          <input id="logs-search-input" type="text" placeholder="${t('logs.search_placeholder')}"
            style="width: 100%; padding: 5px 10px; border-radius: var(--border-radius-sm); background: var(--bg-tertiary); color: var(--text-primary); border: 1px solid var(--border-color); font-size: 11px;" />
          <button id="btn-refresh-logs" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px;">🔄</button>
          <button id="btn-clear-logs" class="btn btn-sm btn-secondary" style="padding: 4px 10px; font-size: 11px; color: #ef4444; border-color: rgba(239, 68, 68, 0.4);" title="${t('logs.clear')}">${t('logs.clear')}</button>
        </div>
      </div>

      <!-- Logs Data Table Container -->
      <div style="background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--border-radius-md); overflow: hidden;">
        <div id="logs-table-body" style="padding: 12px; font-family: var(--font-mono); font-size: 12px;">
          <div style="text-align: center; color: var(--text-secondary); padding: 32px;">${t('logs.loading')}</div>
        </div>
      </div>
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
    const data = await fetchLogs({ level: currentLevel, search: currentSearch, limit: 100 });
    updateStats(data.stats || {});
    renderLogEntries(table, data.logs || []);
  } catch (err) {
    table.innerHTML = `<div style="color: #ef4444; padding: 20px; text-align: center;">${t('logs.load_failed', { error: err.message })}</div>`;
  }
}

function updateStats(stats) {
  if (el('stat-crashes')) el('stat-crashes').textContent = stats.crashes || 0;
  if (el('stat-errors')) el('stat-errors').textContent = stats.errors || 0;
  if (el('stat-warnings')) el('stat-warnings').textContent = stats.warnings || 0;
  if (el('stat-total')) el('stat-total').textContent = stats.total || 0;
}

function renderLogEntries(container, logs) {
  if (!logs.length) {
    container.innerHTML = `
      <div style="text-align: center; color: var(--text-muted); padding: 48px;">
        <div style="font-size: 28px; margin-bottom: 8px;">✨</div>
        <div style="font-size: 13px; color: var(--text-secondary); font-weight: 500;">${t('logs.zero_found')}</div>
        <div style="font-size: 11px; margin-top: 4px;">${t('logs.healthy')}</div>
      </div>
    `;
    return;
  }

  let html = `<div style="display: flex; flex-direction: column; gap: 8px;">`;
  logs.forEach((log) => {
    const isCrash = log.level === 'CRASH';
    const isErr = log.level === 'ERROR';
    const isWarn = log.level === 'WARN';
    const badgeBg = isCrash ? '#ef444422' : (isErr ? '#f9731622' : (isWarn ? '#f59e0b22' : '#00e5ff22'));
    const badgeColor = isCrash ? '#ef4444' : (isErr ? '#f97316' : (isWarn ? '#f59e0b' : '#00e5ff'));
    const icon = isCrash ? '💥' : (isErr ? '❌' : (isWarn ? '⚠️' : 'ℹ️'));

    html += `
      <div style="border: 1px solid var(--border-color); border-radius: var(--border-radius-sm); background: var(--bg-tertiary); padding: 10px 14px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
          <div style="display: flex; gap: 8px; align-items: center;">
            <span style="background: ${badgeBg}; color: ${badgeColor}; border: 1px solid ${badgeColor}55; padding: 2px 7px; border-radius: 4px; font-size: 10px; font-weight: 700;">
              ${icon} ${log.level}
            </span>
            <span style="color: #00e5ff; font-weight: 600; font-size: 11px;">${escapeHtml(log.source)}</span>
            ${log.project_path ? `<span style="color: var(--text-muted); font-size: 10px;">[${escapeHtml(log.project_path)}]</span>` : ''}
          </div>
          <span style="color: var(--text-muted); font-size: 11px;">${log.time_str || ''}</span>
        </div>
        <div style="color: var(--text-primary); font-size: 12px; line-height: 1.5; word-break: break-word;">
          ${escapeHtml(log.message)}
        </div>
        ${log.details ? `
          <details style="margin-top: 8px; border-top: 1px solid var(--border-color); padding-top: 6px;">
            <summary style="font-size: 10px; color: var(--accent); cursor: pointer; user-select: none;">${t('logs.stacktrace')}</summary>
            <pre style="margin-top: 6px; padding: 8px; background: #080c14; border: 1px solid #1e293b; border-radius: 4px; color: #94a3b8; font-size: 10.5px; overflow-x: auto; max-height: 220px; white-space: pre-wrap;">${escapeHtml(log.details)}</pre>
          </details>
        ` : ''}
      </div>
    `;
  });
  html += `</div>`;
  container.innerHTML = html;
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
      loadLogs();
    };
  });
}

function setupSearchAndActions() {
  const searchInput = el('logs-search-input');
  if (searchInput) {
    let debounceTimer;
    searchInput.oninput = (e) => {
      clearTimeout(debounceTimer);
      debounceTimer = setTimeout(() => {
        currentSearch = e.target.value;
        loadLogs();
      }, 250);
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
        await loadLogs();
      }
    };
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
