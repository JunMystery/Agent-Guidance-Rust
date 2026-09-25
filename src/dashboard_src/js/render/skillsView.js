import {
  fetchSkillsRegistry,
  fetchSkillsBinaryStats,
  deleteBulkSkills,
  compileSkillsBinary,
} from '../api.js';
import { showAlert, showConfirm } from '../dialog.js';

let skillsCache = [];
let selectedNames = new Set();
let searchQuery = '';

export async function renderSkillsView() {
  const container = document.getElementById('view-skills');
  if (!container) return;

  container.innerHTML = `
    <div class="terminal-window">
      <div class="terminal-header">
        <div class="terminal-dots">
          <div class="terminal-dot dot-red"></div>
          <div class="terminal-dot dot-yellow"></div>
          <div class="terminal-dot dot-green"></div>
        </div>
        <div class="terminal-title">Centralized Skill Registry &amp; Binary Compactor</div>
      </div>
      <div class="terminal-body-raw" style="padding:20px">
        <!-- Binary Metrics Banner -->
        <div id="skills-binary-stats-card" style="display:grid;grid-template-columns:repeat(auto-fit, minmax(180px, 1fr));gap:12px;margin-bottom:20px">
          <div style="background:var(--bg-tertiary);border:1px solid var(--border-color);border-radius:6px;padding:12px">
            <div style="font-size:11px;color:var(--text-secondary);text-transform:uppercase">skills.bin Size</div>
            <div id="metric-skills-bin" style="font-size:18px;font-weight:700;margin-top:4px">--</div>
          </div>
          <div style="background:var(--bg-tertiary);border:1px solid var(--border-color);border-radius:6px;padding:12px">
            <div style="font-size:11px;color:var(--text-secondary);text-transform:uppercase">vectors.bin Size</div>
            <div id="metric-vectors-bin" style="font-size:18px;font-weight:700;margin-top:4px">--</div>
          </div>
          <div style="background:var(--bg-tertiary);border:1px solid var(--border-color);border-radius:6px;padding:12px">
            <div style="font-size:11px;color:var(--text-secondary);text-transform:uppercase">Total Skills / Sections</div>
            <div id="metric-skills-count" style="font-size:18px;font-weight:700;margin-top:4px">--</div>
          </div>
          <div style="background:var(--bg-tertiary);border:1px solid var(--border-color);border-radius:6px;padding:12px">
            <div style="font-size:11px;color:var(--text-secondary);text-transform:uppercase">Catalog Hash / Tombstones</div>
            <div id="metric-catalog-hash" style="font-size:14px;font-family:var(--font-mono);margin-top:6px;overflow:hidden;text-overflow:ellipsis">--</div>
          </div>
        </div>

        <!-- Action Toolbar -->
        <div style="display:flex;flex-wrap:wrap;gap:12px;align-items:center;margin-bottom:16px;justify-content:space-between">
          <div style="display:flex;gap:10px;align-items:center;flex:1;max-width:400px">
            <input type="text" id="skills-search-input" placeholder="Filter skills by name..."
              style="width:100%;padding:8px 12px;border-radius:var(--border-radius-sm);background:var(--bg-tertiary);color:var(--text-primary);border:1px solid var(--border-color);font-size:13px">
          </div>
          <div style="display:flex;gap:12px;align-items:center">
            <label style="display:flex;align-items:center;gap:6px;font-size:12px;color:var(--text-secondary);cursor:pointer">
              <input type="checkbox" id="skills-purge-staging-toggle" checked style="cursor:pointer">
              <span>Purge from staging directory</span>
            </label>
            <button id="btn-compile-staging" class="btn-secondary" style="padding:7px 14px;cursor:pointer" title="Compile staging skills to binary with SHA-256 diff">
              ⚡ Compile Staging to Binary
            </button>
            <button id="btn-delete-selected-skills" class="btn-accent" style="padding:7px 14px;background:#ef4444;border-color:#dc2626;cursor:pointer" disabled>
              🗑️ Delete Selected (<span id="delete-count-badge">0</span>)
            </button>
          </div>
        </div>

        <!-- Skills Table -->
        <div style="overflow-x:auto;border:1px solid var(--border-color);border-radius:6px;background:var(--bg-secondary)">
          <table style="width:100%;border-collapse:collapse;font-size:13px;text-align:left">
            <thead>
              <tr style="background:var(--bg-tertiary);border-bottom:1px solid var(--border-color)">
                <th style="padding:10px 14px;width:40px">
                  <input type="checkbox" id="skills-select-all" style="cursor:pointer">
                </th>
                <th style="padding:10px 14px">Skill Slug</th>
                <th style="padding:10px 14px">Sections</th>
                <th style="padding:10px 14px">Source</th>
                <th style="padding:10px 14px">Status</th>
                <th style="padding:10px 14px">Size</th>
              </tr>
            </thead>
            <tbody id="skills-table-body">
              <tr><td colspan="6" style="padding:20px;text-align:center;color:var(--text-secondary)">Loading skills registry...</td></tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  `;

  bindSkillsEvents();
  await refreshSkillsData();
}

