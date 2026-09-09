// Force-directed Canvas Architecture Graph Renderer with Community Auras & Linker Pulses
import { partitionAndLayoutGraph } from './graphLayout.js';
import { drawNodeLabels } from './graphLabels.js';

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

  function fitToView() {
    const visibleNodes = nodeArr.filter(n => !(hideOrphans && n.isOrphan));
    if (!visibleNodes.length) return;
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    visibleNodes.forEach(n => {
      minX = Math.min(minX, n.x - n.radius);
      maxX = Math.max(maxX, n.x + n.radius);
      minY = Math.min(minY, n.y - n.radius);
      maxY = Math.max(maxY, n.y + n.radius);
    });
    auras.forEach(a => {
      minX = Math.min(minX, a.x - a.radius);
      maxX = Math.max(maxX, a.x + a.radius);
      minY = Math.min(minY, a.y - a.radius);
      maxY = Math.max(maxY, a.y + a.radius);
    });
    const gw = Math.max(maxX - minX, 120);
    const gh = Math.max(maxY - minY, 120);
    const pad = 48;
    const targetScale = Math.min((W - pad * 2) / gw, (H - pad * 2) / gh);
    scale = Math.max(0.18, Math.min(targetScale, 1.25));
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

    // 2. Draw Silky Fiber Linkers & Rapid Photon Comet Pulses
    edgeList.forEach(e => {
      const src = nodeMap.get(e.source);
      const tgt = nodeMap.get(e.target);
      if (!src || !tgt) return;
      if (hideOrphans && (src.isOrphan || tgt.isOrphan)) return;

      const isConn = activeTargetId && (e.source === activeTargetId || e.target === activeTargetId);
      const isSemantic = e.origin === 'semantic' || e.dashed;

      const dx = tgt.x - src.x;
      const dy = tgt.y - src.y;
      const len = Math.hypot(dx, dy) || 1;
      const curveOffset = Math.min(20, Math.max(6, len * 0.075));
      const mx = (src.x + tgt.x) / 2;
      const my = (src.y + tgt.y) / 2;
      const cx = mx - (dy / len) * curveOffset;
      const cy = my + (dx / len) * curveOffset;

      // Ultra-fine, silky thread styling
      ctx.save();
      if (activeTargetId) {
        if (isConn) {
          ctx.lineWidth = 1.35;
          ctx.strokeStyle = 'rgba(56, 189, 248, 0.95)';
          ctx.shadowColor = '#00e5ff';
          ctx.shadowBlur = 4;
        } else {
          ctx.lineWidth = 0.4;
          ctx.strokeStyle = 'rgba(148, 163, 184, 0.04)';
        }
      } else {
        if (isSemantic) {
          ctx.lineWidth = 0.6;
          ctx.strokeStyle = 'rgba(244, 114, 182, 0.32)';
          ctx.setLineDash([3, 4]);
        } else if (e.type === 'calls') {
          ctx.lineWidth = 0.65;
          ctx.strokeStyle = 'rgba(56, 189, 248, 0.26)';
        } else {
          ctx.lineWidth = 0.5;
          ctx.strokeStyle = 'rgba(167, 139, 250, 0.22)';
        }
      }

      ctx.beginPath();
      ctx.moveTo(src.x, src.y);
      ctx.quadraticCurveTo(cx, cy, tgt.x, tgt.y);
      ctx.stroke();
      ctx.restore();

      // Photon Comet Pulses: slow and graceful traversal (speed 0.4)
      if (pulseEnabled && (isConn || !activeTargetId)) {
        const speed = e.type === 'calls' ? 0.4 : 0.32;
        const t = (animTime * speed + e.offset) % 1.0;
        const tHead = t;
        const tTail = Math.max(0, t - (isConn ? 0.045 : 0.03));

        const pxHead = (1 - tHead) * (1 - tHead) * src.x + 2 * (1 - tHead) * tHead * cx + tHead * tHead * tgt.x;
        const pyHead = (1 - tHead) * (1 - tHead) * src.y + 2 * (1 - tHead) * tHead * cy + tHead * tHead * tgt.y;
        const pxTail = (1 - tTail) * (1 - tTail) * src.x + 2 * (1 - tTail) * tTail * cx + tTail * tTail * tgt.x;
        const pyTail = (1 - tTail) * (1 - tTail) * src.y + 2 * (1 - tTail) * tTail * cy + tTail * tTail * tgt.y;

        ctx.save();
        // Luminous streak tail
        ctx.beginPath();
        ctx.moveTo(pxTail, pyTail);
        ctx.lineTo(pxHead, pyHead);
        ctx.strokeStyle = isConn ? 'rgba(56, 189, 248, 0.9)' : (isSemantic ? 'rgba(244, 114, 182, 0.7)' : 'rgba(56, 189, 248, 0.6)');
        ctx.lineWidth = isConn ? 2.2 : 1.35;
        ctx.lineCap = 'round';
        ctx.stroke();

        // Brilliant photon head
        ctx.beginPath();
        ctx.arc(pxHead, pyHead, isConn ? 2.2 : 1.5, 0, 2 * Math.PI);
        ctx.fillStyle = '#ffffff';
        ctx.shadowColor = isSemantic ? '#ec4899' : (e.type === 'calls' ? '#00e5ff' : '#a78bfa');
        ctx.shadowBlur = isConn ? 9 : 5;
        ctx.fill();
        ctx.restore();
      }
    });

    // 3. Draw Nodes with Glowing Hub Rings
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
    });

    // 4. Draw Non-Colliding Labels with Priority LOD & Halos
    ctx.globalAlpha = 1.0;
    drawNodeLabels(ctx, nodeArr, {
      selectedId,
      hoverId,
      neighborIds,
      scale,
      filter,
      hideOrphans
    });

    ctx.restore();
    requestAnimationFrame(render);
  }

  requestAnimationFrame(render);

  // Pan & Zoom Event Listeners
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const zoomFactor = e.deltaY < 0 ? 1.14 : 0.88;
    const newScale = Math.min(4.0, Math.max(0.15, scale * zoomFactor));
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
    toggleOrphans: (hide) => { hideOrphans = hide; fitToView(); },
    zoomIn: () => {
      scale = Math.min(4.0, scale * 1.25);
      if (onZoom) onZoom(scale);
    },
    zoomOut: () => {
      scale = Math.max(0.15, scale * 0.8);
      if (onZoom) onZoom(scale);
    },
    resetFit: fitToView,
    destroy: () => { running = false; }
  };
}
