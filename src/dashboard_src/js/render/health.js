import { setText } from '../dom.js';
import { timeAgo, fmtDuration } from '../format.js';

export function renderHealthPanel(h, embedQueries) {
  const modelLoaded = h.model_loaded !== undefined ? h.model_loaded : null;
  const clients = h.clients !== undefined ? h.clients : null;

  const daemonStatus = h.status === 'ok' ? 'running' : 'stopped';
  setText('sys-daemon', daemonStatus);
  const daemonEl = document.getElementById('sys-daemon');
  if (daemonEl) {
    daemonEl.className = h.status === 'ok' ? 'hud-node-status status-ok' : 'hud-node-status status-err';
  }

  const modelStatus = modelLoaded === null ? 'unknown' : (modelLoaded ? 'loaded' : 'unloaded');
  setText('sys-model', modelStatus);
  const modelEl = document.getElementById('sys-model');
  if (modelEl) {
    modelEl.className = modelLoaded ? 'hud-node-status status-ok' : 'hud-node-status status-warn';
  }

  setText('sys-clients', clients === null ? '0' : String(clients));

  setText('out-daemon-status', daemonStatus);
  setText('out-model-status', modelStatus);
  setText('out-engine', h.engine || 'unknown');
  if (embedQueries !== undefined && embedQueries !== null) {
    setText('out-embed-queries', String(embedQueries));
    setText('out-embed-metrics-queries', String(embedQueries));
  }
  setText('out-uptime', h.uptime_seconds ? fmtDuration(h.uptime_seconds) : '--');
  setText('out-clients', clients === null ? '0' : String(clients));
  setText('out-last-embed', h.last_embed_time ? timeAgo(h.last_embed_time) : 'Active');

  setText('sys-embed-model', modelStatus);
  setText('sys-embed-daemon', h.status === 'ok' ? 'running' : 'stopped');
  setText('sys-embed-clients', clients === null ? '0' : String(clients));
  setText('sys-embed-backend', h.backend || h.engine || 'candle-bert');
}