function bindSkillsEvents() {
  const searchInput = document.getElementById('skills-search-input');
  if (searchInput) {
    searchInput.addEventListener('input', (e) => {
      searchQuery = e.target.value.toLowerCase().trim();
      renderSkillsTable();
    });
  }

  const selectAll = document.getElementById('skills-select-all');
  if (selectAll) {
    selectAll.addEventListener('change', (e) => {
      const checked = e.target.checked;
      const visible = getFilteredSkills();
      if (checked) {
        visible.forEach(s => selectedNames.add(s.name));
      } else {
        visible.forEach(s => selectedNames.delete(s.name));
      }
      updateSelectionUI();
    });
  }

  const btnCompile = document.getElementById('btn-compile-staging');
  if (btnCompile) {
    btnCompile.addEventListener('click', onCompileClick);
  }

  const btnDelete = document.getElementById('btn-delete-selected-skills');
  if (btnDelete) {
    btnDelete.addEventListener('click', onDeleteClick);
  }
}

async function refreshSkillsData() {
  try {
    const [reg, bstats] = await Promise.all([
      fetchSkillsRegistry().catch(() => ({ skills: [] })),
      fetchSkillsBinaryStats().catch(() => ({})),
    ]);

    skillsCache = reg.skills || [];
    renderSkillsStats(bstats, skillsCache.length);
    renderSkillsTable();
  } catch (e) {
    console.error('Error refreshing skills data:', e);
  }
}

function renderSkillsStats(stats, total) {
  const fmtBytes = (b) => {
    if (!b || b === 0) return '0 KB';
    if (b > 1024 * 1024) return (b / (1024 * 1024)).toFixed(2) + ' MB';
    return (b / 1024).toFixed(1) + ' KB';
  };

  const sBin = document.getElementById('metric-skills-bin');
  const vBin = document.getElementById('metric-vectors-bin');
  const sCount = document.getElementById('metric-skills-count');
  const cHash = document.getElementById('metric-catalog-hash');

  if (sBin) sBin.textContent = fmtBytes(stats.skills_bin_bytes);
  if (vBin) vBin.textContent = fmtBytes(stats.vectors_bin_bytes);
  if (sCount) sCount.textContent = `${total} skills (${stats.total_skills || total} compiled)`;
  if (cHash) {
    const hash = stats.catalog_hash || 'none';
    const tombs = stats.tombstones_count || 0;
    cHash.innerHTML = `<span title="${hash}">${hash.substring(0, 12)}...</span> · ${tombs} tombstones`;
  }
}

function getFilteredSkills() {
  if (!searchQuery) return skillsCache;
  return skillsCache.filter(s => s.name.toLowerCase().includes(searchQuery));
}

