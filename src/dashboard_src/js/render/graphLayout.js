// Community-Clustered ForceAtlas2 Graph Layout Engine with Island Partitioning
import { computeHubThreshold } from './graphLabels.js';

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
      isHub: false,
      commKey,
      radius: Math.max(5, Math.min(15, 5 + deg * 1.2))
    });
  }

  const hubThreshold = computeHubThreshold(processed);
  processed.forEach(n => {
    n.isHub = n.deg >= hubThreshold;
  });

  const connected = processed.filter(n => n.deg > 0);
  const orphans = processed.filter(n => n.deg === 0);

  // Group connected nodes by community
  const commGroups = new Map();
  connected.forEach(n => {
    if (!commGroups.has(n.commKey)) commGroups.set(n.commKey, []);
    commGroups.get(n.commKey).push(n);
  });

  // 1. Position Community Anchors (Macro Layout with generous spacing)
  const commKeys = Array.from(commGroups.keys());
  const commCount = Math.max(commKeys.length, 1);
  const commAnchors = new Map();
  const R_comm = commCount > 1
    ? Math.max(Math.min(W, H) * 0.48, 260 + Math.sqrt(connected.length) * 16)
    : 0;

  commKeys.forEach((key, idx) => {
    const angle = (idx / commCount) * 2 * Math.PI - Math.PI / 2;
    commAnchors.set(key, {
      cx: commCount > 1 ? R_comm * Math.cos(angle) : 0,
      cy: commCount > 1 ? R_comm * Math.sin(angle) : 0,
      color: COMM_COLORS[idx % COMM_COLORS.length],
      count: commGroups.get(key).length,
      name: key
    });
  });

  // Initialize connected positions using Fermat's Spiral (Phyllotaxis) around community anchors
  connected.forEach((n, idx) => {
    const anchor = commAnchors.get(n.commKey) || { cx: 0, cy: 0 };
    const phi = idx * 2.3999632;
    const spread = 35 + Math.sqrt(idx + 1) * 32;
    nodeMap.set(n.id, {
      ...n,
      x: anchor.cx + spread * Math.cos(phi),
      y: anchor.cy + spread * Math.sin(phi),
      color: anchor.color || '#00e5ff'
    });
  });

  // 2. Physics Simulation: ForceAtlas2 with LinLog Repulsion & Soft Distance Falloff
  const connNodes = Array.from(nodeMap.values());
  const validEdges = edges.filter(e => nodeMap.has(e.source) && nodeMap.has(e.target));
  const iterations = 110;

  for (let iter = 0; iter < iterations; iter++) {
    const temp = Math.pow(1.0 - iter / iterations, 1.4);

    // Repulsion (LinLog: effective over wider radius, cross-community repel 2.6x)
    for (let i = 0; i < connNodes.length; i++) {
      for (let j = i + 1; j < connNodes.length; j++) {
        const a = connNodes[i];
        const b = connNodes[j];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.hypot(dx, dy) || 1;
        const crossComm = a.commKey !== b.commKey;
        const kRepel = crossComm ? 1800 : 900;
        const f = (kRepel * Math.sqrt((a.deg + 1) * (b.deg + 1)) * temp) / (dist + 30);
        const nx = dx / dist;
        const ny = dy / dist;
        a.x -= nx * f;
        a.y -= ny * f;
        b.x += nx * f;
        b.y += ny * f;
      }
    }

    // Attraction along actual graph edges (Generous rest length)
    validEdges.forEach(e => {
      const a = nodeMap.get(e.source);
      const b = nodeMap.get(e.target);
      if (!a || !b) return;
      const dx = b.x - a.x;
      const dy = b.y - a.y;
      const dist = Math.hypot(dx, dy) || 1;
      const restLen = 95 + Math.min(a.deg + b.deg, 16) * 2;
      const f = (dist - restLen) * 0.028 * temp;
      const nx = dx / dist;
      const ny = dy / dist;
      a.x += nx * f;
      a.y -= ny * f;
      b.x -= nx * f;
      b.y += ny * f;
    });

    // Gentle Community Centroid Gravity (Allows clusters to breathe while grouping)
    connNodes.forEach(n => {
      const anchor = commAnchors.get(n.commKey);
      if (anchor) {
        n.x += (anchor.cx - n.x) * 0.016 * temp;
        n.y += (anchor.cy - n.y) * 0.016 * temp;
      }
    });

    // Anti-collision push: Generous buffer so nodes and their labels never collide
    for (let i = 0; i < connNodes.length; i++) {
      for (let j = i + 1; j < connNodes.length; j++) {
        const a = connNodes[i];
        const b = connNodes[j];
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.hypot(dx, dy) || 1;
        const minDist = a.radius + b.radius + 52;
        if (dist < minDist) {
          const push = ((minDist - dist) / 2) * 0.75;
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

  // Re-center connected nodes strictly around (0, 0)
  if (connNodes.length > 0) {
    const cX = connNodes.reduce((s, n) => s + n.x, 0) / connNodes.length;
    const cY = connNodes.reduce((s, n) => s + n.y, 0) / connNodes.length;
    connNodes.forEach(n => {
      n.x -= cX;
      n.y -= cY;
    });
  }

  const maxConnR = connNodes.reduce((max, n) => Math.max(max, Math.hypot(n.x, n.y)), 0);

  // 3. Position Orphans in Peripheral Orbit Dock
  const numOrphans = orphans.length;
  const numRings = numOrphans > 70 ? 3 : (numOrphans > 32 ? 2 : 1);
  const baseOrbitR = Math.max(maxConnR + 65, Math.min(W, H) * 0.45);

  orphans.forEach((n, idx) => {
    const ringIdx = idx % numRings;
    const perRing = Math.ceil(numOrphans / numRings);
    const posInRing = Math.floor(idx / numRings);
    const angle = (posInRing / Math.max(perRing, 1)) * 2 * Math.PI;
    const r = baseOrbitR + ringIdx * 28;
    nodeMap.set(n.id, {
      ...n,
      x: r * Math.cos(angle),
      y: r * Math.sin(angle),
      isOrphan: true,
      color: '#475569'
    });
  });

  // Calculate Community Auras based on true node bounds
  const auras = [];
  commAnchors.forEach((anc, key) => {
    const members = connNodes.filter(n => n.commKey === key);
    if (members.length < 2) return;
    const avgX = members.reduce((s, m) => s + m.x, 0) / members.length;
    const avgY = members.reduce((s, m) => s + m.y, 0) / members.length;
    const maxR = members.reduce((max, m) => Math.max(max, Math.hypot(m.x - avgX, m.y - avgY)), 0);
    auras.push({
      key,
      name: anc.name,
      x: avgX,
      y: avgY,
      radius: Math.max(55, maxR + 35),
      color: anc.color,
      count: members.length
    });
  });

  return {
    nodeMap,
    edgeList: validEdges.map(e => ({ ...e, offset: Math.random() })),
    auras,
    connectedCount: connected.length,
    orphanCount: orphans.length,
    hubThreshold
  };
}
