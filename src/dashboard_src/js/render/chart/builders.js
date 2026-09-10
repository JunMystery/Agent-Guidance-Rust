// SVG string builders for Glass Stream Wave & Heatmap Ribbon chart.

import { fmtTokens, fmtPct } from '../../format.js';
import { slotCenter, CHART_W, PAD } from './scale.js';
import { buildSplinePath, buildSplineAreaPath } from './spline.js';
import { t } from '../../i18n/index.js';

export function buildDefs(hours) {
  const n = hours.length;
  let heatStops = '';
  hours.forEach((h, i) => {
    const pct = n > 1 ? (i / (n - 1)) * 100 : 0;
    const calls = h.calls || 0;
    const lat = h.avg_duration_ms || 0;
    let color = '#334155';
    if (calls > 0) {
      if (lat < 150) color = '#10b981';
      else if (lat < 500) color = '#06b6d4';
      else if (lat < 1000) color = '#f59e0b';
      else color = '#ef4444';
    }
    heatStops += `<stop offset="${pct.toFixed(1)}%" stop-color="${color}" />`;
  });

  return '<defs>' +
    '<linearGradient id="grad-glass-stream" x1="0" y1="0" x2="0" y2="1">' +
      '<stop offset="0%" stop-color="#8b5cf6" stop-opacity="0.45"/>' +
      '<stop offset="60%" stop-color="#3b82f6" stop-opacity="0.18"/>' +
      '<stop offset="100%" stop-color="#3b82f6" stop-opacity="0.0"/>' +
    '</linearGradient>' +
    '<linearGradient id="grad-latency-stream" x1="0" y1="0" x2="0" y2="1">' +
      '<stop offset="0%" stop-color="#38bdf8" stop-opacity="0.35"/>' +
      '<stop offset="100%" stop-color="#38bdf8" stop-opacity="0.0"/>' +
    '</linearGradient>' +
    '<linearGradient id="heat-ribbon-grad" x1="0" y1="0" x2="1" y2="0">' +
      (heatStops || '<stop offset="0%" stop-color="#10b981"/><stop offset="100%" stop-color="#38bdf8"/>') +
    '</linearGradient>' +
    '<filter id="glow-violet" x="-20%" y="-20%" width="140%" height="140%">' +
      '<feGaussianBlur stdDeviation="2.5" result="blur"/>' +
      '<feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>' +
    '</filter>' +
  '</defs>';
}

export function buildGrid(box) {
  let grid = '';
  for (let g = 0; g <= 4; g++) {
    const gy = box.y0 + (box.h / 4) * g;
    grid += '<line x1="' + box.x0 + '" y1="' + gy.toFixed(1) + '" x2="' + (CHART_W - PAD.right) + '" y2="' + gy.toFixed(1) + '" class="chart-grid" />';
  }
  return grid;
}

export function buildAxes(box, maxCallsDomain, maxLatencyDomain) {
  let axes = '';
  for (let step = 0; step <= 4; step++) {
    const y = box.y0 + (box.h / 4) * step;
    const callsVal = maxCallsDomain * (1 - step / 4);
    const latVal = (maxLatencyDomain || 0) * (1 - step / 4);
    axes += '<text x="' + (box.x0 - 8) + '" y="' + (y + 3).toFixed(1) + '" class="axis-label axis-left" fill="var(--text-muted)">' + Math.round(callsVal) + '</text>';
    axes += '<text x="' + (CHART_W - PAD.right + 8) + '" y="' + (y + 3).toFixed(1) + '" class="axis-label axis-right" fill="#38bdf8">' + Math.round(latVal) + 'ms</text>';
  }
  return axes;
}

export function buildWaves(hours, box, yCalls, yLatency) {
  const n = hours.length;
  const ptsCalls = hours.map((h, i) => ({
    x: slotCenter(i, n, box),
    y: yCalls(h.calls || 0)
  }));

  const ptsLatency = hours.map((h, i) => ({
    x: slotCenter(i, n, box),
    y: (yLatency || yCalls)(h.avg_duration_ms || 0)
  }));

  const areaCalls = '<path d="' + buildSplineAreaPath(ptsCalls, box.yMax, box.y0) + '" fill="url(#grad-glass-stream)" class="stream-area" />';
  const strokeCalls = '<path d="' + buildSplinePath(ptsCalls, box.yMax, box.y0) + '" fill="none" stroke="#a78bfa" stroke-width="2.5" filter="url(#glow-violet)" class="stream-line-saved" />';
  const strokeLatency = '<path d="' + buildSplinePath(ptsLatency, box.yMax, box.y0) + '" fill="none" stroke="#38bdf8" stroke-width="1.8" stroke-dasharray="4 3" class="stream-line-opt" />';

  return areaCalls + strokeLatency + strokeCalls;
}

