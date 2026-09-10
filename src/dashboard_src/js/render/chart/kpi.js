// KPI + legend blocks for Glass Stream chart.

import { fmtTokens, fmtPct } from '../../format.js';
import { t } from '../../i18n/index.js';

export function buildKpi(tot, hours = []) {
  const totalCalls = hours.reduce((m, h) => m + (h.calls || 0), 0) || (tot.tool_calls || 0);
  const activeHours = hours.filter(h => (h.calls || 0) > 0);
  const avgLatency = activeHours.length
    ? Math.round(activeHours.reduce((m, h) => m + (h.avg_duration_ms || 0), 0) / activeHours.length)
    : 0;
  const peakCalls = hours.reduce((m, h) => Math.max(m, h.calls || 0), 0);
  const opt = tot.tokens_optimized || hours.reduce((m, h) => m + (h.tokens_optimized || h.optimized || 0), 0);

  return '<div class="savings-kpi">' +
    '<div class="kpi"><span class="kpi-label">' + t('chart.total_invocations') + '</span><span class="kpi-val kpi-opt">' + totalCalls.toLocaleString() + '</span></div>' +
    '<div class="kpi"><span class="kpi-label">' + t('chart.avg_latency') + '</span><span class="kpi-val">' + avgLatency + 'ms</span></div>' +
    '<div class="kpi kpi-velocity"><span class="kpi-label">' + t('chart.peak_invocations') + '</span><span class="kpi-val kpi-peak">⚡ ' + peakCalls + t('chart.per_hr') + '</span></div>' +
    '<div class="kpi"><span class="kpi-label">' + t('chart.opt_payload') + '</span><span class="kpi-val kpi-saved">' + fmtTokens(opt) + '</span></div>' +
    '</div>';
}

export function buildLegend() {
  return '<div class="chart-legend">' +
    '<span class="lg-item"><span class="lg-swatch lg-wave-saved"></span>' + t('chart.legend_calls_wave') + '</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-line-opt"></span>' + t('chart.legend_latency_line') + '</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-heat-ribbon"></span>' + t('chart.legend_heat_ribbon') + '</span>' +
    '</div>';
}
