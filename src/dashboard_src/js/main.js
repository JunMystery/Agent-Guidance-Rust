import { qsa, el } from './dom.js';
import { fetchData, refreshEmbedStatus, fetchGraphData } from './api.js';
import { renderGraphView } from './render/graphView.js';
import { renderLogsView } from './render/logsView.js';
import { startPoll, stopPoll } from './poll.js';
import { changeProjectPath, closeDirBrowser, selectDirPath } from './dirBrowser.js';
import { initI18n, getLanguage, setLanguage, onLanguageChange } from './i18n/index.js';
import { initSidebar, toggleSidebar, toggleSidebarCollapse } from './sidebar.js';
import { initProjectSelector } from './projectSelect.js';

export { toggleSidebar, toggleSidebarCollapse };

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
