// KPI + legend blocks for Glass Stream chart.

import { fmtTokens, fmtPct } from '../../format.js';
import { t } from '../../i18n/index.js';

export function buildKpi(tot, hours = []) {
  const peakVelocity = hours.reduce((m, h) => Math.max(m, h.optimized || h.saved || 0), 0);
  const orig = tot.tokens_original || 0;
  const opt = tot.tokens_optimized || 0;
  const calls = tot.tool_calls || 0;
  const avgPayload = calls > 0 ? Math.round(opt / calls) : 0;
  const saved = tot.token_savings || (orig - opt);
  const pct = tot.savings_pct !== undefined ? tot.savings_pct : (orig > 0 ? (saved / orig) * 100 : 0);

  return '<div class="savings-kpi">' +
    '<div class="kpi"><span class="kpi-label">' + t('chart.context_emitted') + '</span><span class="kpi-val kpi-opt">' + fmtTokens(opt) + '</span></div>' +
    '<div class="kpi"><span class="kpi-label">' + t('chart.avg_payload') + '</span><span class="kpi-val">' + fmtTokens(avgPayload) + '</span></div>' +
    '<div class="kpi kpi-velocity"><span class="kpi-label">' + t('chart.peak_velocity') + '</span><span class="kpi-val kpi-peak">⚡ ' + fmtTokens(peakVelocity) + t('chart.per_hr') + '</span></div>' +
    '<div class="kpi"><span class="kpi-label">' + t('chart.net_saved') + '</span><span class="kpi-val kpi-saved">' + fmtTokens(saved) + ' (' + fmtPct(pct) + ')</span></div>' +
    '</div>';
}

export function buildLegend() {
  return '<div class="chart-legend">' +
    '<span class="lg-item"><span class="lg-swatch lg-wave-saved"></span>' + t('chart.legend_payload_wave') + '</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-line-opt"></span>' + t('chart.legend_opt_payload') + '</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-line-saved-accent"></span>' + t('chart.legend_net_saved') + '</span>' +
    '<span class="lg-item"><span class="lg-swatch lg-heat-ribbon"></span>' + t('chart.legend_heat_ribbon') + '</span>' +
    '</div>';
}