export function buildHeatRibbon(hours, box) {
  const ry = box.yMax + 8;
  const mainBar = '<rect x="' + box.x0 + '" y="' + ry + '" width="' + box.w + '" height="6" rx="3" fill="url(#heat-ribbon-grad)" class="heat-ribbon-bar" />';

  const n = hours.length;
  let notches = '';
  hours.forEach((h, i) => {
    const x = slotCenter(i, n, box);
    notches += '<circle cx="' + x.toFixed(1) + '" cy="' + (ry + 3) + '" r="1.2" fill="#fff" opacity="0.5" />';
  });

  return '<g class="chart-heat-ribbon">' + mainBar + notches + '</g>';
}

export function formatHour(h, i) {
  const ts = typeof h?.timestamp === 'number' ? h.timestamp : (typeof h?.hour === 'number' ? h.hour : null);
  if (ts !== null) {
    const d = new Date(ts * 1000);
    return String(d.getHours()).padStart(2, '0') + ':00';
  }
  if (typeof h?.hour === 'string' && h.hour.includes(':')) {
    return h.hour;
  }
  const now = new Date();
  const targetHour = (now.getHours() - (23 - i) + 48) % 24;
  return String(targetHour).padStart(2, '0') + ':00';
}

export function buildXLabels(hours, box) {
  const n = hours.length;
  let labels = '';
  const step = Math.max(1, Math.floor(n / 6));
  hours.forEach((h, i) => {
    if (i % step === 0 || i === n - 1) {
      const x = slotCenter(i, n, box);
      const isLast = i === n - 1;
      const hourStr = formatHour(h, i);
      const text = (isLast && h.is_current) ? (t('chart.now') || 'NOW') : hourStr;
      const cls = (isLast && h.is_current) ? 'chart-xlabel font-bold is-now' : 'chart-xlabel';
      labels += '<text x="' + x.toFixed(1) + '" y="' + (box.yMax + 28) + '" class="' + cls + '" data-hour="' + hourStr + '">' + text + '</text>';
    }
  });
  return labels;
}

export function buildHoverOverlay(hours, box, yCalls, yLatency) {
  const n = hours.length;
  const slotW = box.w / Math.max(1, n - 1);
  let overlay = '<g class="chart-interactive-layer">';
  overlay += '<line id="crosshair-line" x1="0" y1="' + box.y0 + '" x2="' + (box.yMax + 14) + '" class="chart-crosshair hidden" />';
  overlay += '<circle id="crosshair-dot-payload" cx="0" cy="0" r="5" class="crosshair-dot dot-payload hidden" />';
  overlay += '<circle id="crosshair-dot-opt" cx="0" cy="0" r="3.5" class="crosshair-dot dot-opt hidden" />';

  hours.forEach((h, i) => {
    const x = slotCenter(i, n, box);
    const yCallsPt = yCalls(h.calls || 0);
    const yLatencyPt = (yLatency || yCalls)(h.avg_duration_ms || 0);
    const rx = x - slotW / 2;
    const calls = h.calls || 0;
    const latency = h.avg_duration_ms || 0;
    const opt = h.tokens_optimized ?? h.optimized ?? 0;
    const tipJson = JSON.stringify({
      hour: formatHour(h, i),
      calls,
      avg_duration_ms: latency,
      opt,
      is_current: h.is_current,
    }).replace(/"/g, '&quot;');

    overlay += '<rect x="' + rx.toFixed(1) + '" y="' + box.y0 + '" width="' + slotW.toFixed(1) + '" height="' + (box.h + 20) + '" fill="transparent" class="slot-hitbox" data-slot="' + i + '" data-cx="' + x.toFixed(1) + '" data-ypayload="' + yCallsPt.toFixed(1) + '" data-yopt="' + yLatencyPt.toFixed(1) + '" data-meta="' + tipJson + '" />';
  });
  overlay += '</g>';
  return overlay;
}
