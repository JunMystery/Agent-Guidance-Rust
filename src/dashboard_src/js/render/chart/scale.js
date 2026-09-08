// Pure scale/axis math for the stream chart. No DOM, no side effects.

export const CHART_W = 760;
export const CHART_H = 220;
export const PAD = { left: 45, right: 35, top: 20, bottom: 45 };

export function plotBox() {
  return {
    w: CHART_W - PAD.left - PAD.right,
    h: CHART_H - PAD.top - PAD.bottom,
    x0: PAD.left,
    y0: PAD.top,
    yMax: CHART_H - PAD.bottom,
  };
}

export function maxOf(hours, keys) {
  return hours.reduce((m, h) => {
    keys.forEach(k => { m = Math.max(m, h[k] || 0); });
    return m;
  }, 0);
}

// Round up to neat human-readable boundaries (e.g. 10k, 25k, 50k, 100k)
export function roundUpDomain(val) {
  if (val <= 0) return 1000;
  const mag = Math.pow(10, Math.floor(Math.log10(val)));
  const norm = val / mag;
  let mult;
  if (norm <= 1.0) mult = 1.0;
  else if (norm <= 2.0) mult = 2.0;
  else if (norm <= 2.5) mult = 2.5;
  else if (norm <= 5.0) mult = 5.0;
  else mult = 10.0;
  return mult * mag;
}

export function linScale(domainMax, y0, yMax) {
  return (v) => yMax - (domainMax ? (v / domainMax) * (yMax - y0) : 0);
}

export function slotCenter(i, n, box) {
  const slot = box.w / Math.max(1, n - 1);
  return box.x0 + slot * i;
}
