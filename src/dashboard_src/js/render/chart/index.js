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

  const hours = (data.hourly_savings || []);
  const maxRaw = maxOf(hours, ['saved', 'original', 'optimized']);

  if (!hours.length || maxRaw === 0) {
    chart.innerHTML = `<div class="chart-empty">${t('chart.no_data')}</div>`;
    return;
  }

  const maxDomain = roundUpDomain(maxRaw);
  chart.innerHTML = buildChartHtml(hours, maxDomain, totals);
  bindChartTooltip(chart);
}

function buildChartHtml(hours, maxDomain, t) {
  const box = plotBox();
  const yVal = linScale(maxDomain, box.y0, box.yMax);

  const defs = buildDefs(hours);
  const grid = buildGrid(box);
  const axes = buildAxes(box, maxDomain);
  const waves = buildWaves(hours, box, yVal);
  const ribbon = buildHeatRibbon(hours, box);
  const xlabels = buildXLabels(hours, box);
  const overlay = buildHoverOverlay(hours, box, yVal);

  const svg = '<svg viewBox="-10 -5 ' + (CHART_W + 20) + ' ' + (CHART_H + 20) + '" class="glass-stream-chart" preserveAspectRatio="xMidYMid meet">' +
    defs + grid + axes + waves + ribbon + xlabels + overlay + '</svg>';

  return buildKpi(t, hours) + buildLegend() + '<div class="chart-wrap">' + svg + '<div class="chart-tip" id="chart-tip"></div></div>';
}
