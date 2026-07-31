// Pure scale/axis math for the combo chart. No DOM, no side effects.

export const CHART_W = 760;
export const CHART_H = 232;
export const PAD = { left: 38, right: 82, top: 16, bottom: 46 };

export function plotBox() {
  return {
    w: 640,
    h: 170,
    x0: 38,
    y0: 16,
    yMax: 186,
  };
}

export function maxOf(hours, keys) {
  return hours.reduce((m, h) => {
    keys.forEach(k => { m = Math.max(m, h[k] || 0); });
    return m;
  }, 0);
}

// Linear scale from [0, domainMax] into pixel space [yMax, y0].
export function linScale(domainMax, y0, yMax) {
  return (v) => yMax - (domainMax ? (v / domainMax) * (yMax - y0) : 0);
}

// Column x-center for slot index i across n slots.
export function slotCenter(i, n, box) {
  const slot = box.w / n;
  return box.x0 + slot * i + slot / 2;
}

export function columnWidth(n, box) {
  const slot = box.w / n;
  return Math.max(4, slot * 0.5);
}
