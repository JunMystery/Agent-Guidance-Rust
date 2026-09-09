// Silky Fiber Linkers & Rapid Photon Comet Pulses Renderer

export function drawGraphEdges(ctx, edgeList, nodeMap, opts) {
  const { activeTargetId, hideOrphans, pulseEnabled, animTime } = opts;

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
}
