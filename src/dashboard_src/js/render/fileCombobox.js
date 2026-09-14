import { t } from '../i18n/index.js';

function escapeHtml(str) {
  return String(str || '').replace(/[&<>"']/g, m => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[m]));
}

export function initFileCombobox(container, options = {}) {
  if (!container) return;

  const {
    files = [],
    selectedFile = '',
    onSelect = () => {}
  } = options;

  const sortedFiles = [...files].sort((a, b) => a.localeCompare(b));
  let isOpen = false;
  let activeIndex = -1;
  let currentSelection = selectedFile || 'all';

  const getDisplayVal = (val) => {
    if (val === 'all') return `🌐 ${t('graph.show_all_files')}`;
    return val ? val : '';
  };

  container.innerHTML = `
    <div class="file-combobox" style="position: relative; display: inline-flex; align-items: center;">
      <div style="position: relative; display: flex; align-items: center;">
        <input type="text" id="combobox-search-input"
          placeholder="${t('graph.select_file_placeholder')}"
          value="${escapeHtml(getDisplayVal(currentSelection))}"
          autocomplete="off"
          spellcheck="false"
          style="background: var(--bg-secondary); border: 1px solid var(--border-color); color: #38bdf8; font-size: 11px; padding: 4px 26px 4px 8px; border-radius: var(--border-radius-sm); outline: none; width: 240px; font-family: var(--font-mono); text-overflow: ellipsis;"
        />
        <button type="button" id="combobox-toggle-btn"
          style="position: absolute; right: 2px; top: 50%; transform: translateY(-50%); background: transparent; border: none; color: #94a3b8; font-size: 10px; cursor: pointer; padding: 2px 6px; line-height: 1;">
          ▼
        </button>
      </div>
      <div id="combobox-dropdown"
        style="display: none; position: absolute; top: calc(100% + 4px); left: 0; width: 360px; max-height: 280px; overflow-y: auto; background: #0b1329; border: 1px solid #334155; border-radius: 6px; box-shadow: 0 8px 24px rgba(0,0,0,0.6); z-index: 9999; padding: 4px;">
      </div>
    </div>
  `;

  const input = container.querySelector('#combobox-search-input');
  const toggleBtn = container.querySelector('#combobox-toggle-btn');
  const dropdown = container.querySelector('#combobox-dropdown');

  function renderOptions(query = '') {
    const q = query.trim().toLowerCase();
    const items = [];

    // Always include "all" option if matching or no query
    if (!q || 'all'.includes(q) || t('graph.show_all_files').toLowerCase().includes(q)) {
      items.push({ value: 'all', label: `🌐 ${t('graph.show_all_files')}`, sub: 'Cross-file call relationships' });
    }

    // Filter file items
    for (const f of sortedFiles) {
      if (!q || f.toLowerCase().includes(q)) {
        const parts = f.split(/[/\\]/);
        const name = parts.pop();
        const dir = parts.join('/') || '.';
        items.push({ value: f, label: name, sub: dir });
      }
    }

    if (!items.length) {
      dropdown.innerHTML = `<div style="padding: 8px 12px; font-size: 11px; color: #64748b; text-align: center;">${t('graph.nav_no_match')}</div>`;
      return items;
    }

    dropdown.innerHTML = items.map((item, idx) => {
      const isSel = item.value === currentSelection;
      const isAct = idx === activeIndex;
      return `
        <div class="combobox-item" data-value="${escapeHtml(item.value)}" data-index="${idx}"
          style="padding: 5px 8px; border-radius: 4px; cursor: pointer; display: flex; flex-direction: column; gap: 1px; background: ${isAct ? '#0284c730' : (isSel ? '#0284c718' : 'transparent')}; border-left: 2px solid ${isSel ? '#00e5ff' : 'transparent'};">
          <div style="font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: ${isSel ? '#00e5ff' : '#f1f5f9'}; text-overflow: ellipsis; overflow: hidden; white-space: nowrap;">
            ${item.value === 'all' ? '' : '📄 '}${escapeHtml(item.label)}
          </div>
          <div style="font-family: var(--font-mono); font-size: 9px; color: #64748b; text-overflow: ellipsis; overflow: hidden; white-space: nowrap;">
            ${escapeHtml(item.sub)}
          </div>
        </div>
      `;
    }).join('');

    dropdown.querySelectorAll('.combobox-item').forEach(elItem => {
      elItem.onmouseenter = () => {
        activeIndex = parseInt(elItem.getAttribute('data-index'), 10);
      };
      elItem.onclick = (e) => {
        e.stopPropagation();
        selectValue(elItem.getAttribute('data-value'));
      };
    });

    return items;
  }

  function openDropdown() {
    isOpen = true;
    activeIndex = -1;
    dropdown.style.display = 'block';
    renderOptions(input.value.startsWith('🌐') ? '' : input.value);
  }

  function closeDropdown() {
    isOpen = false;
    activeIndex = -1;
    dropdown.style.display = 'none';
    input.value = getDisplayVal(currentSelection);
  }

  function selectValue(val) {
    currentSelection = val;
    input.value = getDisplayVal(val);
    closeDropdown();
    onSelect(val);
  }

  input.addEventListener('focus', () => {
    openDropdown();
    input.select();
  });

  input.addEventListener('input', () => {
    if (!isOpen) openDropdown();
    activeIndex = 0;
    renderOptions(input.value);
  });

  input.addEventListener('keydown', (e) => {
    if (!isOpen) {
      if (e.key === 'ArrowDown' || e.key === 'Enter') {
        openDropdown();
        e.preventDefault();
      }
      return;
    }

    const items = dropdown.querySelectorAll('.combobox-item');
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      activeIndex = Math.min(items.length - 1, activeIndex + 1);
      updateActiveStyles(items);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      activeIndex = Math.max(0, activeIndex - 1);
      updateActiveStyles(items);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (items.length > 0) {
        const targetIdx = activeIndex >= 0 ? activeIndex : 0;
        const targetVal = items[targetIdx]?.getAttribute('data-value');
        if (targetVal) selectValue(targetVal);
      }
    } else if (e.key === 'Escape') {
      closeDropdown();
    }
  });

  function updateActiveStyles(items) {
    items.forEach((item, idx) => {
      if (idx === activeIndex) {
        item.style.background = '#0284c730';
        item.scrollIntoView({ block: 'nearest' });
      } else {
        const isSel = item.getAttribute('data-value') === currentSelection;
        item.style.background = isSel ? '#0284c718' : 'transparent';
      }
    });
  }

  toggleBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    if (isOpen) closeDropdown();
    else {
      openDropdown();
      input.focus();
    }
  });

  const onDocClick = (e) => {
    if (!container.contains(e.target)) {
      closeDropdown();
    }
  };
  document.addEventListener('click', onDocClick);

  return {
    setValue: (val) => {
      currentSelection = val;
      input.value = getDisplayVal(val);
    },
    destroy: () => {
      document.removeEventListener('click', onDocClick);
    }
  };
}
