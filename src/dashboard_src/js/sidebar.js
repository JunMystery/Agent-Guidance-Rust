import { el, qsa } from './dom.js';

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

export function initSidebar() {
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
