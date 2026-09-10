// Force-directed Canvas Architecture Graph Renderer with Community Auras & Linker Pulses
import { partitionAndLayoutGraph } from './graphLayout.js';
import { drawNodeLabels } from './graphLabels.js';
import { drawGraphEdges } from './graphEdges.js';

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
  let langFilter = 'all';
  let pulseEnabled = true;
  let hideOrphans = false;
  let animTime = 0, running = true;

  function fitToView() {
    const visibleNodes = nodeArr.filter(n => !(hideOrphans && n.isOrphan) && isFinite(n.x) && isFinite(n.y));
    if (!visibleNodes.length) return;
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    visibleNodes.forEach(n => {
      minX = Math.min(minX, n.x - n.radius);
      maxX = Math.max(maxX, n.x + n.radius);
      minY = Math.min(minY, n.y - n.radius);
      maxY = Math.max(maxY, n.y + n.radius);
    });
    auras.filter(a => isFinite(a.x) && isFinite(a.y)).forEach(a => {
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
      ctx.font = '600 9px Inter, sans-serif';
      ctx.fillText(`${a.name.toUpperCase()} (${a.count})`, a.x - a.radius * 0.6, a.y - a.radius + 13);
      ctx.restore();
    });

    // 2. Draw Silky Fiber Linkers & Rapid Photon Comet Pulses
    drawGraphEdges(ctx, edgeList, nodeMap, {
      activeTargetId,
      hideOrphans,
      pulseEnabled,
      animTime
    });

    // 3. Draw Nodes with Glowing Hub Rings
    nodeArr.forEach(n => {
      if (hideOrphans && n.isOrphan) return;
      const matchKind = filter === 'all' || (filter === 'hubs' ? n.isHub : n.kind === filter);
      const nLang = n.lang || detectNodeLang(n.file);
      const matchLang = langFilter === 'all' || nLang === langFilter;
      const matchFilter = matchKind && matchLang;

      const isHigh = !activeTargetId || neighborIds.has(n.id);
      const isSelected = n.id === selectedId;
      const isHovered = n.id === hoverId;
      ctx.globalAlpha = matchFilter ? (isHigh ? 1.0 : 0.12) : 0.05;

      if (n.isHub && matchFilter) {
        const pulseR = n.radius + 2.8 + Math.sin(animTime * 3) * 1.1;
        ctx.beginPath();
        ctx.arc(n.x, n.y, pulseR, 0, 2 * Math.PI);
        ctx.strokeStyle = '#f59e0b';
        ctx.lineWidth = 1.4;
        ctx.stroke();
      }

      if (isSelected || isHovered) {
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius + 3.8, 0, 2 * Math.PI);
        ctx.strokeStyle = '#00e5ff';
        ctx.lineWidth = 1.8;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(n.x, n.y, n.radius, 0, 2 * Math.PI);
      ctx.fillStyle = n.isOrphan ? '#475569' : (n.kind === 'function' ? '#10b981' : (n.kind === 'struct' ? '#00e5ff' : (n.kind === 'module' ? '#8b5cf6' : '#ec4899')));
      ctx.fill();
      ctx.strokeStyle = isSelected ? '#ffffff' : (n.isOrphan ? '#64748b' : 'rgba(255,255,255,0.7)');
      ctx.lineWidth = isSelected ? 1.8 : 0.8;
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
      if (Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 4.5) {
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
      if (Math.hypot(n.x - worldX, n.y - worldY) <= n.radius + 4.5) {
        clicked = n;
        break;
      }
    }
    selectedId = clicked ? (selectedId === clicked.id ? null : clicked.id) : null;
    if (clicked && onSelect) onSelect(clicked);
  });

  return {
    setFilter: (f, l) => {
      if (f !== undefined && f !== null) filter = f;
      if (l !== undefined && l !== null) langFilter = l;
    },
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
    selectNode: (id) => {
      selectedId = id;
      const n = nodeMap.get(id);
      if (n) {
        panX = W / 2 - n.x * scale;
        panY = H / 2 - n.y * scale;
      }
    },
    destroy: () => { running = false; }
  };
}

function detectNodeLang(file) {
  if (!file) return 'other';
  const ext = file.split('.').pop().toLowerCase();
  if (ext === 'rs') return 'rust';
  if (['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs'].includes(ext)) return 'typescript';
  if (['py', 'pyw'].includes(ext)) return 'python';
  if (ext === 'go') return 'go';
  if (['kt', 'kts'].includes(ext)) return 'kotlin';
  if (ext === 'java') return 'java';
  if (['c', 'h', 'cpp', 'hpp', 'cc', 'cxx'].includes(ext)) return 'cpp';
  if (ext === 'cs') return 'csharp';
  if (['sh', 'bash', 'zsh', 'ps1'].includes(ext)) return 'shell';
  if (['vue', 'svelte', 'astro', 'html', 'css'].includes(ext)) return ext;
  if (['sql', 'prisma', 'graphql', 'gql'].includes(ext)) return ext;
  return 'other';
}
