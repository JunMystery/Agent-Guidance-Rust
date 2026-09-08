import { setText, el, emptyState } from '../dom.js';
import { fmtTokens, fmtPct, timeAgo } from '../format.js';
import { renderHourlyChart } from './chart/index.js';
import { renderRecentCalls } from './recent.js';
import { renderActionsView } from './actionsView.js';
import { makeSortable, filterRows, bindFilter } from '../interactions.js';
import { paginate, renderPagination, resetPage } from '../pagination.js';
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
  setText('sidebar-port-badge', data.server_port || '3000');
  setText('sidebar-version', 'version: v' + (data.version || '--'));
  const projName = data.project_path ? data.project_path.replace(/\\/g, '/').split('/').filter(Boolean).pop() : '--';
  setText('sys-project', projName);
  const sysProjEl = el('sys-project');
  if (sysProjEl) sysProjEl.title = data.project_path || '';
  setText('sys-version', 'v' + (data.version || '--'));
  setText('sys-db-status', data.db_status || '--');

  bindTimeframeTabs();
  updateTimeframeSummary(activeTimeframe);

  renderSkillsTable(data.top_skills);
  renderRecentSkillCalls(data.recent_skill_calls);
  renderActionsView(data);
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
  setText('out-embed-metrics-queries', s.embed_queries || 0);

  setText('out-original-tokens', fmtTokens(s.tokens_original));
  setText('out-optimized-tokens', fmtTokens(s.tokens_optimized));
  setText('out-token-savings', fmtTokens(s.token_savings));
  setText('out-savings-pct', '+' + fmtPct(s.savings_pct));

  if (data.top_skills) {
    setText('out-skills-catalog', String(data.top_skills.length));
  }
}

function renderSkillsTable(topSkills) {
  store.top_skills = topSkills || [];
  bindFilter('skills-filter', () => {
    resetPage('dash-skills');
    drawSkillsTable();
  });
  makeSortable('dash-skills', store.top_skills, drawSkillsTable);
  drawSkillsTable();
}

function drawSkillsTable() {
  const body = el('dash-skills');
  if (!body) return;
  const query = el('skills-filter')?.value || '';
  const rows = filterRows(store.top_skills, query, ['skill_id']);
  const paged = paginate('dash-skills', rows);
  body.innerHTML = '';
  if (paged.pagedRows.length) {
    paged.pagedRows.forEach((sk, idx) => {
      const globalIdx = paged.startIdx + idx;
      const rankBadge = globalIdx < 3 ? 'badge green' : 'badge';
      body.innerHTML += '<tr>' +
        '<td><span class="' + rankBadge + '">#' + (globalIdx + 1) + '</span></td>' +
        '<td><strong>' + sk.skill_id + '</strong></td>' +
        '<td>' + sk.cnt + '</td>' +
        '</tr>';
    });
  } else {
    emptyState('dash-skills', 3, 'No matching skills loaded.');
  }
  renderPagination('dash-skills-pagination', 'dash-skills', paged, drawSkillsTable);
}

function renderRecentSkillCalls(recentSkillCalls) {
  store.recent_skill_calls = recentSkillCalls || [];
  makeSortable('recent-skills-body', store.recent_skill_calls, drawRecentSkillCalls);
  drawRecentSkillCalls();
}

function drawRecentSkillCalls() {
  const body = el('recent-skills-body');
  if (!body) return;
  const paged = paginate('recent-skills-body', store.recent_skill_calls);
  body.innerHTML = '';
  if (paged.pagedRows.length) {
    paged.pagedRows.forEach(sk => {
      body.innerHTML += '<tr>' +
        '<td>' + timeAgo(sk.loaded_at) + '</td>' +
        '<td><strong>' + sk.skill_id + '</strong></td>' +
        '</tr>';
    });
  } else {
    emptyState('recent-skills-body', 2, 'No skill activations recorded yet.');
  }
  renderPagination('recent-skills-pagination', 'recent-skills-body', paged, drawRecentSkillCalls);
}
