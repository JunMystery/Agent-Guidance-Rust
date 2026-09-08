// SVG string builders for Glass Stream Wave & Heatmap Ribbon chart.

import { fmtTokens, fmtPct } from '../../format.js';
import { slotCenter, CHART_W, PAD } from './scale.js';
import { buildSplinePath, buildSplineAreaPath } from './spline.js';

export function buildDefs(hours) {
  const n = hours.length;
  let heatStops = '';
  hours.forEach((h, i) => {
    const pct = n > 1 ? (i / (n - 1)) * 100 : 0;
    const orig = h.original || 0;
    const saved = h.saved || 0;
    const eff = orig > 0 ? (saved / orig) * 100 : 0;
    let color = '#334155';
    if (orig > 0) {
      if (eff >= 60) color = '#10b981';
      else if (eff >= 25) color = '#06b6d4';
      else color = '#f59e0b';
    }
    heatStops += `<stop offset="${pct.toFixed(1)}%" stop-color="${color}" />`;
  });

  return '<defs>' +
    '<linearGradient id="grad-glass-stream" x1="0" y1="0" x2="0" y2="1">' +
      '<stop offset="0%" stop-color="#8b5cf6" stop-opacity="0.45"/>' +
      '<stop offset="50%" stop-color="#3b82f6" stop-opacity="0.18"/>' +
      '<stop offset="100%" stop-color="#3b82f6" stop-opacity="0.0"/>' +
    '</linearGradient>' +
    '<linearGradient id="grad-glass-saved" x1="0" y1="0" x2="0" y2="1">' +
      '<stop offset="0%" stop-color="#10b981" stop-opacity="0.4"/>' +
      '<stop offset="100%" stop-color="#10b981" stop-opacity="0.0"/>' +
    '</linearGradient>' +
    '<linearGradient id="heat-ribbon-grad" x1="0" y1="0" x2="1" y2="0">' +
      (heatStops || '<stop offset="0%" stop-color="#3b82f6"/><stop offset="100%" stop-color="#10b981"/>') +
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

export function buildAxes(box, maxDomain) {
  let axes = '';
  for (let step = 0; step <= 4; step++) {
    const y = box.y0 + (box.h / 4) * step;
    const val = maxDomain * (1 - step / 4);
    axes += '<text x="' + (box.x0 - 8) + '" y="' + (y + 3).toFixed(1) + '" class="axis-label axis-left">' + fmtTokens(val) + '</text>';
  }
  return axes;
}

export function buildWaves(hours, box, yVal) {
  const n = hours.length;
  const ptsPayload = hours.map((h, i) => ({
    x: slotCenter(i, n, box),
    y: yVal(Math.max(h.original || 0, h.optimized || 0))
  }));

  const ptsOpt = hours.map((h, i) => ({
    x: slotCenter(i, n, box),
    y: yVal(h.optimized || 0)
  }));

  const hasSaved = hours.some(h => (h.saved || 0) > 0);
  const ptsSaved = hours.map((h, i) => ({
    x: slotCenter(i, n, box),
    y: yVal(h.saved || 0)
  }));

  const areaPayload = '<path d="' + buildSplineAreaPath(ptsPayload, box.yMax, box.y0) + '" fill="url(#grad-glass-stream)" class="stream-area" />';
  const areaSaved = hasSaved ? '<path d="' + buildSplineAreaPath(ptsSaved, box.yMax, box.y0) + '" fill="url(#grad-glass-saved)" class="stream-area-saved" opacity="0.6" />' : '';
  const strokePayload = '<path d="' + buildSplinePath(ptsPayload, box.yMax, box.y0) + '" fill="none" stroke="#a78bfa" stroke-width="2.5" filter="url(#glow-violet)" class="stream-line-saved" />';
  const strokeOpt = '<path d="' + buildSplinePath(ptsOpt, box.yMax, box.y0) + '" fill="none" stroke="#38bdf8" stroke-width="1.8" stroke-dasharray="3 3" class="stream-line-opt" />';
  const strokeSaved = hasSaved ? '<path d="' + buildSplinePath(ptsSaved, box.yMax, box.y0) + '" fill="none" stroke="#10b981" stroke-width="1.5" class="stream-line-saved-accent" />' : '';

  return areaPayload + areaSaved + strokeOpt + strokeSaved + strokePayload;
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

export function buildXLabels(hours, box) {
  const n = hours.length;
  let labels = '';
  const step = Math.max(1, Math.floor(n / 6));
  hours.forEach((h, i) => {
    if (i % step === 0 || i === n - 1) {
      const x = slotCenter(i, n, box);
      const isLast = i === n - 1;
      const text = isLast && h.is_current ? 'NOW' : (h.hour || `${i}:00`);
      const cls = isLast ? 'chart-xlabel font-bold is-now' : 'chart-xlabel';
      labels += '<text x="' + x.toFixed(1) + '" y="' + (box.yMax + 28) + '" class="' + cls + '">' + text + '</text>';
    }
  });
  return labels;
}

export function buildHoverOverlay(hours, box, yVal) {
  const n = hours.length;
  const slotW = box.w / Math.max(1, n - 1);
  let overlay = '<g class="chart-interactive-layer">';
  overlay += '<line id="crosshair-line" x1="0" y1="' + box.y0 + '" x2="0" y2="' + (box.yMax + 14) + '" class="chart-crosshair hidden" />';
  overlay += '<circle id="crosshair-dot-payload" cx="0" cy="0" r="5" class="crosshair-dot dot-payload hidden" />';
  overlay += '<circle id="crosshair-dot-opt" cx="0" cy="0" r="3.5" class="crosshair-dot dot-opt hidden" />';

  hours.forEach((h, i) => {
    const x = slotCenter(i, n, box);
    const yPayload = yVal(Math.max(h.original || 0, h.optimized || 0));
    const yOpt = yVal(h.optimized || 0);
    const rx = x - slotW / 2;
    const orig = h.original || 0;
    const saved = h.saved || 0;
    const opt = h.optimized || 0;
    const pct = orig > 0 ? (saved / orig) * 100 : 0;
    const tipJson = JSON.stringify({
      hour: h.hour || `${i}:00`,
      orig, opt, saved, pct: fmtPct(pct),
      is_current: h.is_current,
    }).replace(/"/g, '&quot;');

    overlay += '<rect x="' + rx.toFixed(1) + '" y="' + box.y0 + '" width="' + slotW.toFixed(1) + '" height="' + (box.h + 20) + '" fill="transparent" class="slot-hitbox" data-slot="' + i + '" data-cx="' + x.toFixed(1) + '" data-ypayload="' + yPayload.toFixed(1) + '" data-yopt="' + yOpt.toFixed(1) + '" data-meta="' + tipJson + '" />';
  });
  overlay += '</g>';
  return overlay;
}
