import { drawArrowhead } from './graphEdges.js';

export const calcFileNodeRadius = (loc = 0, syms = 0) =>
  Math.max(12, Math.min(34, Math.round((10 + Math.sqrt(Math.max(0, Number(loc) || 0)) * 0.65 + Math.log2(Math.max(0, Number(syms) || 0) + 1) * 2.0) * 10) / 10));

const detectFileLang = (file = '') => ({
  color: ({ rs: '#f97316', ts: '#38bdf8', tsx: '#38bdf8', js: '#38bdf8', py: '#10b981', go: '#06b6d4', cpp: '#8b5cf6', c: '#8b5cf6' })[file.split('.').pop().toLowerCase()] || '#94a3b8'
});

export function initFileGraphCanvas(canvasId, rawNodes = [], rawEdges = [], onSelect, onZoom) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return { zoomIn: () => {}, zoomOut: () => {}, resetFit: () => {}, selectNode: () => {}, clearIsolate: () => {}, setPulse: () => {}, destroy: () => {} };

  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);

  let W = rect.width || 700, H = rect.height || 540;
  const nodeMap = new Map();

  rawNodes.forEach((n) => {
    const deg = (n.in_degree || 0) + (n.out_degree || 0);
    const radius = calcFileNodeRadius(n.loc, n.total_symbols ?? n.symbol_count ?? 0);
    nodeMap.set(n.id, {
      ...n, deg, isOrphan: deg === 0, radius,
      langColor: detectFileLang(n.file || n.id).color, x: W / 2, y: H / 2
    });
  });

  const nodeArr = Array.from(nodeMap.values());
  const connectedNodes = nodeArr.filter(n => !n.isOrphan);
  const orphanNodes = nodeArr.filter(n => n.isOrphan);

  connectedNodes.forEach((n, idx) => {
    const theta = idx * 2.399963, dist = 70 + Math.sqrt(idx + 1) * 125;
    n.x = W / 2 + Math.cos(theta) * dist; n.y = H / 2 + Math.sin(theta) * dist;
  });

  orphanNodes.forEach((n, idx) => {
    const theta = idx * 2.399963, dist = 750 + Math.sqrt(idx + 1) * 35;
    n.x = W / 2 + Math.cos(theta) * dist;
    n.y = H / 2 + Math.sin(theta) * dist;
  });

  const edgeList = rawEdges.map((e, idx) => ({ ...e, offset: (idx * 0.17) % 1.0 }));

  // Relax positions: 120px node repulsion & 300px edge distance
  for (let iter = 0; iter < 50; iter++) {
    for (let i = 0; i < nodeArr.length; i++) {
      for (let j = i + 1; j < nodeArr.length; j++) {
        const a = nodeArr[i], b = nodeArr[j];
        if (a.isOrphan && b.isOrphan) continue;
        const dx = b.x - a.x, dy = b.y - a.y, dist = Math.hypot(dx, dy) || 1;
        const minDist = a.radius + b.radius + 120;
        if (dist < minDist) {
          const force = ((minDist - dist) / dist) * 0.42;
          a.x -= dx * force; a.y -= dy * force;
          b.x += dx * force; b.y += dy * force;
        }
      }
    }
    edgeList.forEach(e => {
      const src = nodeMap.get(e.source), tgt = nodeMap.get(e.target);
      if (!src || !tgt) return;
      const dx = tgt.x - src.x, dy = tgt.y - src.y, dist = Math.hypot(dx, dy) || 1;
      const force = (dist - 300) * 0.012;
      src.x += (dx / dist) * force; src.y += (dy / dist) * force;
      tgt.x -= (dx / dist) * force; tgt.y -= (dy / dist) * force;
    });
  }

  const resizeObs = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const cr = entry.contentRect;
      if (cr.width > 20 && cr.height > 20 && (Math.abs(W - cr.width) > 2 || Math.abs(H - cr.height) > 2)) {
        W = cr.width; H = cr.height;
        canvas.width = Math.round(W * dpr); canvas.height = Math.round(H * dpr);
        ctx.scale(dpr, dpr);
      }
    }
  });
  if (canvas.parentElement) resizeObs.observe(canvas.parentElement);

  let scale = 0.85, panX = 0, panY = 0;
  let isDragging = false, hasDragged = false, startX = 0, startY = 0, lastX = 0, lastY = 0;
  let selectedId = null, hoverId = null, isIsolated = false;
  let pulseEnabled = true, animTime = 0, running = true;
  let hideOrphans = true;

  function computeIsolatedSet(focusId) {
    if (!focusId) return new Set();
    const s = new Set([focusId]);
    edgeList.forEach(e => {
      if (e.source === focusId) s.add(e.target);
      if (e.target === focusId) s.add(e.source);
    });
    return s;
  }

  function fitToNodes(subset) {
    const visible = subset && subset.length ? subset : nodeArr.filter(n => !(hideOrphans && n.isOrphan));
    const list = visible.length ? visible : nodeArr;
    if (!list.length) return;
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    list.forEach(n => {
      minX = Math.min(minX, n.x - n.radius);
      maxX = Math.max(maxX, n.x + n.radius);
      minY = Math.min(minY, n.y - n.radius);
      maxY = Math.max(maxY, n.y + n.radius);
    });
    const gw = Math.max(maxX - minX, 100), gh = Math.max(maxY - minY, 100);
    scale = Math.max(0.2, Math.min((W - 100) / gw, (H - 100) / gh, 1.4));
    panX = W / 2 - ((minX + maxX) / 2) * scale;
    panY = H / 2 - ((minY + maxY) / 2) * scale;
    if (onZoom) onZoom(scale);
  }

  fitToNodes();

  function render() {
    if (!running) return;
    animTime += 0.016;
    ctx.save();
    ctx.clearRect(0, 0, W, H);
    ctx.translate(panX, panY);
    ctx.scale(scale, scale);

    const isoSet = isIsolated && selectedId ? computeIsolatedSet(selectedId) : null;

    // 1. Edges
    edgeList.forEach(e => {
      const src = nodeMap.get(e.source), tgt = nodeMap.get(e.target);
      if (!src || !tgt) return;
      const inSub = isoSet ? (isoSet.has(e.source) && isoSet.has(e.target)) : true;
      const isConn = selectedId && (e.source === selectedId || e.target === selectedId);

      ctx.save();
      ctx.globalAlpha = isoSet ? (inSub ? 0.95 : 0.10) : (selectedId ? (isConn ? 0.95 : 0.10) : 0.45);
      const w = Math.min(4.5, Math.max(1.2, 1.0 + Math.log2(Math.max(1, e.weight || (e.calls ? e.calls.length : 1)))));
      ctx.lineWidth = w;
      ctx.strokeStyle = isConn ? '#00e5ff' : '#38bdf8';
      ctx.beginPath();
      ctx.moveTo(src.x, src.y);
      ctx.lineTo(tgt.x, tgt.y);
      ctx.stroke();
      drawArrowhead(ctx, src.x, src.y, tgt.x, tgt.y, tgt.radius, isConn ? '#00e5ff' : '#38bdf8', 6);
      ctx.restore();

      if (pulseEnabled && (!isoSet || inSub) && (isConn || !selectedId)) {
        const tVal = (animTime * 0.45 + e.offset) % 1.0;
        const px = src.x + (tgt.x - src.x) * tVal, py = src.y + (tgt.y - src.y) * tVal;
        ctx.save();
        ctx.beginPath();
        ctx.arc(px, py, 2.2, 0, 2 * Math.PI);
        ctx.fillStyle = '#ffffff';
        ctx.shadowColor = '#00e5ff';
        ctx.shadowBlur = 8;
        ctx.fill();
        ctx.restore();
      }
    });

    // 2. Nodes
    nodeArr.forEach(n => {
      if (hideOrphans && n.isOrphan) return;
      const inSub = isoSet ? isoSet.has(n.id) : true;
      const isSel = n.id === selectedId, isHov = n.id === hoverId;
      ctx.save();
      ctx.globalAlpha = isoSet ? (inSub ? 1.0 : 0.10) : (selectedId ? (isSel || inSub ? 1.0 : 0.10) : 1.0);

      if (isSel || isHov) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 5, 0, 2 * Math.PI);
        ctx.strokeStyle = isSel ? '#00e5ff' : 'rgba(56, 189, 248, 0.6)';
        ctx.lineWidth = 2.0;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, 2 * Math.PI);
      ctx.fillStyle = '#0f172a';
      ctx.fill();
      ctx.lineWidth = 2.0;
      ctx.strokeStyle = n.langColor;
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(n.x, n.y, Math.max(3, n.radius - 3), 0, 2 * Math.PI);
      ctx.fillStyle = isSel ? '#0284c7' : '#1e293b';
      ctx.fill();

      // LOD label rendering with contrast halo
      const showLabel = isSel || isHov || (scale >= 0.7 && (n.deg || 0) >= 2) || (scale >= 0.45 && (n.deg || 0) >= 6) || (scale >= 1.0);
      if (showLabel) {
        const name = n.label || (n.file ? n.file.split(/[/\\]/).pop() : n.id);
        ctx.font = '600 11px Inter, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';

        // Dark halo behind text for readability
        ctx.lineWidth = 3;
        ctx.strokeStyle = '#080c14';
        ctx.strokeText(name, n.x, n.y + n.radius + 5);

        ctx.fillStyle = isSel ? '#38bdf8' : (isHov ? '#00e5ff' : '#e2e8f0');
        ctx.fillText(name, n.x, n.y + n.radius + 5);

        if (scale > 0.8 || isSel || isHov) {
          ctx.font = '400 9px monospace';
          ctx.strokeStyle = '#080c14';
          ctx.lineWidth = 2.5;
          ctx.strokeText(`${n.loc || 0}L • In:${n.in_degree || 0} Out:${n.out_degree || 0}`, n.x, n.y + n.radius + 18);
          ctx.fillStyle = '#94a3b8';
          ctx.fillText(`${n.loc || 0}L • In:${n.in_degree || 0} Out:${n.out_degree || 0}`, n.x, n.y + n.radius + 18);
        }
      }
      ctx.restore();
    });

    ctx.restore();
    requestAnimationFrame(render);
  }

  requestAnimationFrame(render);

  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.14 : 0.88;
    const newScale = Math.min(3.5, Math.max(0.15, scale * factor));
    const cr = canvas.getBoundingClientRect();
    const mx = e.clientX - cr.left, my = e.clientY - cr.top;
    panX = mx - (mx - panX) * (newScale / scale);
    panY = my - (my - panY) * (newScale / scale);
    scale = newScale;
    if (onZoom) onZoom(scale);
  }, { passive: false });

  canvas.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    isDragging = true; hasDragged = false;
    startX = e.clientX; startY = e.clientY;
    lastX = e.clientX; lastY = e.clientY;
  });

  window.addEventListener('mousemove', (e) => {
    if (isDragging) {
      if (Math.hypot(e.clientX - startX, e.clientY - startY) > 4) hasDragged = true;
      panX += e.clientX - lastX; panY += e.clientY - lastY;
      lastX = e.clientX; lastY = e.clientY;
    }
    const cr = canvas.getBoundingClientRect();
    const worldX = (e.clientX - cr.left - panX) / scale, worldY = (e.clientY - cr.top - panY) / scale;
    const found = nodeArr.find(n => !(hideOrphans && n.isOrphan) && Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 4.5);
    hoverId = found ? found.id : null;
    canvas.style.cursor = found ? 'pointer' : (isDragging ? 'grabbing' : 'grab');
  });

  window.addEventListener('mouseup', () => { isDragging = false; });

  canvas.addEventListener('click', (e) => {
    if (hasDragged) return;
    const cr = canvas.getBoundingClientRect();
    const worldX = (e.clientX - cr.left - panX) / scale, worldY = (e.clientY - cr.top - panY) / scale;
    const clicked = nodeArr.find(n => !(hideOrphans && n.isOrphan) && Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 4.5);
    if (clicked) {
      selectedId = clicked.id;
      isIsolated = true;
      const subSet = computeIsolatedSet(clicked.id);
      fitToNodes(nodeArr.filter(n => subSet.has(n.id)));
      if (onSelect) onSelect(clicked, true);
    } else {
      selectedId = null;
      isIsolated = false;
      fitToNodes();
      if (onSelect) onSelect(null, false);
    }
  });

  return {
    zoomIn: () => { scale = Math.min(3.5, scale * 1.25); if (onZoom) onZoom(scale); },
    zoomOut: () => { scale = Math.max(0.15, scale * 0.8); if (onZoom) onZoom(scale); },
    resetFit: () => { isIsolated = false; selectedId = null; fitToNodes(); },
    selectNode: (id, isolate = true) => {
      selectedId = id; isIsolated = isolate;
      const n = nodeMap.get(id);
      if (n) {
        if (isolate) fitToNodes(nodeArr.filter(item => computeIsolatedSet(id).has(item.id)));
        else { panX = W / 2 - n.x * scale; panY = H / 2 - n.y * scale; }
      }
    },
    clearIsolate: () => { isIsolated = false; selectedId = null; fitToNodes(); },
    toggleOrphans: (hide) => { hideOrphans = hide; fitToNodes(); },
    setPulse: (p) => { pulseEnabled = p; },
    destroy: () => { running = false; if (resizeObs) resizeObs.disconnect(); }
  };
}
