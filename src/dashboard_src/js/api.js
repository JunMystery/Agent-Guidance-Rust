import { setText, setDisplay, setLoading, activeView, pollSpanFor } from './dom.js';
import { renderDashboard } from './render/statsView.js';
import { renderHealthPanel } from './render/health.js';
import { pollBackoff, setBackoff, resetBackoff } from './state.js';
import { showAlert } from './dialog.js';
import { t } from './i18n/index.js';

let selectedProject = 'all';

export function setSelectedProject(proj) {
  selectedProject = proj;
}

export function getSelectedProject() {
  return selectedProject;
}

async function fetchStats() {
  const projParam = selectedProject ? `&project=${encodeURIComponent(selectedProject)}` : '&project=all';
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
  setText('error-banner', t('api.connect_failed', { error: e.message || t('status.unknown') }));
  setDisplay('error-banner', 'block');
  const pollSpanId = pollSpanFor(activeView());
  if (pollSpanId) {
    setText(pollSpanId, t('api.retry_in', { sec: (pollBackoff / 1000).toFixed(0) }));
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
    renderHealthPanel(hdata);
    if (hdata.db_size_bytes !== undefined) {
      const mb = (hdata.db_size_bytes / (1024 * 1024)).toFixed(2);
      setText('sidebar-db-size', t('sidebar.db_size_val', { size: mb }));
    }
  } catch (e) {
    renderHealthPanel({ status: 'unknown' });
  }
  setLoading(false);
}

export async function refreshEmbedEngine() {
  try {
    const resp = await fetch('/api/engine/refresh', { method: 'POST' });
    if (!resp.ok) throw new Error('HTTP ' + resp.status);
    return await resp.json();
  } catch (e) {
    return { success: false, error: e.message };
  }
}

export async function refreshEmbedStatus() {
  const btn = document.getElementById('btn-embed-refresh');
  const txt = document.getElementById('btn-embed-refresh-text');
  if (btn) btn.disabled = true;
  if (txt) {
    txt.textContent = t('deck.refreshing');
  } else if (btn) {
    btn.textContent = t('deck.refreshing');
  }
  try {
    const res = await refreshEmbedEngine();
    const [stats, health] = await Promise.all([fetchStats().catch(() => null), fetchHealth().catch(() => null)]);
    if (stats) renderDashboard(stats);
    if (health) renderHealthPanel(health);
    await showAlert({
      title: t('deck.engine_control'),
      message: res.message || (res.success ? 'Engine refreshed' : (res.error || 'Refresh failed')),
      variant: res.success ? 'success' : 'danger',
    });
  } finally {
    if (btn) btn.disabled = false;
    if (txt) {
      txt.textContent = t('deck.refresh_model');
    } else if (btn) {
      btn.textContent = t('deck.refresh_model');
    }
  }
}

export async function fetchLogs({ level = 'all', search = '', limit = 50, offset = 0 } = {}) {
  const query = new URLSearchParams();
  if (level && level !== 'all') query.set('level', level);
  if (search && search.trim()) query.set('search', search.trim());
  if (limit) query.set('limit', limit);
  if (offset) query.set('offset', offset);

  const res = await fetch('/api/logs?' + query.toString());
  if (!res.ok) throw new Error('Failed to fetch logs: ' + res.statusText);
  return res.json();
}

export async function clearLogs(level = 'all') {
  const query = level && level !== 'all' ? `?level=${encodeURIComponent(level)}` : '';
  const res = await fetch('/api/logs/clear' + query, { method: 'POST' });
  if (!res.ok) throw new Error('Failed to clear logs: ' + res.statusText);
  return res.json();
}
