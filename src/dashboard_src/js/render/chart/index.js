// Glass Stream Wave & Heatmap Ribbon chart orchestrator.
// Public API: renderHourlyChart(data, totals).

import { el } from '../../dom.js';
import { plotBox, maxOf, roundUpDomain, linScale, CHART_W, CHART_H } from './scale.js';
import {
  buildDefs, buildGrid, buildAxes, buildWaves, buildHeatRibbon, buildXLabels, buildHoverOverlay,
} from './builders.js';
import { buildKpi, buildLegend } from './kpi.js';
import { bindChartTooltip } from './tooltip.js';
import { t } from '../../i18n/index.js';

export function renderHourlyChart(data, totals) {
  const chart = el('hourly-chart');
  if (!chart) return;

  const rawHours = (data.hourly_savings || []);
  const hours = rawHours.map((h, i) => {
    const calls = h.calls ?? 0;
    const latency = h.avg_duration_ms ?? 0;
    const opt = h.tokens_optimized ?? h.optimized ?? 0;
    const orig = h.tokens_original ?? h.original ?? 0;
    const saved = h.tokens_saved ?? h.saved ?? (orig - opt);
    const ts = typeof h.timestamp === 'number' ? h.timestamp : (typeof h.hour === 'number' ? h.hour : null);
    const isCurrent = Boolean(h.is_current ?? (i === rawHours.length - 1));
    return {
      ...h,
      calls,
      avg_duration_ms: latency,
      original: orig,
      optimized: opt,
      saved,
      timestamp: ts,
      is_current: isCurrent,
    };
  });

  const maxCalls = maxOf(hours, ['calls']);
  const maxLatency = maxOf(hours, ['avg_duration_ms']);
  const maxPayload = maxOf(hours, ['optimized', 'saved', 'original']);

  if (!hours.length || (maxCalls === 0 && maxLatency === 0 && maxPayload === 0)) {
    chart.innerHTML = `<div class="chart-empty">${t('chart.no_data')}</div>`;
    return;
  }

  const maxDomain = roundUpDomain(maxCalls > 0 ? maxCalls : 10);
  const maxLatencyDomain = roundUpDomain(maxLatency > 0 ? maxLatency : 100);
  chart.innerHTML = buildChartHtml(hours, maxDomain, maxLatencyDomain, totals);
  bindChartTooltip(chart);
}

function buildChartHtml(hours, maxDomain, maxLatencyDomain, t) {
  const box = plotBox();
  const yVal = linScale(maxDomain, box.y0, box.yMax);
  const yLatency = linScale(maxLatencyDomain, box.y0, box.yMax);

  const defs = buildDefs(hours);
  const grid = buildGrid(box);
  const axes = buildAxes(box, maxDomain, maxLatencyDomain);
  const waves = buildWaves(hours, box, yVal, yLatency);
  const ribbon = buildHeatRibbon(hours, box);
  const xlabels = buildXLabels(hours, box);
  const overlay = buildHoverOverlay(hours, box, yVal, yLatency);

  const svg = '<svg viewBox="-10 -5 ' + (CHART_W + 20) + ' ' + (CHART_H + 20) + '" class="glass-stream-chart" preserveAspectRatio="xMidYMid meet">' +
    defs + grid + axes + waves + ribbon + xlabels + overlay + '</svg>';

  return buildKpi(t, hours) + buildLegend() + '<div class="chart-wrap">' + svg + '<div class="chart-tip" id="chart-tip"></div></div>';
}
