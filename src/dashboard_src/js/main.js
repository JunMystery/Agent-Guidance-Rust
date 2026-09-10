import { qsa, el } from './dom.js';
import { fetchData, refreshEmbedStatus, fetchProjects, pruneProjects, setSelectedProject, triggerAutoCleanup, fetchGraphData } from './api.js';
import { renderGraphView } from './render/graphView.js';
import { renderLogsView } from './render/logsView.js';
import { startPoll, stopPoll } from './poll.js';
import { showConfirm, showAlert } from './dialog.js';
import { changeProjectPath, closeDirBrowser, selectDirPath } from './dirBrowser.js';
import { initI18n, getLanguage, setLanguage, onLanguageChange, t } from './i18n/index.js';

const VIEWS = ['dashboard', 'actions', 'graph', 'logs'];

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

export function toggleSidebarCollapse(force) {
  const isMobile = window.innerWidth <= 768;
  if (isMobile) {
    const sidebar = el('sidebar');
    if (!sidebar) return;
    const open = force !== undefined ? !force : !sidebar.classList.contains('open');
    sidebar.classList.toggle('open', open);
    const toggleBtn = el('sidebar-toggle-btn') || document.querySelector('.hamburger');
    if (toggleBtn) toggleBtn.setAttribute('aria-expanded', open ? 'true' : 'false');
  } else {
    const isCollapsed = force !== undefined ? force : !document.body.classList.contains('sidebar-collapsed');
    document.body.classList.toggle('sidebar-collapsed', isCollapsed);
    try {
      localStorage.setItem('agy_sidebar_collapsed', isCollapsed ? 'true' : 'false');
    } catch (e) {}
    const toggleBtn = el('sidebar-toggle-btn');
    if (toggleBtn) toggleBtn.setAttribute('aria-expanded', !isCollapsed ? 'true' : 'false');
  }
}

export function toggleSidebar() {
  toggleSidebarCollapse();
}

function initSidebar() {
  try {
    if (localStorage.getItem('agy_sidebar_collapsed') === 'true' && window.innerWidth > 768) {
      document.body.classList.add('sidebar-collapsed');
    }
  } catch (e) {}

  const toggleBtn = el('sidebar-toggle-btn') || document.querySelector('.hamburger');
  if (toggleBtn) toggleBtn.addEventListener('click', () => toggleSidebarCollapse());
  const collapseBtn = el('btn-collapse-sidebar');
  if (collapseBtn) collapseBtn.addEventListener('click', () => toggleSidebarCollapse(true));

  window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'b') {
      const tag = document.activeElement?.tagName?.toLowerCase();
      if (tag === 'input' || tag === 'textarea' || tag === 'select') return;
      e.preventDefault();
      toggleSidebarCollapse();
    }
  });

  qsa('.sidebar nav a').forEach(a => {
    a.addEventListener('click', () => {
      if (window.innerWidth <= 768) el('sidebar')?.classList.remove('open');
    });
  });
}

async function initProjectSelector() {
  const select = el('project-selector');
  const headerSelect = el('header-project-selector');
  const badge = el('project-status-badge');
  const headerBadge = el('header-project-badge');
  const pruneBtn = el('btn-prune-projects');
  if (!select) return;

  const projects = await fetchProjects();
  const optionsHtml = [`<option value="all">${t('sidebar.all_projects_analytics')}</option>`];
  const seenPaths = new Set();

  projects.forEach(p => {
    let norm = (p.path || '').trim().replace(/\//g, '\\');
    while (norm.length > 3 && norm.endsWith('\\')) norm = norm.slice(0, -1);
    const key = norm.toLowerCase();
    if (seenPaths.has(key)) return;
    seenPaths.add(key);

    const statusIcon = p.status === 'active' ? '🟢' : t('sidebar.status_missing');
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
    const msg = isMissing ? t('sidebar.missing_msg') : (val === 'all' ? t('sidebar.all_tracked') : t('sidebar.active_project', { name: chosen?.name || val }));

    if (badge) {
      badge.textContent = isMissing ? t('sidebar.missing_on_disk') : '';
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
        title: t('dialog.prune_title'),
        message: t('dialog.prune_msg'),
        subtext: t('dialog.prune_subtext'),
        confirmText: t('dialog.prune_btn'),
        cancelText: t('dialog.cancel'),
        variant: 'danger',
      });
      if (ok) {
        const res = await pruneProjects();
        await showAlert({
          title: t('dialog.prune_title'),
          message: res.message || t('dialog.pruned_success', { count: res.pruned_count || 0 }),
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
        title: t('dialog.optimize_title'),
        message: t('dialog.optimize_msg'),
        subtext: t('dialog.optimize_subtext'),
        confirmText: t('dialog.optimize_btn'),
        cancelText: t('dialog.cancel'),
        variant: 'info',
      });
      if (!ok) return;

      const res = await triggerAutoCleanup();
      await showAlert({
        title: t('dialog.optimize_title'),
        message: res.message || t('dialog.cleanup_complete'),
        variant: res.success ? 'success' : 'danger',
      });
      fetchData();
    });
  }
}

function initLanguageSelector() {
  const select = el('lang-selector');
  if (!select) return;
  select.value = getLanguage();
  select.addEventListener('change', (e) => {
    setLanguage(e.target.value);
  });
  onLanguageChange(() => {
    initProjectSelector();
    const current = location.hash.slice(1) || 'dashboard';
    if (current === 'graph') {
      loadAndRenderGraph();
    } else if (current === 'logs') {
      loadAndRenderLogs();
    } else {
      fetchData();
    }
  });
}

function initEngineControl() {
  const btn = el('btn-embed-refresh');
  if (btn) {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      refreshEmbedStatus();
    });
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initI18n();
  initLanguageSelector();
  initA11y();
  initSidebar();
  initRouter();
  initProjectSelector();
  initEngineControl();
});

window.toggleSidebar = toggleSidebar;
window.toggleSidebarCollapse = toggleSidebarCollapse;
window.refreshEmbedStatus = refreshEmbedStatus;
window.changeProjectPath = changeProjectPath;
window.closeDirBrowser = closeDirBrowser;
window.selectDirPath = selectDirPath;
