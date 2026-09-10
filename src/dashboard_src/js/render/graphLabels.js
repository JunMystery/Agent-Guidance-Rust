// Graph Label Rendering with Dynamic Hub LOD, BBox Collision Culling, and Contrast Halos

export function computeHubThreshold(nodes) {
  const degs = nodes.map(n => n.deg || 0).filter(d => d > 0).sort((a, b) => b - a);
  if (!degs.length) return 5;
  const top12Idx = Math.floor(degs.length * 0.12);
  return Math.max(5, degs[top12Idx] || 5);
}

export function drawNodeLabels(ctx, nodeArr, opts) {
  const { selectedId, hoverId, neighborIds, scale, filter, hideOrphans } = opts;
  const activeTargetId = selectedId || hoverId;

  // 1. Filter candidates by Zoom Level-of-Detail (LOD)
  const candidates = [];
  for (let i = 0; i < nodeArr.length; i++) {
    const n = nodeArr[i];
    if (hideOrphans && n.isOrphan) continue;
    const matchFilter = filter === 'all' || (filter === 'hubs' ? n.isHub : n.kind === filter);
    if (!matchFilter) continue;

    const isTarget = n.id === activeTargetId;
    const isNeighbor = activeTargetId && neighborIds && neighborIds.has(n.id);
    const isHigh = isTarget || isNeighbor;

    let shouldShow = false;
    if (isTarget) {
      shouldShow = true;
    } else if (scale >= 0.95) {
      shouldShow = !activeTargetId || isHigh;
    } else if (scale >= 0.65) {
      shouldShow = isHigh || n.isHub || (n.deg >= 4 && !activeTargetId);
    } else if (scale >= 0.4) {
      shouldShow = isHigh || (n.isHub && (!activeTargetId || isHigh));
    } else {
      // Zoom < 0.4 (extreme overview)
      shouldShow = isTarget || (n.isHub && !activeTargetId && n.deg >= 8);
    }

    if (shouldShow) {
      candidates.push({
        node: n,
        isTarget,
        isNeighbor,
        isHigh,
        priority: isTarget ? 3 : (isNeighbor ? 2 : (n.isHub ? 1 : 0))
      });
    }
  }

  // 2. Sort by priority: Selected/Hovered -> Neighbors -> Hubs (by degree) -> Others
  candidates.sort((a, b) => {
    if (a.priority !== b.priority) return b.priority - a.priority;
    return (b.node.deg || 0) - (a.node.deg || 0);
  });

  // 3. Greedy Bounding-Box Collision Culling
  const placedBoxes = [];
  const baseFontSize = Math.max(7, Math.min(10, Math.round(8.5 / Math.sqrt(scale))));

  for (let i = 0; i < candidates.length; i++) {
    const { node: n, isTarget, isNeighbor, isHigh } = candidates[i];
    const fontSize = isTarget ? baseFontSize + 1 : baseFontSize;
    ctx.font = `${isHigh ? '600 ' : '400 '}${fontSize}px Inter, -apple-system, sans-serif`;

    const labelText = (!isHigh && n.label.length > 22)
      ? (n.label.slice(0, 20) + '…')
      : n.label;

    const tw = ctx.measureText(labelText).width;
    const lx = n.x + n.radius + 4;
    const ly = n.y + fontSize * 0.35;
    const box = {
      x1: lx - 3,
      y1: ly - fontSize * 0.9,
      x2: lx + tw + 4,
      y2: ly + fontSize * 0.3
    };

    // Collision check against previously placed labels (target always displays)
    if (!isTarget) {
      let overlaps = false;
      const pad = 3;
      for (let j = 0; j < placedBoxes.length; j++) {
        const b = placedBoxes[j];
        if (!(box.x2 + pad < b.x1 || box.x1 - pad > b.x2 || box.y2 + pad < b.y1 || box.y1 - pad > b.y2)) {
          overlaps = true;
          break;
        }
      }
      if (overlaps) continue; // Skip occluded label
    }

    placedBoxes.push(box);

    // 4. Render with Dark Halo for Crystal-Clear Readability
    ctx.strokeStyle = '#080c14';
    ctx.lineWidth = 2.4;
    ctx.lineJoin = 'round';
    ctx.strokeText(labelText, lx, ly);

    ctx.fillStyle = isTarget ? '#00e5ff' : (isNeighbor ? '#e0f2fe' : (n.isHub ? '#f8fafc' : '#cbd5e1'));
    ctx.fillText(labelText, lx, ly);
  }
}
