import { qsa, el } from './dom.js';
import { fetchData, refreshEmbedStatus, fetchProjects, pruneProjects, setSelectedProject, triggerAutoCleanup, fetchGraphData } from './api.js';
import { renderGraphView } from './render/graphView.js';
import { renderLogsView } from './render/logsView.js';
import { startPoll, stopPoll } from './poll.js';
import { showConfirm, showAlert } from './dialog.js';
import { changeProjectPath, closeDirBrowser, selectDirPath } from './dirBrowser.js';

const VIEWS = ['dashboard', 'top-skills', 'actions', 'recent-calls', 'graph', 'logs'];

export async function loadAndRenderGraph() {
  const data = await fetchGraphData();
  renderGraphView(data);
}

export async function loadAndRenderLogs() {
  await renderLogsView();
}

function syncView(view, { push = true } = {}) {
  if (!VIEWS.includes(view)) view = 'dashboard';
  qsa('.sidebar nav a').forEach(x => {
    const isActive = x.dataset.view === view;
    x.classList.toggle('active', isActive);
    x.setAttribute('aria-selected', isActive ? 'true' : 'false');
    x.tabIndex = isActive ? 0 : -1;
  });
  qsa('main section').forEach(s => s.classList.add('hidden'));
  const section = el('view-' + view);
  if (section) {
    section.classList.remove('hidden');
    section.tabIndex = -1;
  }
  if (push && location.hash.slice(1) !== view) {
    history.replaceState(null, '', '#' + view);
  }
  if (view === 'graph') {
    stopPoll();
    loadAndRenderGraph();
  } else if (view === 'logs') {
    stopPoll();
    loadAndRenderLogs();
  } else {
    startPoll();
  }
}

function initA11y() {
  qsa('table th').forEach(th => { if (!th.hasAttribute('scope')) th.setAttribute('scope', 'col'); });
  const tabs = Array.from(qsa('.sidebar nav a'));
  tabs.forEach((tab, i) => {
    tab.addEventListener('keydown', (e) => {
      let next = null;
      if (e.key === 'ArrowDown' || e.key === 'ArrowRight') next = tabs[(i + 1) % tabs.length];
      else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') next = tabs[(i - 1 + tabs.length) % tabs.length];
      if (next) { e.preventDefault(); next.focus(); }
    });
  });
}

function onViewClick(a, e) {
  e.preventDefault();
  syncView(a.dataset.view);
}

function initRouter() {
  qsa('.sidebar nav a').forEach(a => {
    a.addEventListener('click', e => onViewClick(a, e));
  });
  window.addEventListener('hashchange', () => syncView(location.hash.slice(1), { push: false }));
  syncView(location.hash.slice(1) || 'dashboard');
  fetchData();
}

export function toggleSidebar() {
  const open = el('sidebar').classList.toggle('open');
  const btn = document.querySelector('.hamburger');
  if (btn) btn.setAttribute('aria-expanded', open ? 'true' : 'false');
}

async function initProjectSelector() {
  const select = el('project-selector');
  const headerSelect = el('header-project-selector');
  const badge = el('project-status-badge');
  const headerBadge = el('header-project-badge');
  const pruneBtn = el('btn-prune-projects');
  if (!select) return;

  const projects = await fetchProjects();
  const optionsHtml = ['<option value="all">🌐 All Projects (Global Analytics)</option>'];
  const seenPaths = new Set();

  projects.forEach(p => {
    let norm = (p.path || '').trim().replace(/\//g, '\\');
    while (norm.length > 3 && norm.endsWith('\\')) norm = norm.slice(0, -1);
    const key = norm.toLowerCase();
    if (seenPaths.has(key)) return;
    seenPaths.add(key);

    const statusIcon = p.status === 'active' ? '🟢' : '⚠️ [Missing]';
    optionsHtml.push(`<option value="${p.path}">${statusIcon} ${p.name} (${p.path})</option>`);
  });

  const fullHtml = optionsHtml.join('');
  select.innerHTML = fullHtml;
  if (headerSelect) headerSelect.innerHTML = fullHtml;

  const onProjectChange = (val) => {
    select.value = val;
    if (headerSelect) headerSelect.value = val;
    setSelectedProject(val);

    const chosen = projects.find(p => p.path === val);
    const isMissing = chosen && chosen.status === 'missing';
    const msg = isMissing ? '⚠️ Missing on disk (Showing historical records)' : (val === 'all' ? '🌐 All Tracked Projects' : `📂 Active: ${chosen?.name || val}`);

    if (badge) {
      badge.textContent = isMissing ? '⚠️ Missing on disk' : '';
      badge.style.display = isMissing ? 'block' : 'none';
      badge.style.color = '#ffbd2e';
    }
    if (headerBadge) {
      headerBadge.textContent = msg;
      headerBadge.style.display = 'inline-block';
      headerBadge.style.color = isMissing ? '#ffbd2e' : 'var(--accent-primary)';
    }

    fetchData();
    if (location.hash.slice(1) === 'graph') {
      loadAndRenderGraph();
    }
  };

  select.addEventListener('change', () => onProjectChange(select.value));
  if (headerSelect) {
    headerSelect.addEventListener('change', () => onProjectChange(headerSelect.value));
  }

  const defaultProj = el('sidebar-proj')?.textContent?.trim();
  if (defaultProj && defaultProj !== '--') {
    const match = projects.find(p => p.path === defaultProj || p.name === defaultProj);
    if (match) {
      select.value = match.path;
      if (headerSelect) headerSelect.value = match.path;
      setSelectedProject(match.path);
    }
  }

  if (pruneBtn) {
    pruneBtn.addEventListener('click', async () => {
      const ok = await showConfirm({
        title: 'Prune Missing Projects',
        message: 'Remove missing/deleted projects from the tracking registry?',
        subtext: 'This removes project records whose paths no longer exist on disk.',
        confirmText: 'Prune Projects',
        cancelText: 'Cancel',
        variant: 'danger',
      });
      if (ok) {
        const res = await pruneProjects();
        await showAlert({
          title: 'Pruning Complete',
          message: res.message || `Pruned ${res.pruned_count || 0} projects.`,
          variant: 'success',
        });
        await initProjectSelector();
        fetchData();
      }
    });
  }

  const cleanupBtn = el('btn-auto-cleanup');
  if (cleanupBtn) {
    cleanupBtn.addEventListener('click', async () => {
      const ok = await showConfirm({
        title: 'Optimize Database',
        message: 'Run automated log cleanup and vacuum SQLite database?',
        subtext: 'Prunes expired tool calls, vacuum tables, and reclaims disk space.',
        confirmText: '⚡ Optimize Now',
        cancelText: 'Cancel',
        variant: 'info',
      });
      if (!ok) return;

      const res = await triggerAutoCleanup();
      await showAlert({
        title: 'Database Optimized',
        message: res.message || 'Cleanup complete.',
        variant: res.success ? 'success' : 'danger',
      });
      fetchData();
    });
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initA11y();
  initRouter();
  initProjectSelector();
});

window.toggleSidebar = toggleSidebar;
window.refreshEmbedStatus = refreshEmbedStatus;
window.changeProjectPath = changeProjectPath;
window.closeDirBrowser = closeDirBrowser;
window.selectDirPath = selectDirPath;
