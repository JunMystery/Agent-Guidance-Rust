import { el, activeView, pollSpanFor } from './dom.js';
import { pollBackoff } from './state.js';
import { fetchData } from './api.js';

let pollTimer = null;

function scheduleNextPoll() {
  if (pollTimer) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
  if (document.hidden) return;
  const view = activeView();
  if (view === 'graph' || view === 'logs') return;

  pollTimer = setTimeout(async () => {
    try {
      await fetchData();
    } finally {
      scheduleNextPoll();
    }
  }, pollBackoff);
}

export function startPoll() {
  stopPoll();
  const spanId = pollSpanFor(activeView());
  if (spanId) {
    const node = el(spanId);
    if (node) node.textContent = '(polling ' + (pollBackoff / 1000).toFixed(0) + 's)';
  }
  scheduleNextPoll();
}

export function stopPoll() {
  if (pollTimer) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
  const aEl = el('actions-poll');
  if (aEl) aEl.textContent = '';
  const rEl = el('recent-calls-poll');
  if (rEl) rEl.textContent = '';
  const sEl = el('skills-poll');
  if (sEl) sEl.textContent = '';
}

document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    stopPoll();
  } else {
    const view = activeView();
    if (view && view !== 'graph' && view !== 'logs') {
      fetchData().finally(() => startPoll());
    }
  }
});
