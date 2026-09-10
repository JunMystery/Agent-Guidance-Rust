// Magnetic crosshair scrubber & rich glass card tooltip.

import { fmtTokens } from '../../format.js';
import { t } from '../../i18n/index.js';

export function bindChartTooltip(scope) {
  const tip = scope.querySelector('#chart-tip');
  const line = scope.querySelector('#crosshair-line');
  const dotPayload = scope.querySelector('#crosshair-dot-payload') || scope.querySelector('#crosshair-dot-saved');
  const dotOpt = scope.querySelector('#crosshair-dot-opt');
  const wrap = scope.querySelector('.chart-wrap');
  if (!tip || !wrap) return;

  const hitboxes = scope.querySelectorAll('.slot-hitbox');

  hitboxes.forEach(hitbox => {
    const show = () => {
      const cx = parseFloat(hitbox.getAttribute('data-cx'));
      const yPayload = parseFloat(hitbox.getAttribute('data-ypayload') || hitbox.getAttribute('data-ysaved'));
      const yOpt = parseFloat(hitbox.getAttribute('data-yopt'));
      const meta = JSON.parse(hitbox.getAttribute('data-meta') || '{}');

      // Update crosshair & dots
      if (line) {
        line.setAttribute('x1', cx);
        line.setAttribute('x2', cx);
        line.classList.remove('hidden');
      }
      if (dotPayload) {
        dotPayload.setAttribute('cx', cx);
        dotPayload.setAttribute('cy', yPayload);
        dotPayload.classList.remove('hidden');
      }
      if (dotOpt) {
        dotOpt.setAttribute('cx', cx);
        dotOpt.setAttribute('cy', yOpt);
        dotOpt.classList.remove('hidden');
      }

      // Populate rich glass tooltip
      const hourTitle = meta.is_current ? `${meta.hour} ${t('chart.current_suffix')}` : `${meta.hour}`;
      const calls = meta.calls || 0;
      const latency = meta.avg_duration_ms || 0;
      const opt = meta.opt || 0;

      tip.innerHTML = '<div class="glass-tip-inner">' +
        '<div class="tip-header"><span>' + hourTitle + '</span><span class="tip-badge font-mono">' + latency + 'ms</span></div>' +
        '<div class="tip-row"><span class="tip-label">' + t('chart.tip_calls') + '</span><span class="tip-val tip-saved font-bold">' + calls + ' ' + t('chart.calls_unit') + '</span></div>' +
        '<div class="tip-row"><span class="tip-label">' + t('chart.tip_latency') + '</span><span class="tip-val font-mono">' + latency + 'ms</span></div>' +
        '<div class="tip-row"><span class="tip-label">' + t('chart.tip_payload') + '</span><span class="tip-val font-mono text-muted">' + fmtTokens(opt) + '</span></div>' +
        '</div>';
      tip.style.opacity = '1';
    };

    const move = (e) => {
      const r = wrap.getBoundingClientRect();
      let left = e.clientX - r.left + 14;
      let top = e.clientY - r.top - 20;
      if (left + 170 > r.width) left -= 185;
      if (top < 10) top = 10;
      tip.style.left = left + 'px';
      tip.style.top = top + 'px';
    };

    const hide = () => {
      if (line) line.classList.add('hidden');
      if (dotPayload) dotPayload.classList.add('hidden');
      if (dotOpt) dotOpt.classList.add('hidden');
      tip.style.opacity = '0';
    };

    hitbox.addEventListener('mouseenter', show);
    hitbox.addEventListener('mousemove', move);
    hitbox.addEventListener('mouseleave', hide);
  });
}
