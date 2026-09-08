// Community-Clustered ForceAtlas2 Graph Layout Engine with Island Partitioning
const COMM_COLORS = ['#00e5ff', '#ec4899', '#10b981', '#a78bfa', '#f59e0b', '#38bdf8', '#fb7185', '#34d399'];

export function partitionAndLayoutGraph(nodes, edges, rawCommunities, W, H) {
  const nodeCount = Math.min(nodes.length, 300);
  const nodeMap = new Map();
  const inDegree = new Map();
  const outDegree = new Map();

  edges.forEach(e => {
    outDegree.set(e.source, (outDegree.get(e.source) || 0) + 1);
    inDegree.set(e.target, (inDegree.get(e.target) || 0) + 1);
  });

  const commMap = new Map();
  const commList = Array.isArray(rawCommunities) ? rawCommunities : (rawCommunities?.communities || []);
  commList.forEach(c => {
    const title = c.summary?.title || c.id;
    (c.entity_ids || []).forEach(eid => commMap.set(eid, title));
    (c.file_paths || []).forEach(fp => commMap.set(fp, title));
  });

  const processed = [];
  for (let i = 0; i < nodeCount; i++) {
    const n = nodes[i];
    const deg = (inDegree.get(n.id) || 0) + (outDegree.get(n.id) || 0);
    const commKey = commMap.get(n.id) || commMap.get(n.file) || n.file?.split(/[/\\]/)[0] || 'core';
    processed.push({
      ...n,
      deg,
      isHub: deg >= 3,
      commKey,
      radius: Math.max(5, Math.min(15, 5 + deg * 1.2))
    });
  }

  const connected = processed.filter(n => n.deg > 0);
  const orphans = processed.filter(n => n.deg === 0);

  // Group connected nodes by community
  const commGroups = new Map();
  connected.forEach(n => {
    if (!commGroups.has(n.commKey)) commGroups.set(n.commKey, []);
    commGroups.get(n.commKey).push(n);
  });

  // 1. Position Community Anchors (Macro Layout)
  const commKeys = Array.from(commGroups.keys());
  const commCount = Math.max(commKeys.length, 1);
  const commAnchors = new Map();
  const R_comm = Math.min(W, H) * 0.36;

  commKeys.forEach((key, idx) => {
    const angle = (idx / commCount) * 2 * Math.PI - Math.PI / 2;
    commAnchors.set(key, {
      cx: R_comm * Math.cos(angle),
      cy: R_comm * Math.sin(angle),
      color: COMM_COLORS[idx % COMM_COLORS.length],
      count: commGroups.get(key).length,
      name: key
    });
  });

  // Initialize connected positions near community anchors
  connected.forEach((n, idx) => {
    const anchor = commAnchors.get(n.commKey) || { cx: 0, cy: 0 };
    const subAngle = (idx % 12) * (Math.PI / 6);
    const spread = 25 + (idx % 4) * 20;
    nodeMap.set(n.id, {
      ...n,
      x: anchor.cx + spread * Math.cos(subAngle),
      y: anchor.cy + spread * Math.sin(subAngle),
      color: anchor.color || '#00e5ff'
    });
  });

  // 2. Physics Simulation: ForceAtlas2 with LinLog Repulsion (85 iterations)
  const connNodes = Array.from(nodeMap.values());
  const validEdges = edges.filter(e => nodeMap.has(e.source) && nodeMap.has(e.target));

  for (let iter = 0; iter < 85; iter++) {
    const temp = Math.pow(1.0 - iter / 85, 1.6);

    // Repulsion (LinLog: hubs repel hubs strongly, cross-community repel 2.8x)
    for (let i = 0; i < connNodes.length; i++) {
      for (let j = i + 1; j < connNodes.length; j++) {
        const a = connNodes[i];
        const b = connNodes[j];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.hypot(dx, dy) || 1;
        const crossComm = a.commKey !== b.commKey;
        const kRepel = crossComm ? 580 : 220;
        const f = (kRepel * (a.deg + 1) * (b.deg + 1) * temp) / (dist * dist);
        const nx = dx / dist;
        const ny = dy / dist;
        a.x -= nx * f;
        a.y -= ny * f;
        b.x += nx * f;
        b.y += ny * f;
      }
    }

    // Attraction along actual graph edges (Linear hooke)
    validEdges.forEach(e => {
      const a = nodeMap.get(e.source);
      const b = nodeMap.get(e.target);
      if (!a || !b) return;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist = Math.hypot(dx, dy) || 1;
      const f = (dist - 45) * 0.075 * temp;
      const nx = dx / dist;
      const ny = dy / dist;
      a.x += nx * f;
      a.y += ny * f;
      b.x -= nx * f;
      b.y -= ny * f;
    });

    // Community Centroid Gravity (Keeps module members inside their island)
    connNodes.forEach(n => {
      const anchor = commAnchors.get(n.commKey);
      if (anchor) {
        n.x += (anchor.cx - n.x) * 0.055 * temp;
        n.y += (anchor.cy - n.y) * 0.055 * temp;
      }
    });

    // Anti-collision push
    for (let i = 0; i < connNodes.length; i++) {
      for (let j = i + 1; j < connNodes.length; j++) {
        const a = connNodes[i];
        const b = connNodes[j];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.hypot(dx, dy) || 1;
        const minDist = a.radius + b.radius + 32;
        if (dist < minDist) {
          const push = ((minDist - dist) / 2) * 0.6;
          const nx = dx / dist;
          const ny = dy / dist;
          a.x -= nx * push;
          a.y -= ny * push;
          b.x += nx * push;
          b.y += ny * push;
        }
      }
    }
  }

  // 3. Position Orphans in Peripheral Orbit Dock (Clean outer ring)
  const R_orbit = Math.min(W, H) * 0.49;
  orphans.forEach((n, idx) => {
    const angle = (idx / Math.max(orphans.length, 1)) * 2 * Math.PI;
    nodeMap.set(n.id, {
      ...n,
      x: R_orbit * Math.cos(angle),
      y: R_orbit * Math.sin(angle),
      isOrphan: true,
      color: '#475569'
    });
  });

  // Calculate Community Auras
  const auras = [];
  commAnchors.forEach((anc, key) => {
    const members = connNodes.filter(n => n.commKey === key);
    if (!members.length) return;
    const avgX = members.reduce((s, m) => s + m.x, 0) / members.length;
    const avgY = members.reduce((s, m) => s + m.y, 0) / members.length;
    const maxR = members.reduce((max, m) => Math.max(max, Math.hypot(m.x - avgX, m.y - avgY)), 0);
    auras.push({
      key,
      name: anc.name,
      x: avgX,
      y: avgY,
      radius: Math.max(50, maxR + 35),
      color: anc.color,
      count: members.length
    });
  });

  return {
    nodeMap,
    edgeList: validEdges.map(e => ({ ...e, offset: Math.random() })),
    auras,
    connectedCount: connected.length,
    orphanCount: orphans.length
  };
}
