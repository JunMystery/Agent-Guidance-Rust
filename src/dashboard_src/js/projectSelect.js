import { el } from './dom.js';
import { fetchProjects, pruneProjects, deleteProject, setSelectedProject, getSelectedProject, triggerAutoCleanup, fetchData, fetchGraphData } from './api.js';
import { renderGraphView } from './render/graphView.js';
import { showConfirm, showAlert } from './dialog.js';
import { createProjectCombobox } from './combobox.js';
import { t } from './i18n/index.js';

let headerCombobox = null;

async function loadAndRenderGraph() {
  const data = await fetchGraphData({ view: 'files' });
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
  const uniqueProjects = [];

  projects.forEach(p => {
    if (p.path && !seenPaths.has(p.path)) {
      seenPaths.add(p.path);
      uniqueProjects.push(p);
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
    if (headerCombobox) headerCombobox.setValue(val);
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

  const onProjectDelete = async (pPath, pName) => {
    const ok = await showConfirm({
      title: t('dialog.delete_project_title'),
      message: t('dialog.delete_project_msg', { name: pName || pPath }),
      subtext: t('dialog.delete_project_subtext'),
      confirmText: t('dialog.delete_project_btn'),
      cancelText: t('dialog.cancel'),
      variant: 'danger',
    });
    if (!ok) return;

    const res = await deleteProject(pPath, true);
    await showAlert({
      title: t('dialog.delete_project_title'),
      message: res.message || t('dialog.delete_project_success', { name: pName || pPath }),
      variant: res.success ? 'success' : 'danger',
    });

    if (getSelectedProject() === pPath) {
      onProjectChange('all');
    }
    await initProjectSelector();
    fetchData();
  };

  const inputEl = el('header-project-search');
  const dropdownEl = el('header-combobox-dropdown');
  const optionsEl = el('header-combobox-options');
  const arrowEl = el('header-combobox-arrow');

  if (inputEl && dropdownEl && optionsEl) {
    headerCombobox = createProjectCombobox({
      inputEl,
      dropdownEl,
      optionsEl,
      arrowEl,
      projects: uniqueProjects,
      selectedVal: getSelectedProject() || 'all',
      onSelect: onProjectChange,
      onDelete: onProjectDelete,
    });
  }

  select.addEventListener('change', () => onProjectChange(select.value));
  if (headerSelect) {
    headerSelect.addEventListener('change', () => onProjectChange(headerSelect.value));
  }

  const defaultProj = el('sidebar-proj')?.textContent?.trim();
  if (defaultProj && defaultProj !== '--') {
    const match = projects.find(p => p.path === defaultProj || p.name === defaultProj);
    if (match) {
      onProjectChange(match.path);
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
