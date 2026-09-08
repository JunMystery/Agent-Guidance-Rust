// KPI + legend blocks for Glass Stream chart.

import { fmtTokens, fmtPct } from '../../format.js';

export function buildKpi(t, hours = []) {
  const peakVelocity = hours.reduce((m, h) => Math.max(m, h.saved || 0), 0);
  const orig = t.tokens_original || 0;
  const opt = t.tokens_optimized || 0;
  const saved = t.token_savings || (orig - opt);
  const pct = t.savings_pct !== undefined ? t.savings_pct : (orig > 0 ? (saved / orig) * 100 : 0);

  return '<div class="savings-kpi">' +
    '<div class="kpi"><span class="kpi-label">Original Tokens</span><span class="kpi-val">' + fmtTokens(orig) + '</span></div>' +
    '<div class="kpi"><span class="kpi-label">Optimized Payload</span><span class="kpi-val kpi-opt">' + fmtTokens(opt) + '</span></div>' +
    '<div class="kpi"><span class="kpi-label">Net Saved</span><span class="kpi-val kpi-saved">' + fmtTokens(saved) + ' (' + fmtPct(pct) + ')</span></div>' +
    '<div class="kpi kpi-velocity"><span class="kpi-label">Peak Velocity</span><span class="kpi-val kpi-peak">⚡ ' + fmtTokens(peakVelocity) + '/hr</span></div>' +
    '</div>';
}

export function buildLegend() {
  return '<div class="chart-legend">' +
    '<span class="lg-item"><span class="lg-swatch lg-wave-saved"></span>Payload Wave</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-line-opt"></span>Optimized Payload</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-line-saved-accent"></span>Net Saved</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-heat-ribbon"></span>Efficiency Heat Ribbon</span>' +
    '</div>';
}