function renderSkillsTable() {
  const tbody = document.getElementById('skills-table-body');
  if (!tbody) return;

  const filtered = getFilteredSkills();
  if (filtered.length === 0) {
    tbody.innerHTML = `<tr><td colspan="6" style="padding:24px;text-align:center;color:var(--text-secondary)">No skills match "${searchQuery}"</td></tr>`;
    return;
  }

  tbody.innerHTML = filtered.map(skill => {
    const isSelected = selectedNames.has(skill.name);
    const sourceBadge = skill.source === 'staged_override'
      ? '<span style="background:rgba(168,85,247,0.15);color:#a855f7;padding:2px 6px;border-radius:4px;font-size:11px">Staged Override</span>'
      : skill.source === 'staging_only'
      ? '<span style="background:rgba(234,179,8,0.15);color:#eab308;padding:2px 6px;border-radius:4px;font-size:11px">Staging Only</span>'
      : '<span style="background:rgba(59,130,246,0.15);color:#3b82f6;padding:2px 6px;border-radius:4px;font-size:11px">Binary AGV1</span>';

    const statusBadge = skill.tombstoned
      ? '<span style="background:rgba(239,68,68,0.15);color:#ef4444;padding:2px 6px;border-radius:4px;font-size:11px">Tombstoned</span>'
      : '<span style="background:rgba(16,185,129,0.15);color:#10b981;padding:2px 6px;border-radius:4px;font-size:11px">Active</span>';

    const lenKb = skill.content_len ? `${(skill.content_len / 1024).toFixed(1)} KB` : '--';

    return `
      <tr style="border-bottom:1px solid var(--border-color);background:${isSelected ? 'rgba(59,130,246,0.06)' : 'transparent'}">
        <td style="padding:10px 14px">
          <input type="checkbox" class="skill-row-checkbox" data-name="${skill.name}" ${isSelected ? 'checked' : ''} style="cursor:pointer">
        </td>
        <td style="padding:10px 14px;font-weight:600;font-family:var(--font-mono)">${skill.name}</td>
        <td style="padding:10px 14px">${skill.sections || 1} sections</td>
        <td style="padding:10px 14px">${sourceBadge}</td>
        <td style="padding:10px 14px">${statusBadge}</td>
        <td style="padding:10px 14px;color:var(--text-secondary)">${lenKb}</td>
      </tr>
    `;
  }).join('');

  tbody.querySelectorAll('.skill-row-checkbox').forEach(cb => {
    cb.addEventListener('change', (e) => {
      const name = e.target.dataset.name;
      if (e.target.checked) selectedNames.add(name);
      else selectedNames.delete(name);
      updateSelectionUI();
    });
  });

  updateSelectionUI();
}

function updateSelectionUI() {
  const badge = document.getElementById('delete-count-badge');
  const btnDelete = document.getElementById('btn-delete-selected-skills');
  const selectAll = document.getElementById('skills-select-all');

  const count = selectedNames.size;
  if (badge) badge.textContent = count;
  if (btnDelete) btnDelete.disabled = count === 0;

  const visible = getFilteredSkills();
  if (selectAll && visible.length > 0) {
    const allSelected = visible.every(s => selectedNames.has(s.name));
    selectAll.checked = allSelected;
  }
}

async function onCompileClick() {
  const btn = document.getElementById('btn-compile-staging');
  if (btn) btn.disabled = true;
  try {
    const res = await compileSkillsBinary(false);
    if (res.status === 'ok') {
      const st = res.stats || {};
      await showAlert({
        title: 'Skills Compilation Succeeded',
        message: `Compiled ${st.total_skills || 0} skills in ${st.duration_ms || 0} ms.\nReindexed: ${st.reindexed || 0} | Unchanged: ${st.unchanged_skipped || 0}\nCatalog Hash: ${st.catalog_hash || 'ok'}`,
        variant: 'success',
      });
      await refreshSkillsData();
    }
  } catch (e) {
    await showAlert({ title: 'Compilation Failed', message: e.message, variant: 'danger' });
  } finally {
    if (btn) btn.disabled = false;
  }
}

async function onDeleteClick() {
  if (selectedNames.size === 0) return;
  const count = selectedNames.size;
  const purgeToggle = document.getElementById('skills-purge-staging-toggle');
  const purge = purgeToggle ? purgeToggle.checked : true;

  const confirmed = await showConfirm({
    title: `Delete ${count} Skill(s)?`,
    message: `Are you sure you want to delete ${count} selected skill(s)? This will compact vectors.bin and record them into tombstones.json${purge ? ' and purge their staging folders' : ''}.`,
    confirmText: 'Delete & Compact',
    variant: 'danger',
  });

  if (!confirmed) return;

  const btnDelete = document.getElementById('btn-delete-selected-skills');
  if (btnDelete) btnDelete.disabled = true;

  try {
    const names = Array.from(selectedNames);
    const res = await deleteBulkSkills(names, purge);
    if (res.status === 'ok') {
      selectedNames.clear();
      await showAlert({
        title: 'Skills Deleted & Compacted',
        message: `Successfully pruned ${names.length} skills in ${res.result?.duration_ms || 0} ms.\nNew skills.bin count: ${res.result?.new_count || 'updated'}`,
        variant: 'success',
      });
      await refreshSkillsData();
    }
  } catch (e) {
    await showAlert({ title: 'Deletion Failed', message: e.message, variant: 'danger' });
  } finally {
    if (btnDelete) btnDelete.disabled = selectedNames.size === 0;
  }
}
