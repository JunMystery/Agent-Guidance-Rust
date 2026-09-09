import { setText } from '../dom.js';
import { timeAgo, fmtDuration } from '../format.js';
import { t } from '../i18n/index.js';

export function renderHealthPanel(h, embedQueries) {
  const modelLoaded = h.model_loaded !== undefined ? h.model_loaded : null;
  const clients = h.clients !== undefined ? h.clients : null;

  const daemonStatus = h.status === 'ok' ? t('status.running') : t('status.stopped');
  setText('sys-daemon', daemonStatus);
  const daemonEl = document.getElementById('sys-daemon');
  if (daemonEl) {
    daemonEl.className = h.status === 'ok' ? 'hud-node-status status-ok' : 'hud-node-status status-err';
  }

  const modelStatus = modelLoaded === null ? t('status.unknown') : (modelLoaded ? t('status.loaded') : t('status.unloaded'));
  setText('sys-model', modelStatus);
  const modelEl = document.getElementById('sys-model');
  if (modelEl) {
    modelEl.className = modelLoaded ? 'hud-node-status status-ok' : 'hud-node-status status-warn';
  }

  setText('sys-clients', clients === null ? '0' : String(clients));

  setText('out-daemon-status', daemonStatus);
  setText('out-model-status', modelStatus);
  setText('out-engine', h.engine || t('status.unknown'));
  if (embedQueries !== undefined && embedQueries !== null) {
    setText('out-embed-queries', String(embedQueries));
    setText('out-embed-metrics-queries', String(embedQueries));
  }
  setText('out-uptime', h.uptime_seconds ? fmtDuration(h.uptime_seconds) : '--');
  setText('out-clients', clients === null ? '0' : String(clients));
  setText('out-last-embed', h.last_embed_time ? timeAgo(h.last_embed_time) : t('status.active'));

  setText('sys-embed-model', modelStatus);
  setText('sys-embed-daemon', daemonStatus);
  setText('sys-embed-clients', clients === null ? '0' : String(clients));
  setText('sys-embed-backend', h.backend || h.engine || 'candle-bert');
}
