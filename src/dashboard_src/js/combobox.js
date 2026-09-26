// Lightweight Searchable Combobox for Project Selection
import { escapeHtml } from './dom.js';
import { t } from './i18n/index.js';

export function createProjectCombobox(options) {
  const {
    inputEl,
    dropdownEl,
    optionsEl,
    arrowEl,
    projects = [],
    selectedVal = 'all',
    onSelect,
    onDelete,
  } = options;

  if (!inputEl || !dropdownEl || !optionsEl) return null;

  let currentProjects = [...projects];
  let activeVal = selectedVal;
  let focusedIndex = -1;
  let isOpen = false;

  function getDisplayLabel(val) {
    if (!val || val === 'all') return t('sidebar.all_projects_analytics');
    const p = currentProjects.find(x => x.path === val);
    return p ? (p.name || p.path) : val;
  }

  function renderList(query = '') {
    const q = query.trim().toLowerCase();
    const filtered = currentProjects.filter(p => {
      if (!q) return true;
      return (p.name && p.name.toLowerCase().includes(q)) ||
             (p.path && p.path.toLowerCase().includes(q));
    });

    const isAllVisible = !q || 'all'.includes(q) || t('sidebar.all_projects_analytics').toLowerCase().includes(q);
    let html = '';

    if (isAllVisible) {
      const isSelected = activeVal === 'all';
      html += `<div class="combobox-item ${isSelected ? 'active' : ''}" data-value="all">
        <div class="combobox-item-info">
          <span class="combobox-item-name">${t('sidebar.all_projects_analytics')}</span>
          <span class="combobox-item-path">global scope</span>
        </div>
      </div>`;
    }

    if (filtered.length === 0 && !isAllVisible) {
      html += `<div class="combobox-empty">${t('combobox.no_matches')}</div>`;
    } else {
      filtered.forEach(p => {
        const isSelected = p.path === activeVal;
        const isMissing = p.status === 'missing';
        html += `<div class="combobox-item ${isSelected ? 'active' : ''}" data-value="${escapeHtml(p.path)}">
          <div class="combobox-item-info">
            <span class="combobox-item-name">${escapeHtml(p.name || p.path)}</span>
            <span class="combobox-item-path">${escapeHtml(p.path)}</span>
          </div>
          ${isMissing ? `<span class="combobox-tag-missing">${t('sidebar.missing')}</span>` : ''}
          <button type="button" class="combobox-delete-btn" data-project="${escapeHtml(p.path)}" data-name="${escapeHtml(p.name || p.path)}" title="${t('dialog.delete_project_title')}">🗑️</button>
        </div>`;
      });
    }

    optionsEl.innerHTML = html;
    bindItemEvents();
  }

  function openDropdown() {
    isOpen = true;
    dropdownEl.classList.remove('hidden');
    inputEl.setAttribute('aria-expanded', 'true');
    renderList(inputEl.value === getDisplayLabel(activeVal) ? '' : inputEl.value);
  }

  function closeDropdown() {
    isOpen = false;
    focusedIndex = -1;
    dropdownEl.classList.add('hidden');
    inputEl.setAttribute('aria-expanded', 'false');
    inputEl.value = getDisplayLabel(activeVal);
  }

  function bindItemEvents() {
    optionsEl.querySelectorAll('.combobox-item').forEach(item => {
      item.addEventListener('click', (e) => {
        if (e.target.closest('.combobox-delete-btn')) return;
        const val = item.dataset.value;
        selectValue(val);
      });
    });

    optionsEl.querySelectorAll('.combobox-delete-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        e.preventDefault();
        const pPath = btn.dataset.project;
        const pName = btn.dataset.name;
        if (onDelete) onDelete(pPath, pName);
      });
    });
  }

  function selectValue(val) {
    activeVal = val;
    inputEl.value = getDisplayLabel(val);
    closeDropdown();
    if (onSelect) onSelect(val);
  }

  inputEl.value = getDisplayLabel(activeVal);

  inputEl.addEventListener('focus', () => openDropdown());
  inputEl.addEventListener('click', () => { if (!isOpen) openDropdown(); });
  inputEl.addEventListener('input', () => {
    if (!isOpen) openDropdown();
    renderList(inputEl.value);
  });

  if (arrowEl) {
    arrowEl.addEventListener('click', (e) => {
      e.stopPropagation();
      if (isOpen) closeDropdown();
      else { inputEl.focus(); openDropdown(); }
    });
  }

  inputEl.addEventListener('keydown', (e) => {
    const items = optionsEl.querySelectorAll('.combobox-item');
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (!isOpen) openDropdown();
      focusedIndex = (focusedIndex + 1) % items.length;
      updateFocus(items);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (!isOpen) openDropdown();
      focusedIndex = (focusedIndex - 1 + items.length) % items.length;
      updateFocus(items);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (focusedIndex >= 0 && items[focusedIndex]) {
        selectValue(items[focusedIndex].dataset.value);
      } else {
        closeDropdown();
      }
    } else if (e.key === 'Escape') {
      closeDropdown();
    }
  });

  function updateFocus(items) {
    items.forEach((it, idx) => {
      it.classList.toggle('focused', idx === focusedIndex);
      if (idx === focusedIndex) it.scrollIntoView({ block: 'nearest' });
    });
  }

  document.addEventListener('click', (e) => {
    if (!inputEl.contains(e.target) && !dropdownEl.contains(e.target) && !(arrowEl && arrowEl.contains(e.target))) {
      if (isOpen) closeDropdown();
    }
  });

  return {
    setValue: (val) => {
      activeVal = val;
      inputEl.value = getDisplayLabel(val);
    },
    updateProjects: (newProjects) => {
      currentProjects = [...newProjects];
      inputEl.value = getDisplayLabel(activeVal);
      if (isOpen) renderList(inputEl.value);
    },
  };
}
