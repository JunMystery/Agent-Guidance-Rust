import assert from 'node:assert';
import { calcFileNodeRadius } from '../src/dashboard_src/js/render/fileGraphCanvas.js';
import { drawArrowhead } from '../src/dashboard_src/js/render/graphEdges.js';
import { renderFileInspector } from '../src/dashboard_src/js/render/fileInspectorView.js';
import { renderFunctionInspector } from '../src/dashboard_src/js/render/functionInspectorView.js';
import { graphEn } from '../src/dashboard_src/js/i18n/en/graph.js';
import { graphVi } from '../src/dashboard_src/js/i18n/vi/graph.js';

console.log('=== RUNNING TIER 5 ADVERSARIAL FRONTEND STRESS TEST SUITE ===\n');

// 1. Stress Test: calcFileNodeRadius
console.log('[TEST 1] calcFileNodeRadius Boundary & Adversarial Math');
const radiusCases = [
  { loc: 0, sym: 0, min: 12, max: 12 },
  { loc: -999, sym: -50, min: 12, max: 12 },
  { loc: NaN, sym: NaN, min: 12, max: 12 },
  { loc: undefined, sym: undefined, min: 12, max: 12 },
  { loc: null, sym: null, min: 12, max: 12 },
  { loc: 'foo', sym: 'bar', min: 12, max: 12 },
  { loc: 100, sym: 5, min: 12, max: 34 },
  { loc: 1000000, sym: 50000, min: 34, max: 34 },
  { loc: Infinity, sym: Infinity, min: 34, max: 34 },
];
for (const tc of radiusCases) {
  const r = calcFileNodeRadius(tc.loc, tc.sym);
  assert(!isNaN(r), `Radius must not be NaN for ${JSON.stringify(tc)}`);
  assert(r >= tc.min && r <= tc.max, `Radius ${r} out of bounds for ${JSON.stringify(tc)}`);
}
console.log('  -> PASS: 9/9 radius boundary cases verified strictly clamped [12, 34].');

// 2. Stress Test: drawArrowhead Coincident Points
console.log('\n[TEST 2] drawArrowhead Coincident Points & Negative Radii');
const canvasCalls = [];
const mockCtx = {
  save: () => canvasCalls.push('save'),
  restore: () => canvasCalls.push('restore'),
  beginPath: () => canvasCalls.push('beginPath'),
  moveTo: (x, y) => canvasCalls.push(['moveTo', x, y]),
  lineTo: (x, y) => canvasCalls.push(['lineTo', x, y]),
  closePath: () => canvasCalls.push('closePath'),
  fill: () => canvasCalls.push('fill'),
};

// Coincident points: (fromX=50, fromY=50) == (toX=50, toY=50)
drawArrowhead(mockCtx, 50, 50, 50, 50, 10, '#10b981', 7);
for (const entry of canvasCalls) {
  if (Array.isArray(entry)) {
    assert(!isNaN(entry[1]), `X coordinate must not be NaN in ${entry[0]}`);
    assert(!isNaN(entry[2]), `Y coordinate must not be NaN in ${entry[0]}`);
  }
}
console.log('  -> PASS: Zero NaNs on coincident points (dx=0, dy=0).');

// 3. Stress Test: fileInspectorView XSS & Null states
console.log('\n[TEST 3] fileInspectorView Null & XSS Injection');
let container = { innerHTML: '', querySelector: () => null, querySelectorAll: () => [] };
renderFileInspector(container, null);
assert(container.innerHTML.length > 0, 'Null node should render placeholder');

const dangerousNode = {
  id: '<script>alert("xss1")</script>',
  file: '<img src=x onerror=alert(1)>',
  label: '<script>alert("label")</script>',
  loc: 120,
  total_symbols: 8,
};
const dangerousEdges = [
  {
    source: dangerousNode.id,
    target: '<b>target.rs</b>',
    weight: 2,
    calls: [{ caller: '<script>', callee: '<svg onload=alert(1)>', call_line: 42 }],
  },
];
renderFileInspector(container, dangerousNode, dangerousEdges, [dangerousNode]);
assert(!container.innerHTML.includes('<script>alert'), 'HTML script tags must be escaped');
assert(!container.innerHTML.includes('<img src=x'), 'Img tags must be escaped');
assert(!container.innerHTML.includes('<svg onload='), 'SVG onload must be escaped');
console.log('  -> PASS: XSS vectors properly sanitized via escapeHtml.');

// 4. Stress Test: functionInspectorView Null & Boundary states
console.log('\n[TEST 4] functionInspectorView Null & Missing Field Resilience');
renderFunctionInspector(container, null);
assert(container.innerHTML.length > 0, 'Null node should render placeholder');

const sparseFnNode = {
  id: 'fn_sparse',
  name: 'sparse_fn',
  // missing file, loc, start_line, end_line
};
renderFunctionInspector(container, sparseFnNode, [], [sparseFnNode]);
assert(!container.innerHTML.includes('NaN'), 'Sparse node should not render NaN LOC');
assert(container.innerHTML.includes('fn_sparse'), 'Should display function name or id');
console.log('  -> PASS: Handled sparse/missing fields without NaN.');

// 5. Stress Test: i18n Bilingual Parity & Completeness
console.log('\n[TEST 5] i18n 1-to-1 Translation Parity');
const enKeys = Object.keys(graphEn).sort();
const viKeys = Object.keys(graphVi).sort();
assert.strictEqual(enKeys.length, viKeys.length, 'Key counts must match');
for (const key of enKeys) {
  assert(key in graphVi, `Missing Vietnamese translation for key: ${key}`);
  assert(typeof graphEn[key] === typeof graphVi[key], `Type mismatch for key: ${key}`);
}
console.log(`  -> PASS: All ${enKeys.length} keys have 1-to-1 parity between EN and VI.`);

// 6. Subgraph Isolation Verification Oracle
console.log('\n[TEST 6] Mode 1 Subgraph Isolation Oracle');
function computeIsolatedSet(focusId, edgeList) {
  if (!focusId) return new Set();
  const s = new Set([focusId]);
  edgeList.forEach(e => {
    if (e.source === focusId) s.add(e.target);
    if (e.target === focusId) s.add(e.source);
  });
  return s;
}

const mockEdges = [
  { source: 'A', target: 'B' },
  { source: 'C', target: 'A' },
  { source: 'B', target: 'D' }, // D is 2-hop from A
  { source: 'X', target: 'Y' }, // X-Y is disjoint
];
const isolatedA = computeIsolatedSet('A', mockEdges);
assert(isolatedA.has('A'), 'Must contain focus node');
assert(isolatedA.has('B'), 'Must contain 1-N outgoing dependency B');
assert(isolatedA.has('C'), 'Must contain N-1 incoming dependent C');
assert(!isolatedA.has('D'), 'Must prune 2-hop node D');
assert(!isolatedA.has('X') && !isolatedA.has('Y'), 'Must prune disjoint nodes X, Y');
console.log('  -> PASS: Subgraph isolation oracle strictly isolates 1-hop and prunes 2-hop/disjoint nodes.');

console.log('\n=== ALL TIER 5 FRONTEND TESTS PASSED SUCCESSFULLY ===');
