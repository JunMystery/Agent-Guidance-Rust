// Directory browser modal component for changing tracked project paths.
import { el } from './dom.js';
import { fetchData } from './api.js';
import { showAlert } from './dialog.js';
import { t } from './i18n/index.js';

let currentBrowserPath = '.';

export async function changeProjectPath() {
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

export async function openDirBrowser(path = '.') {
  currentBrowserPath = path;
  el('dir-modal')?.classList.add('open');
  await renderDirBrowser(path);
}

export async function renderDirBrowser(path) {
  try {
    const resp = await fetch('/api/dirs?path=' + encodeURIComponent(path));
    const data = await resp.json();
    const curEl = el('dir-current');
    if (curEl) curEl.textContent = data.current || path;
    currentBrowserPath = data.current || path;

    const list = el('dir-list');
    if (!list) return;
    list.innerHTML = '';

    if (data.parent) {
      const pdiv = document.createElement('div');
      pdiv.className = 'dir-item parent';
      pdiv.textContent = t('dir_browser.up_one_dir');
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
      list.innerHTML = `<div class="dir-empty">${t('dir_browser.empty')}</div>`;
    }
  } catch (e) {
    const list = el('dir-list');
    if (list) list.innerHTML = `<div class="dir-error">${t('dir_browser.error')}</div>`;
  }
}

export function closeDirBrowser() {
  el('dir-modal')?.classList.remove('open');
}

export async function selectDirPath() {
  try {
    const resp = await fetch('/api/dirs/select?path=' + encodeURIComponent(currentBrowserPath), { method: 'POST' });
    const data = await resp.json();
    if (data.success) {
      closeDirBrowser();
      fetchData();
    } else {
      await showAlert({
        title: t('dir_browser.select_failed'),
        message: data.error || t('dir_browser.failed_msg'),
        variant: 'danger',
      });
    }
  } catch (e) {
    await showAlert({
      title: t('dir_browser.select_error'),
      message: e.message || t('dir_browser.error_msg'),
      variant: 'danger',
    });
  }
}
