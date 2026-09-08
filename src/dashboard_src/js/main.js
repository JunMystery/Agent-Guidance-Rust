import { qsa, el } from './dom.js';
import { fetchData, refreshEmbedStatus, fetchProjects, pruneProjects, setSelectedProject, triggerAutoCleanup, fetchGraphData } from './api.js';
import { renderGraphView } from './render/graphView.js';
import { startPoll, stopPoll } from './poll.js';



const VIEWS = ['dashboard', 'top-skills', 'actions', 'recent-calls', 'guides', 'graph'];

export async function loadAndRenderGraph() {
  const data = await fetchGraphData();
  renderGraphView(data);
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
  if (view === 'graph') loadAndRenderGraph();
  if (view === 'actions' || view === 'recent-calls') startPoll();
  else stopPoll();
}

function initA11y() {
  // Static header cells act as column headers.
  qsa('table th').forEach(th => { if (!th.hasAttribute('scope')) th.setAttribute('scope', 'col'); });
  // Roving tabindex: arrow keys move between view tabs.
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
  const badge = el('project-status-badge');
  const pruneBtn = el('btn-prune-projects');
  if (!select) return;

  const projects = await fetchProjects();
  select.innerHTML = '<option value="all">🌐 All Projects (Global)</option>';

  projects.forEach(p => {
    const opt = document.createElement('option');
    opt.value = p.path;
    const statusIcon = p.status === 'active' ? '🟢' : '⚠️ [Missing]';
    opt.textContent = `${statusIcon} ${p.name} (${p.path})`;
    select.appendChild(opt);
  });

  select.addEventListener('change', () => {
    setSelectedProject(select.value);
    const chosen = projects.find(p => p.path === select.value);
    if (chosen && chosen.status === 'missing') {
      badge.textContent = '⚠️ Missing on disk (Showing historical records)';
      badge.style.display = 'block';
      badge.style.color = '#ffbd2e';
    } else {
      badge.style.display = 'none';
    }
    fetchData();
    if (location.hash.slice(1) === 'graph') {
      loadAndRenderGraph();
    }
  });

  const defaultProj = el('sidebar-proj')?.textContent?.trim();
  if (defaultProj && defaultProj !== '--') {
    const match = projects.find(p => p.path === defaultProj || p.name === defaultProj);
    if (match) {
      select.value = match.path;
      setSelectedProject(match.path);
    }
  }

  if (pruneBtn) {
    pruneBtn.addEventListener('click', async () => {
      if (confirm('Prune missing/deleted projects from tracking registry?')) {
        const res = await pruneProjects();
        alert(res.message || `Pruned ${res.pruned_count || 0} projects`);
        await initProjectSelector();
        fetchData();
      }
    });
  }

  const cleanupBtn = el('btn-auto-cleanup');
  if (cleanupBtn) {
    cleanupBtn.addEventListener('click', async () => {
      cleanupBtn.disabled = true;
      cleanupBtn.textContent = 'Optimizing…';
      try {
        const res = await triggerAutoCleanup();
        if (res.success) {
          const s = res.summary || {};
          const msg = `✓ Cleanup & Vacuum Complete!\n• Tool calls pruned: ${s.tool_calls_pruned}\n• Skill loads pruned: ${s.skill_loads_pruned}\n• Queries pruned: ${s.embed_queries_pruned + s.llm_queries_pruned}\n• Daily summaries pruned: ${s.daily_summaries_pruned}\n• Dead projects pruned: ${s.dead_projects_pruned}\n• Current DB size: ${res.db_size_mb || '--'}`;
          alert(msg);
          await initProjectSelector();
          fetchData();
        } else {
          alert('Cleanup failed: ' + (res.error || 'unknown error'));
        }
      } finally {
        cleanupBtn.disabled = false;
        cleanupBtn.textContent = '⚡ Vacuum & Optimize DB';
      }
    });
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initRouter();
  initA11y();
  initProjectSelector();
  qsa('.sidebar nav a').forEach(a => {
    a.addEventListener('click', () => {
      if (window.innerWidth <= 768) el('sidebar').classList.remove('open');
    });
  });
});

document.addEventListener('visibilitychange', () => {
  const view = document.querySelector('.sidebar nav a.active')?.dataset?.view;
  const onPollView = view === 'actions' || view === 'recent-calls';
  if (document.hidden) stopPoll();
  else if (onPollView) startPoll();
});

let currentBrowserPath = '.';

async function changeProjectPath() {
  try {
    const resp = await fetch('/api/dirs/choose', { method: 'POST' });
    const data = await resp.json();
    if (data.success && data.path) {
      fetchData();
    } else {
      openDirBrowser();
    }
  } catch (e) {
    openDirBrowser();
  }
}

async function openDirBrowser(path = '.') {
  currentBrowserPath = path;
  el('dir-modal').classList.add('open');
  await renderDirBrowser(path);
}

async function renderDirBrowser(path) {
  try {
    const resp = await fetch('/api/dirs?path=' + encodeURIComponent(path));
    const data = await resp.json();
    el('dir-current').textContent = data.current || path;
    currentBrowserPath = data.current || path;
    
    const list = el('dir-list');
    list.innerHTML = '';
    
    if (data.parent) {
      const pdiv = document.createElement('div');
      pdiv.className = 'dir-item parent';
      pdiv.textContent = '📁 .. (Up one directory)';
      pdiv.onclick = () => renderDirBrowser(data.parent);
      list.appendChild(pdiv);
    }
    
    if (data.dirs && data.dirs.length) {
      data.dirs.forEach(d => {
        const ddiv = document.createElement('div');
        ddiv.className = 'dir-item';
        ddiv.textContent = '📁 ' + d.name;
        ddiv.onclick = () => renderDirBrowser(d.path);
        list.appendChild(ddiv);
      });
    } else if (!data.parent) {
      list.innerHTML = '<div class="dir-empty">No directories found or permission denied.</div>';
    }
  } catch (e) {
    el('dir-list').innerHTML = '<div class="dir-error">Error loading directory.</div>';
  }
}

function closeDirBrowser() {
  el('dir-modal').classList.remove('open');
}

async function selectDirPath() {
  try {
    const resp = await fetch('/api/dirs/select?path=' + encodeURIComponent(currentBrowserPath), { method: 'POST' });
    const data = await resp.json();
    if (data.success) {
      closeDirBrowser();
      fetchData();
    } else {
      alert('Failed to select directory: ' + (data.error || 'unknown'));
    }
  } catch (e) {
    alert('Error selecting directory: ' + e.message);
  }
}

window.toggleSidebar = toggleSidebar;
window.refreshEmbedStatus = refreshEmbedStatus;
window.changeProjectPath = changeProjectPath;
window.closeDirBrowser = closeDirBrowser;
window.selectDirPath = selectDirPath;
