// Monotone cubic spline (Fritsch-Carlson) interpolation.
// Guarantees zero undershooting/overshooting and strictly respects baselines.

export function buildSplinePath(points, yMax = Infinity, yMin = -Infinity) {
  if (!points || points.length === 0) return '';
  if (points.length === 1) return `M ${points[0].x.toFixed(1)},${points[0].y.toFixed(1)}`;
  if (points.length === 2) {
    return `M ${points[0].x.toFixed(1)},${points[0].y.toFixed(1)} L ${points[1].x.toFixed(1)},${points[1].y.toFixed(1)}`;
  }

  const n = points.length;
  const dx = [];
  const dy = [];
  const slope = [];

  for (let i = 0; i < n - 1; i++) {
    const dX = points[i + 1].x - points[i].x;
    const dY = points[i + 1].y - points[i].y;
    dx.push(dX);
    dy.push(dY);
    slope.push(dX !== 0 ? dY / dX : 0);
  }

  // Calculate tangents (Fritsch-Carlson)
  const m = new Array(n);
  m[0] = slope[0];
  m[n - 1] = slope[n - 2];

  for (let i = 1; i < n - 1; i++) {
    if (slope[i - 1] * slope[i] <= 0) {
      m[i] = 0; // Local extremum: flat tangent
    } else {
      m[i] = (slope[i - 1] + slope[i]) / 2;
    }
  }

  // Adjust tangents to preserve monotonicity
  for (let i = 0; i < n - 1; i++) {
    if (slope[i] === 0) {
      m[i] = 0;
      m[i + 1] = 0;
    } else {
      const alpha = m[i] / slope[i];
      const beta = m[i + 1] / slope[i];
      const s = alpha * alpha + beta * beta;
      if (s > 9) {
        const tau = 3 / Math.sqrt(s);
        m[i] = tau * alpha * slope[i];
        m[i + 1] = tau * beta * slope[i];
      }
    }
  }

  const clampY = (y) => Math.min(Math.max(y, yMin), yMax);

  let path = `M ${points[0].x.toFixed(1)},${clampY(points[0].y).toFixed(1)}`;
  for (let i = 0; i < n - 1; i++) {
    const p1 = points[i];
    const p2 = points[i + 1];
    const dX = dx[i];

    const cp1x = p1.x + dX / 3;
    const cp1y = clampY(p1.y + (m[i] * dX) / 3);
    const cp2x = p2.x - dX / 3;
    const cp2y = clampY(p2.y - (m[i + 1] * dX) / 3);
    const endY = clampY(p2.y);

    path += ` C ${cp1x.toFixed(1)},${cp1y.toFixed(1)} ${cp2x.toFixed(1)},${cp2y.toFixed(1)} ${p2.x.toFixed(1)},${endY.toFixed(1)}`;
  }

  return path;
}

export function buildSplineAreaPath(points, yBase, yMin = -Infinity) {
  if (!points || points.length === 0) return '';
  const curve = buildSplinePath(points, yBase, yMin);
  const first = points[0];
  const last = points[points.length - 1];
  return `${curve} L ${last.x.toFixed(1)},${yBase.toFixed(1)} L ${first.x.toFixed(1)},${yBase.toFixed(1)} Z`;
}
