// Reusable Promise-based modal dialog replacing window.alert() and window.confirm().
import { el } from './dom.js';

let activeResolver = null;

const ICONS = {
  danger: '⚠️',
  info: 'ℹ️',
  success: '✓',
  warning: '⚡',
};

export function showConfirm({
  title = 'Confirmation',
  message = 'Are you sure?',
  subtext = '',
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  variant = 'danger',
} = {}) {
  return openDialog({
    title,
    message,
    subtext,
    confirmText,
    cancelText,
    variant,
    isConfirm: true,
  });
}

export function showAlert({
  title = 'Notification',
  message = '',
  subtext = '',
  okText = 'OK',
  variant = 'info',
} = {}) {
  return openDialog({
    title,
    message,
    subtext,
    confirmText: okText,
    cancelText: '',
    variant,
    isConfirm: false,
  });
}

function openDialog({ title, message, subtext, confirmText, cancelText, variant, isConfirm }) {
  closeDialog(false);

  const modal = el('dialog-modal');
  if (!modal) {
    // Fallback if modal DOM is missing
    if (isConfirm) return Promise.resolve(window.confirm(`${title}\n\n${message}`));
    window.alert(`${title}\n\n${message}`);
    return Promise.resolve(true);
  }

  const titleEl = el('dialog-title');
  const iconEl = el('dialog-icon');
  const msgEl = el('dialog-message');
  const subEl = el('dialog-subtext');
  const btnConfirm = el('dialog-btn-confirm');
  const btnCancel = el('dialog-btn-cancel');

  if (titleEl) titleEl.textContent = title;
  if (iconEl) iconEl.textContent = ICONS[variant] || 'ℹ️';
  if (msgEl) msgEl.textContent = message;
  if (subEl) {
    subEl.textContent = subtext || '';
    subEl.style.display = subtext ? 'block' : 'none';
  }

  if (btnConfirm) {
    btnConfirm.textContent = confirmText;
    btnConfirm.className = variant === 'danger' ? 'btn-danger' : (variant === 'success' ? 'btn-accent' : 'btn-primary');
  }

  if (btnCancel) {
    btnCancel.textContent = cancelText;
    btnCancel.style.display = isConfirm ? 'inline-block' : 'none';
  }

  modal.classList.add('open');
  modal.setAttribute('aria-hidden', 'false');

  return new Promise(resolve => {
    activeResolver = resolve;

    const cleanup = () => {
      window.removeEventListener('keydown', onKeyDown);
      modal.classList.remove('open');
      modal.setAttribute('aria-hidden', 'true');
      activeResolver = null;
    };

    const onKeyDown = (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cleanup();
        resolve(false);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        cleanup();
        resolve(true);
      }
    };

    window.addEventListener('keydown', onKeyDown);

    if (btnConfirm) {
      btnConfirm.onclick = () => {
        cleanup();
        resolve(true);
      };
      setTimeout(() => btnConfirm.focus(), 50);
    }

    if (btnCancel) {
      btnCancel.onclick = () => {
        cleanup();
        resolve(false);
      };
    }
  });
}

function closeDialog(result = false) {
  if (activeResolver) {
    activeResolver(result);
    activeResolver = null;
  }
  const modal = el('dialog-modal');
  if (modal) {
    modal.classList.remove('open');
    modal.setAttribute('aria-hidden', 'true');
  }
}
