// Force-directed Canvas Architecture Graph Renderer with Community Auras & Linker Pulses
import { partitionAndLayoutGraph } from './graphLayout.js';

export function initGraphCanvas(canvasId, nodes, edges, communities, onSelect, onZoom) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return { setFilter: () => {}, setPulse: () => {}, toggleOrphans: () => {}, zoomIn: () => {}, zoomOut: () => {}, resetFit: () => {} };
  const ctx = canvas.getContext('2d');
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);

  const W = rect.width || 700;
  const H = rect.height || 540;

  // Run Community-Clustered ForceAtlas2 Layout
  const { nodeMap, edgeList, auras } = partitionAndLayoutGraph(nodes, edges, communities, W, H);
  const nodeArr = Array.from(nodeMap.values());

  // State
  let scale = 0.82;
  let panX = W / 2;
  let panY = H / 2;
  let isDragging = false;
  let hasDragged = false;
  let lastMouseX = 0, lastMouseY = 0, startX = 0, startY = 0;
  let selectedId = null, hoverId = null;
  let filter = 'all';
  let pulseEnabled = true;
  let hideOrphans = false;
  let animTime = 0, running = true;

  function render() {
    if (!running) return;
    animTime += 0.012;
    ctx.save();
    ctx.clearRect(0, 0, W, H);

    ctx.translate(panX, panY);
    ctx.scale(scale, scale);

    const neighborIds = new Set();
    const activeTargetId = selectedId || hoverId;
    if (activeTargetId) {
      neighborIds.add(activeTargetId);
      edgeList.forEach(e => {
        if (e.source === activeTargetId) neighborIds.add(e.target);
        if (e.target === activeTargetId) neighborIds.add(e.source);
      });
    }

    // 1. Draw Community Island Auras
    auras.forEach(a => {
      ctx.save();
      ctx.beginPath();
      ctx.arc(a.x, a.y, a.radius, 0, 2 * Math.PI);
      ctx.fillStyle = a.color + '0d';
      ctx.fill();
      ctx.strokeStyle = a.color + '38';
      ctx.setLineDash([6, 6]);
      ctx.lineWidth = 1.4;
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = a.color;
      ctx.font = '600 11px Inter, sans-serif';
      ctx.fillText(`${a.name.toUpperCase()} (${a.count})`, a.x - a.radius * 0.6, a.y - a.radius + 16);
      ctx.restore();
    });

    // 2. Draw Curved Bezier Edges & Animated Energy Particles
    edgeList.forEach(e => {
      const src = nodeMap.get(e.source);
      const tgt = nodeMap.get(e.target);
      if (!src || !tgt) return;
      if (hideOrphans && (src.isOrphan || tgt.isOrphan)) return;

      const isConn = activeTargetId && (e.source === activeTargetId || e.target === activeTargetId);
      const isSemantic = e.origin === 'semantic' || e.dashed;
      const alpha = activeTargetId ? (isConn ? 0.95 : 0.05) : (isSemantic ? 0.65 : (e.type === 'calls' ? 0.38 : 0.22));
      ctx.lineWidth = isConn ? 2.2 : (isSemantic ? 1.6 : (e.type === 'calls' ? 1.3 : 0.9));
      ctx.strokeStyle = isConn ? '#00e5ff' : (isSemantic ? `rgba(236, 72, 153, ${alpha})` : (e.type === 'calls' ? `rgba(0, 229, 255, ${alpha})` : `rgba(167, 139, 250, ${alpha})`));

      const mx = (src.x + tgt.x) / 2;
      const my = (src.y + tgt.y) / 2;
      const dx = tgt.x - src.x;
      const dy = tgt.y - src.y;
      const len = Math.hypot(dx, dy) || 1;
      const cx = mx - (dy / len) * 16;
      const cy = my + (dx / len) * 16;

      if (isSemantic) ctx.setLineDash([5, 4]);
      ctx.beginPath();
      ctx.moveTo(src.x, src.y);
      ctx.quadraticCurveTo(cx, cy, tgt.x, tgt.y);
      ctx.stroke();
      if (isSemantic) ctx.setLineDash([]);

      // Flowing Energy Particle Pulse
      if (pulseEnabled && (isConn || !activeTargetId)) {
        const t = (animTime * (e.type === 'calls' ? 0.85 : 0.55) + e.offset) % 1.0;
        const px = (1 - t) * (1 - t) * src.x + 2 * (1 - t) * t * cx + t * t * tgt.x;
        const py = (1 - t) * (1 - t) * src.y + 2 * (1 - t) * t * cy + t * t * tgt.y;
        ctx.beginPath();
        ctx.arc(px, py, isConn ? 3.2 : 2.0, 0, 2 * Math.PI);
        ctx.fillStyle = isSemantic ? '#ec4899' : (e.type === 'calls' ? '#00e5ff' : '#a78bfa');
        ctx.shadowColor = ctx.fillStyle;
        ctx.shadowBlur = 6;
        ctx.fill();
        ctx.shadowBlur = 0;
      }
    });

    // 3. Draw Nodes with Smart Label LOD
    nodeArr.forEach(n => {
      if (hideOrphans && n.isOrphan) return;
      const matchFilter = filter === 'all' || (filter === 'hubs' ? n.isHub : n.kind === filter);
      const isHigh = !activeTargetId || neighborIds.has(n.id);
      const isSelected = n.id === selectedId;
      const isHovered = n.id === hoverId;
      ctx.globalAlpha = matchFilter ? (isHigh ? 1.0 : 0.12) : 0.05;

      if (n.isHub && matchFilter) {
        const pulseR = n.radius + 4 + Math.sin(animTime * 3) * 1.5;
        ctx.beginPath();
        ctx.arc(n.x, n.y, pulseR, 0, 2 * Math.PI);
        ctx.strokeStyle = '#f59e0b';
        ctx.lineWidth = 1.8;
        ctx.stroke();
      }

      if (isSelected || isHovered) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 5.5, 0, 2 * Math.PI);
        ctx.strokeStyle = '#00e5ff';
        ctx.lineWidth = 2.4;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, 2 * Math.PI);
      ctx.fillStyle = n.isOrphan ? '#475569' : (n.kind === 'function' ? '#10b981' : (n.kind === 'struct' ? '#00e5ff' : (n.kind === 'module' ? '#8b5cf6' : '#ec4899')));
      ctx.fill();
      ctx.strokeStyle = isSelected ? '#ffffff' : (n.isOrphan ? '#64748b' : 'rgba(255,255,255,0.7)');
      ctx.lineWidth = isSelected ? 2.5 : 1.0;
      ctx.stroke();

      // Label LOD: Hide non-hub labels when zoomed out (< 0.8) to eliminate text crowding
      const showLabel = matchFilter && isHigh && !n.isOrphan && (scale >= 0.8 || n.isHub || isSelected || isHovered);
      if (showLabel) {
        ctx.fillStyle = (isSelected || isHovered) ? '#00e5ff' : '#e2e8f0';
        ctx.font = `${Math.max(10, Math.min(13, 11 / scale))}px Inter, sans-serif`;
        ctx.fillText(n.label, n.x + n.radius + 5, n.y + 3);
      }
    });

    ctx.restore();
    requestAnimationFrame(render);
  }

  requestAnimationFrame(render);

  // Pan & Zoom Event Listeners
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const zoomFactor = e.deltaY < 0 ? 1.14 : 0.88;
    const newScale = Math.min(4.0, Math.max(0.2, scale * zoomFactor));
    const cr = canvas.getBoundingClientRect();
    const mx = e.clientX - cr.left;
    const my = e.clientY - cr.top;
    panX = mx - (mx - panX) * (newScale / scale);
    panY = my - (my - panY) * (newScale / scale);
    scale = newScale;
    if (onZoom) onZoom(scale);
  }, { passive: false });

  canvas.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    isDragging = true;
    hasDragged = false;
    startX = e.clientX;
    startY = e.clientY;
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
  });

  window.addEventListener('mousemove', (e) => {
    if (isDragging) {
      if (Math.hypot(e.clientX - startX, e.clientY - startY) > 4) hasDragged = true;
      panX += e.clientX - lastMouseX;
      panY += e.clientY - lastMouseY;
      lastMouseX = e.clientX;
      lastMouseY = e.clientY;
    }

    const cr = canvas.getBoundingClientRect();
    const mx = e.clientX - cr.left;
    const my = e.clientY - cr.top;
    const worldX = (mx - panX) / scale;
    const worldY = (my - panY) / scale;
    let found = null;
    for (const n of nodeArr) {
      if (hideOrphans && n.isOrphan) continue;
      if (Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 6) {
        found = n;
        break;
      }
    }
    if (hoverId !== (found ? found.id : null)) {
      hoverId = found ? found.id : null;
      canvas.style.cursor = found ? 'pointer' : (isDragging ? 'grabbing' : 'grab');
    }
  });

  window.addEventListener('mouseup', () => { isDragging = false; });

  canvas.addEventListener('click', (e) => {
    if (hasDragged) return;
    const cr = canvas.getBoundingClientRect();
    const mx = e.clientX - cr.left;
    const my = e.clientY - cr.top;
    const worldX = (mx - panX) / scale;
    const worldY = (my - panY) / scale;
    let clicked = null;
    for (const n of nodeArr) {
      if (hideOrphans && n.isOrphan) continue;
      if (Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 6) {
        clicked = n;
        break;
      }
    }
    selectedId = clicked ? (selectedId === clicked.id ? null : clicked.id) : null;
    if (clicked && onSelect) onSelect(clicked);
  });

  return {
    setFilter: (f) => { filter = f; },
    setPulse: (p) => { pulseEnabled = p; },
    toggleOrphans: (hide) => { hideOrphans = hide; },
    zoomIn: () => {
      scale = Math.min(4.0, scale * 1.25);
      if (onZoom) onZoom(scale);
    },
    zoomOut: () => {
      scale = Math.max(0.2, scale * 0.8);
      if (onZoom) onZoom(scale);
    },
    resetFit: () => {
      scale = 0.82;
      panX = W / 2;
      panY = H / 2;
      if (onZoom) onZoom(scale);
    },
    destroy: () => { running = false; }
  };
}
