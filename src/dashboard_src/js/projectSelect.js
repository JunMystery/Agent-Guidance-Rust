import { el } from './dom.js';
import { fetchProjects, pruneProjects, setSelectedProject, triggerAutoCleanup, fetchData, fetchGraphData } from './api.js';
import { renderGraphView } from './render/graphView.js';
import { showConfirm, showAlert } from './dialog.js';
import { t } from './i18n/index.js';

async function loadAndRenderGraph() {
  const data = await fetchGraphData();
  renderGraphView(data);
}

export async function initProjectSelector() {
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
    if (p.path && !seenPaths.has(p.path)) {
      seenPaths.add(p.path);
      const isMissing = p.status === 'missing';
      const label = `${p.name || p.path} ${isMissing ? `(${t('sidebar.missing')})` : ''}`.trim();
      optionsHtml.push(`<option value="${p.path}" ${isMissing ? 'style="color: #ffbd2e;"' : ''}>${label}</option>`);
    }
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
