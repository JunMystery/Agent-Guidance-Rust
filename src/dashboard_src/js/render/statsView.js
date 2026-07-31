import { setText, el, emptyState } from '../dom.js';
import { fmtTokens, fmtPct, timeAgo, savingsBadge } from '../format.js';
import { renderHourlyChart } from './chart/index.js';
import { renderRecentCalls } from './recent.js';
import { makeSortable, filterRows, bindFilter } from '../interactions.js';
import { store } from '../state.js';

let activeTimeframe = 'past_24h';

export function renderDashboard(data) {
  store.dashboard_data = data;
  
  setText('out-client-name', 'global');
  setText('out-session-label', 'per-call tracking');

  setText('sidebar-proj', 'project: ' + (data.project_path ? data.project_path.split('/').pop() : '--'));
  const projEl = el('sidebar-proj');
  if (projEl) projEl.title = data.project_path || '';
  setText('sidebar-port', 'port: ' + (data.server_port || '--'));
  setText('sidebar-version', 'version: v' + (data.version || '--'));
  setText('sys-project', data.project_path || '--');
  setText('sys-version', 'v' + (data.version || '--'));
  setText('sys-db-status', data.db_status || '--');

  bindTimeframeTabs();
  updateTimeframeSummary(activeTimeframe);

  renderSkillsTable(data.top_skills);
  renderRecentSkillCalls(data.recent_skill_calls);
  renderActionsTable(data.tool_breakdown);
  renderHourlyChart(data, data.totals || {});
  renderRecentCalls(data.recent_actions);
}

function bindTimeframeTabs() {
  const container = el('timeframe-selector');
  if (!container || container.dataset.bound) return;
  container.dataset.bound = 'true';

  const tabs = container.querySelectorAll('.timeframe-tab');
  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      activeTimeframe = tab.dataset.timeframe;
      updateTimeframeSummary(activeTimeframe);
    });
  });
}

function updateTimeframeSummary(tf) {
  const data = store.dashboard_data;
  if (!data) return;
  const s = (data.summaries && data.summaries[tf]) || data.totals || {};

  setText('out-tool-calls', s.tool_calls || 0);
  setText('out-skills-loaded', s.skills_loaded || 0);
  setText('out-embed-queries', s.embed_queries || 0);

  setText('out-original-tokens', fmtTokens(s.tokens_original));
  setText('out-optimized-tokens', fmtTokens(s.tokens_optimized));
  setText('out-token-savings', fmtTokens(s.token_savings) + ' (' + fmtPct(s.savings_pct) + ' savings)');
}

function renderSkillsTable(topSkills) {
  store.top_skills = topSkills || [];
  bindFilter('skills-filter', () => drawSkillsTable());
  makeSortable('dash-skills', store.top_skills);
  drawSkillsTable();
}

function drawSkillsTable() {
  const body = el('dash-skills');
  if (!body) return;
  const query = el('skills-filter')?.value || '';
  const rows = filterRows(store.top_skills, query, ['skill_id']);
  body.innerHTML = '';
  if (rows.length) {
    rows.forEach((sk, idx) => {
      const rankBadge = idx < 3 ? 'badge green' : 'badge';
      body.innerHTML += '<tr>' +
        '<td><span class="' + rankBadge + '">#' + (idx + 1) + '</span></td>' +
        '<td><strong>' + sk.skill_id + '</strong></td>' +
        '<td>' + sk.cnt + '</td>' +
        '</tr>';
    });
  } else {
    emptyState('dash-skills', 3, 'No matching skills loaded.');
  }
}

function renderRecentSkillCalls(recentSkillCalls) {
  const body = el('recent-skills-body');
  if (!body) return;
  body.innerHTML = '';
  if (recentSkillCalls && recentSkillCalls.length) {
    recentSkillCalls.slice(0, 10).forEach(sk => {
      body.innerHTML += '<tr>' +
        '<td>' + timeAgo(sk.loaded_at) + '</td>' +
        '<td><strong>' + sk.skill_id + '</strong></td>' +
        '</tr>';
    });
  } else {
    emptyState('recent-skills-body', 2, 'No skill activations recorded yet.');
  }
}

function renderActionsTable(toolBreakdown) {
  store.tool_breakdown = (toolBreakdown || []).map(r => {
    const saved = (r.tok_orig || 0) - (r.tok_opt || 0);
    return { ...r, savings: saved, status: r.error_message ? 'error' : 'ok' };
  });
  bindFilter('actions-filter', () => drawActionsTable());
  makeSortable('actions-body', store.tool_breakdown);
  drawActionsTable();
}

function drawActionsTable() {
  const body = el('actions-body');
  if (!body) return;
  const query = el('actions-filter')?.value || '';
  const rows = filterRows(store.tool_breakdown, query, ['tool_name', 'operation']);
  body.innerHTML = '';
  if (rows.length) {
    rows.forEach(r => {
      const saved = r.savings;
      const { pct, badgeClass } = savingsBadge(saved, r.tok_orig);
      body.innerHTML += '<tr><td>' + r.tool_name + '</td><td>' + (r.operation || '--') + '</td><td>' + r.cnt + '</td><td>' + fmtTokens(r.tok_orig) + '</td><td>' + fmtTokens(r.tok_opt) + '</td><td><span class="' + badgeClass + '">' + pct + '%</span></td></tr>';
    });
  } else {
    emptyState('actions-body', 6, 'No matching tool calls.');
  }
}


