import { drawArrowhead } from './graphEdges.js';
import { t } from '../i18n/index.js';

export function initFunctionDrilldownCanvas(canvasId, targetFile, rawNodes = [], rawEdges = [], onSelect, onZoom) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return { zoomIn: () => {}, zoomOut: () => {}, resetFit: () => {}, selectNode: () => {}, setPulse: () => {}, destroy: () => {} };

  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);

  const W = rect.width || 700, H = rect.height || 540;
  const isAllMode = targetFile === 'all' || !targetFile;
  const isInternal = n => !isAllMode && Boolean(n.is_internal ?? (!n.is_external || n.scope === 'internal'));
  const internalList = rawNodes.filter(isInternal);
  const externalList = rawNodes.filter(n => !isInternal(n));
  const nodeMap = new Map();

  const intCount = internalList.length;
  if (isAllMode) {
    const sorted = [...rawNodes].sort((a, b) => (a.file || '').localeCompare(b.file || ''));
    const total = sorted.length;
    const baseR = Math.min(W, H) * 0.42;
    sorted.forEach((n, idx) => {
      const radius = Math.max(12, Math.min(20, 11 + Math.sqrt(n.loc || 10) * 0.6));
      const angle = (idx / Math.max(1, total)) * 2 * Math.PI - Math.PI / 2;
      const r = baseR + (idx % 2 === 0 ? 0 : 25);
      nodeMap.set(n.id, {
        ...n,
        radius,
        isInternal: false,
        x: W / 2 + Math.cos(angle) * r,
        y: H / 2 + Math.sin(angle) * r
      });
    });
  } else {
    internalList.forEach((n, idx) => {
      const radius = Math.max(13, Math.min(22, 12 + Math.sqrt(n.loc || 10) * 0.7));
      const angle = (idx / Math.max(1, intCount)) * 2 * Math.PI;
      const dist = intCount <= 1 ? 0 : Math.min(100, 35 + Math.sqrt(idx) * 35);
      nodeMap.set(n.id, { ...n, radius, isInternal: true, x: W / 2 + Math.cos(angle) * dist, y: H / 2 + Math.sin(angle) * dist });
    });
  }

  let bMinX = W / 2, bMaxX = W / 2, bMinY = H / 2, bMaxY = H / 2;
  if (intCount > 0 && !isAllMode) {
    bMinX = Math.min(...internalList.map(n => nodeMap.get(n.id).x - nodeMap.get(n.id).radius));
    bMaxX = Math.max(...internalList.map(n => nodeMap.get(n.id).x + nodeMap.get(n.id).radius));
    bMinY = Math.min(...internalList.map(n => nodeMap.get(n.id).y - nodeMap.get(n.id).radius));
    bMaxY = Math.max(...internalList.map(n => nodeMap.get(n.id).y + nodeMap.get(n.id).radius));
  }
  const auraBox = {
    x: bMinX - 32, y: bMinY - 40,
    w: Math.max(180, (bMaxX - bMinX) + 64),
    h: Math.max(120, (bMaxY - bMinY) + 72)
  };

  if (!isAllMode) {
    const extCount = externalList.length;
    const orbitR = Math.max(auraBox.w, auraBox.h) * 0.75 + 75;
    externalList.forEach((n, idx) => {
      const radius = Math.max(12, Math.min(20, 11 + Math.sqrt(n.loc || 10) * 0.6));
      const angle = (idx / Math.max(1, extCount)) * 2 * Math.PI;
      nodeMap.set(n.id, { ...n, radius, isInternal: false, x: W / 2 + Math.cos(angle) * orbitR, y: H / 2 + Math.sin(angle) * orbitR });
    });
  }

  const nodeArr = Array.from(nodeMap.values());
  const edgeList = rawEdges.map((e, idx) => ({ ...e, offset: (idx * 0.19) % 1.0 }));

  let scale = 0.85, panX = 0, panY = 0;
  let isDragging = false, hasDragged = false, startX = 0, startY = 0, lastX = 0, lastY = 0;
  let selectedId = null, hoverId = null;
  let pulseEnabled = true, animTime = 0, running = true;

  function fitToView() {
    if (!nodeArr.length) return;
    let minX = isAllMode && nodeArr[0] ? nodeArr[0].x : auraBox.x;
    let maxX = isAllMode && nodeArr[0] ? nodeArr[0].x : auraBox.x + auraBox.w;
    let minY = isAllMode && nodeArr[0] ? nodeArr[0].y : auraBox.y;
    let maxY = isAllMode && nodeArr[0] ? nodeArr[0].y : auraBox.y + auraBox.h;
    nodeArr.forEach(n => {
      minX = Math.min(minX, n.x - n.radius - 20); maxX = Math.max(maxX, n.x + n.radius + 20);
      minY = Math.min(minY, n.y - n.radius - 20); maxY = Math.max(maxY, n.y + n.radius + 20);
    });
    scale = Math.max(0.15, Math.min((W - 100) / Math.max(maxX - minX, 120), (H - 100) / Math.max(maxY - minY, 120), 1.3));
    panX = W / 2 - ((minX + maxX) / 2) * scale;
    panY = H / 2 - ((minY + maxY) / 2) * scale;
    if (onZoom) onZoom(scale);
  }

  fitToView();

  function render() {
    if (!running) return;
    animTime += 0.016;
    ctx.save();
    ctx.clearRect(0, 0, W, H);
    ctx.translate(panX, panY);
    ctx.scale(scale, scale);

    if (intCount > 0 && !isAllMode) {
      ctx.save();
      ctx.beginPath();
      ctx.roundRect ? ctx.roundRect(auraBox.x, auraBox.y, auraBox.w, auraBox.h, 14) : ctx.rect(auraBox.x, auraBox.y, auraBox.w, auraBox.h);
      ctx.fillStyle = 'rgba(2, 132, 199, 0.08)'; ctx.fill();
      ctx.lineWidth = 1.5; ctx.strokeStyle = 'rgba(56, 189, 248, 0.5)'; ctx.setLineDash([6, 5]); ctx.stroke();
      ctx.setLineDash([]);
      ctx.font = '700 11px Inter, sans-serif'; ctx.fillStyle = '#38bdf8'; ctx.textAlign = 'left';
      ctx.fillText(`📁 ${t('graph.file_label')}: ${(targetFile || 'FILE').split(/[/\\]/).pop()} (${intCount} functions)`, auraBox.x + 14, auraBox.y + 18);
      ctx.restore();
    }

    edgeList.forEach(e => {
      const src = nodeMap.get(e.source), tgt = nodeMap.get(e.target);
      if (!src || !tgt) return;
      const isOut = e.direction === 'outgoing', isIn = e.direction === 'incoming';
      const edgeColor = isOut ? '#10b981' : (isIn ? '#a855f7' : '#00e5ff');
      const isConn = selectedId && (e.source === selectedId || e.target === selectedId);

      ctx.save();
      ctx.globalAlpha = selectedId ? (isConn ? 1.0 : 0.10) : 0.75;
      ctx.lineWidth = isConn ? 2.4 : 1.5;
      ctx.strokeStyle = edgeColor;
      ctx.beginPath();
      ctx.moveTo(src.x, src.y);
      ctx.lineTo(tgt.x, tgt.y);
      ctx.stroke();
      drawArrowhead(ctx, src.x, src.y, tgt.x, tgt.y, tgt.radius, edgeColor, 7);
      ctx.restore();

      if (pulseEnabled && (!selectedId || isConn)) {
        const tVal = (animTime * 0.4 + e.offset) % 1.0;
        const px = src.x + (tgt.x - src.x) * tVal, py = src.y + (tgt.y - src.y) * tVal;
        ctx.save();
        ctx.beginPath();
        ctx.arc(px, py, 2.2, 0, 2 * Math.PI);
        ctx.fillStyle = '#ffffff'; ctx.shadowColor = edgeColor; ctx.shadowBlur = 8; ctx.fill();
        ctx.restore();
      }
    });

    nodeArr.forEach(n => {
      const isSel = n.id === selectedId, isHov = n.id === hoverId;
      const isConn = selectedId && edgeList.some(e => (e.source === selectedId && e.target === n.id) || (e.target === selectedId && e.source === n.id));

      ctx.save();
      ctx.globalAlpha = selectedId ? (isSel || isConn ? 1.0 : 0.10) : 1.0;

      if (isSel || isHov) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 4.5, 0, 2 * Math.PI);
        ctx.strokeStyle = isSel ? '#00e5ff' : 'rgba(56, 189, 248, 0.6)'; ctx.lineWidth = 2.0; ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, 2 * Math.PI);
      ctx.fillStyle = n.isInternal ? '#0f172a' : '#1e1b4b'; ctx.fill();
      ctx.lineWidth = 1.8; ctx.strokeStyle = n.isInternal ? '#38bdf8' : '#a855f7'; ctx.stroke();

      ctx.beginPath();
      ctx.arc(n.x, n.y, Math.max(3, n.radius - 3), 0, 2 * Math.PI);
      ctx.fillStyle = isSel ? '#0284c7' : (n.isInternal ? '#0369a1' : '#6b21a8'); ctx.fill();

      ctx.font = '600 11px monospace'; ctx.fillStyle = isSel ? '#00e5ff' : '#f1f5f9';
      ctx.textAlign = 'center'; ctx.textBaseline = 'top';
      ctx.fillText(n.label || n.id, n.x, n.y + n.radius + 4);

      if ((!n.isInternal || isAllMode) && n.file) {
        ctx.font = '500 9px monospace'; ctx.fillStyle = '#a855f7';
        ctx.fillText(`[${n.file.split(/[/\\]/).pop()}]`, n.x, n.y + n.radius + 17);
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
    startX = e.clientX; startY = e.clientY; lastX = e.clientX; lastY = e.clientY;
  });

  window.addEventListener('mousemove', (e) => {
    if (isDragging) {
      if (Math.hypot(e.clientX - startX, e.clientY - startY) > 4) hasDragged = true;
      panX += e.clientX - lastX; panY += e.clientY - lastY;
      lastX = e.clientX; lastY = e.clientY;
    }
    const cr = canvas.getBoundingClientRect();
    const worldX = (e.clientX - cr.left - panX) / scale, worldY = (e.clientY - cr.top - panY) / scale;
    let found = null;
    for (const n of nodeArr) {
      if (Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 4.5) { found = n; break; }
    }
    hoverId = found ? found.id : null;
    canvas.style.cursor = found ? 'pointer' : (isDragging ? 'grabbing' : 'grab');
  });

  window.addEventListener('mouseup', () => { isDragging = false; });

  canvas.addEventListener('click', (e) => {
    if (hasDragged) return;
    const cr = canvas.getBoundingClientRect();
    const worldX = (e.clientX - cr.left - panX) / scale, worldY = (e.clientY - cr.top - panY) / scale;
    let clicked = null;
    for (const n of nodeArr) {
      if (Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 4.5) { clicked = n; break; }
    }
    if (clicked) {
      selectedId = (selectedId === clicked.id) ? null : clicked.id;
      if (onSelect) onSelect(selectedId ? clicked : null);
    } else {
      selectedId = null;
      fitToView();
      if (onSelect) onSelect(null);
    }
  });

  return {
    zoomIn: () => { scale = Math.min(3.5, scale * 1.25); if (onZoom) onZoom(scale); },
    zoomOut: () => { scale = Math.max(0.15, scale * 0.8); if (onZoom) onZoom(scale); },
    resetFit: fitToView,
    selectNode: (id) => {
      selectedId = id;
      const n = nodeMap.get(id);
      if (n) { panX = W / 2 - n.x * scale; panY = H / 2 - n.y * scale; }
    },
    setPulse: (p) => { pulseEnabled = p; },
    destroy: () => { running = false; }
  };
}
