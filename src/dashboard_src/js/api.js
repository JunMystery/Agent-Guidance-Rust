import { setText, setDisplay, setLoading, activeView, pollSpanFor } from './dom.js';
import { renderDashboard } from './render/statsView.js';
import { renderHealthPanel } from './render/health.js';
import { pollBackoff, setBackoff, resetBackoff } from './state.js';

let selectedProject = 'all';

export function setSelectedProject(proj) {
  selectedProject = proj;
}

export function getSelectedProject() {
  return selectedProject;
}

async function fetchStats() {
  const projParam = selectedProject && selectedProject !== 'all' ? `&project=${encodeURIComponent(selectedProject)}` : '';
  const resp = await fetch(`/api/stats?window=24h${projParam}`);
  if (!resp.ok) throw new Error('HTTP ' + resp.status);
  return resp.json();
}

export async function fetchGraphData(proj) {
  const p = proj || selectedProject;
  const param = p && p !== 'all' ? `?project=${encodeURIComponent(p)}` : '';
  try {
    const resp = await fetch(`/api/graph${param}`);
    if (!resp.ok) return { graph_available: false, message: `HTTP ${resp.status}` };
    return resp.json();
  } catch (e) {
    return { graph_available: false, message: e.message };
  }
}

export async function fetchProjects() {
  try {
    const resp = await fetch('/api/projects');
    if (!resp.ok) return [];
    const data = await resp.json();
    return data.projects || [];
  } catch (e) {
    return [];
  }
}

export async function pruneProjects() {
  try {
    const resp = await fetch('/api/projects/prune', { method: 'POST' });
    return resp.json();
  } catch (e) {
    return { success: false, error: e.message };
  }
}

export async function triggerAutoCleanup() {
  try {
    const resp = await fetch('/api/cleanup', { method: 'POST' });
    return resp.json();
  } catch (e) {
    return { success: false, error: e.message };
  }
}

async function fetchHealth() {
  const resp = await fetch('/health');
  return resp.json();
}

async function showFetchError(e) {
  setBackoff(Math.min(pollBackoff * 2, 30000));
  setText('error-banner', 'Failed to connect to stats: ' + (e.message || 'unknown'));
  setDisplay('error-banner', 'block');
  const pollSpanId = pollSpanFor(activeView());
  if (pollSpanId) {
    setText(pollSpanId, '(retry in ' + (pollBackoff / 1000).toFixed(0) + 's)');
  }
}

async function clearFetchError() {
  resetBackoff();
  setText('error-banner', '');
  setDisplay('error-banner', 'none');
}

export async function fetchData() {
  setLoading(true);
  let data;
  try {
    data = await fetchStats();
    renderDashboard(data);
    await clearFetchError();
  } catch (e) {
    console.error('fetch error', e);
    await showFetchError(e);
  }
  try {
    const hdata = await fetchHealth();
    renderHealthPanel(hdata, data?.totals?.embed_queries);
    if (hdata.db_size_bytes !== undefined) {
      const mb = (hdata.db_size_bytes / (1024 * 1024)).toFixed(2);
      setText('sidebar-db-size', `db size: ${mb} MB`);
    }
  } catch (e) {
    renderHealthPanel({ status: 'unknown' }, data?.totals?.embed_queries);
  }
  setLoading(false);
}

export async function refreshEmbedStatus() {
  const btn = document.getElementById('btn-embed-refresh');
  if (btn) { btn.disabled = true; btn.textContent = 'Refreshing…'; }
  try {
    const [stats, health] = await Promise.all([fetchStats().catch(() => null), fetchHealth().catch(() => null)]);
    if (stats) renderDashboard(stats);
    if (health) renderHealthPanel(health, stats?.totals?.embed_queries);
  } finally {
    if (btn) { btn.disabled = false; btn.textContent = 'Refresh Model Status'; }
  }
}
