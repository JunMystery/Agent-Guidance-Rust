import { t } from '../i18n/index.js';

// Rich Symbol Inspector: Functional Roles, Architectural Tiers, Blast Radius & AST Relations

export function renderSymbolInspector(container, node, edges, allNodes, onSelectNode) {
  if (!container) return;
  if (!node) {
    container.innerHTML = `
      <div style="font-size: 12px; font-weight: 600; color: var(--text-primary); margin-bottom: 8px;">${t('inspector.title')}</div>
      <div style="font-size: 12px; color: var(--text-secondary); line-height: 1.6;">
        ${t('inspector.hint')}
      </div>
    `;
    return;
  }

  const nodeMap = new Map(allNodes.map(n => [n.id, n]));
  const incoming = edges.filter(e => e.target === node.id).map(e => ({
    node: nodeMap.get(e.source) || { id: e.source, label: e.source.split('::')[2] || e.source, kind: 'unknown' },
    type: e.type || 'calls',
    origin: e.origin || 'ast'
  }));
  const outgoing = edges.filter(e => e.source === node.id).map(e => ({
    node: nodeMap.get(e.target) || { id: e.target, label: e.target.split('::')[2] || e.target, kind: 'unknown' },
    type: e.type || 'calls',
    origin: e.origin || 'ast'
  }));

  const memberMethods = (node.kind === 'struct' || node.kind === 'class' || node.kind === 'interface')
    ? allNodes.filter(n => n.parent === node.label && n.file === node.file)
    : [];

  const role = inferFunctionalRole(node);
  const blastRisk = getBlastRadiusRisk(node, incoming.length, outgoing.length);

  const kindColors = {
    function: '#10b981',
    struct: '#00e5ff',
    module: '#8b5cf6',
    trait: '#ec4899',
    enum: '#f59e0b',
    unknown: '#94a3b8'
  };
  const kindColor = kindColors[node.kind] || '#00e5ff';

  container.innerHTML = `
    <div style="display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; margin-bottom: 10px;">
      <div style="flex: 1; min-width: 0;">
        <span style="font-size: 10px; text-transform: uppercase; font-weight: 700; color: ${kindColor}; background: ${kindColor}18; border: 1px solid ${kindColor}40; padding: 2px 6px; border-radius: 4px; display: inline-block; margin-bottom: 4px;">
          ${node.kind || 'symbol'}
        </span>
        ${node.lang ? `<span style="font-size: 10px; text-transform: uppercase; font-weight: 700; color: #38bdf8; background: #0284c718; border: 1px solid #0284c740; padding: 2px 6px; border-radius: 4px; display: inline-block; margin-left: 4px; margin-bottom: 4px;">${node.lang}</span>` : ''}
        <h4 style="color: #f8fafc; font-size: 14px; margin: 0; font-family: var(--font-mono); word-break: break-all; line-height: 1.3;">
          ${escapeHtml(node.label)}
        </h4>
      </div>
      <button id="btn-copy-symbol" title="${t('inspector.copy_id_title')}" style="background: var(--bg-tertiary); border: 1px solid var(--border-color); color: #94a3b8; border-radius: 4px; padding: 3px 6px; font-size: 11px; cursor: pointer;">
        📋
      </button>
    </div>

    <!-- Functional Role & Architecture Tier -->
    <div style="background: #0f172a; border: 1px solid #1e293b; border-radius: 6px; padding: 8px 10px; margin-bottom: 10px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;">
        <span style="font-size: 11px; color: #64748b; font-weight: 600;">${t('inspector.role').toUpperCase()}:</span>
        <span style="font-size: 10px; font-family: var(--font-mono); color: #38bdf8; background: #0284c71a; padding: 1px 6px; border-radius: 3px; border: 1px solid #0284c730;">${role.tier}</span>
      </div>
      <div style="font-size: 12px; color: #e2e8f0; font-weight: 500;">
        ${role.description}
      </div>
    </div>

    <!-- Blast Radius & Health Metrics -->
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin-bottom: 10px;">
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">${t('inspector.impact_callers')}</div>
        <div style="font-size: 15px; font-weight: 700; color: #38bdf8;">${node.inDeg || 0}</div>
        <div style="font-size: 9px; color: #64748b;">${t('inspector.dependent_symbols')}</div>
      </div>
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">${t('inspector.dependencies_stat')}</div>
        <div style="font-size: 15px; font-weight: 700; color: #10b981;">${node.outDeg || 0}</div>
        <div style="font-size: 9px; color: #64748b;">${t('inspector.required_symbols')}</div>
      </div>
    </div>

    ${node.isHub ? `<div style="margin-bottom: 10px; background: #f59e0b18; border: 1px solid #f59e0b40; color: #fbbf24; border-radius: 4px; padding: 6px 8px; font-size: 11px; font-weight: 600;">${t('inspector.critical_hub')}</div>` : ''}

    <div style="margin-bottom: 10px; font-size: 11px; color: ${blastRisk.color}; background: ${blastRisk.color}14; border: 1px solid ${blastRisk.color}30; border-radius: 4px; padding: 6px 8px;">
      <strong>${t('inspector.blast_radius')}:</strong> ${blastRisk.label}
    </div>

    <!-- Source Location -->
    <div style="margin-bottom: 12px; font-size: 11px; color: var(--text-secondary);">
      <span style="font-weight: 600; color: #cbd5e1;">${t('inspector.file_size')}</span>
      <div style="font-family: var(--font-mono); font-size: 10px; color: #94a3b8; word-break: break-all; margin-top: 2px; background: var(--bg-tertiary); padding: 4px 6px; border-radius: 4px;">
        ${escapeHtml(node.file || '--')} <span style="color: #64748b;">(${node.loc || '--'} LOC)</span>
      </div>
    </div>

    ${node.parent ? `
      <div style="margin-bottom: 10px; font-size: 11px; background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 5px 8px;">
        <span style="color: #64748b; font-weight: 600; font-size: 10px;">ENCLOSING PARENT:</span>
        <div style="color: #38bdf8; font-family: var(--font-mono); font-size: 11px; margin-top: 2px;">${escapeHtml(node.parent)}</div>
      </div>
    ` : ''}

    ${memberMethods.length ? `
      <div style="margin-bottom: 10px; border-top: 1px solid var(--border-color); padding-top: 8px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
          <span style="font-size: 11px; font-weight: 600; color: #a78bfa;">MEMBER METHODS (${memberMethods.length})</span>
        </div>
        <div style="max-height: 90px; overflow-y: auto; display: flex; flex-direction: column; gap: 3px;">
          ${memberMethods.map(m => `
            <div class="node-jumper" data-node-id="${escapeHtml(m.id)}" style="display: flex; align-items: center; justify-content: space-between; background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 3px; padding: 3px 6px; cursor: pointer;">
              <span style="font-family: var(--font-mono); font-size: 10px; color: #e2e8f0;">${escapeHtml(m.label)}</span>
              <span style="font-size: 9px; color: #64748b;">L${m.loc || 1}</span>
            </div>
          `).join('')}
        </div>
      </div>
    ` : ''}

    <!-- Incoming Callers Details -->
    <div style="margin-bottom: 10px; border-top: 1px solid var(--border-color); padding-top: 8px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span style="font-size: 11px; font-weight: 600; color: #38bdf8;">${t('inspector.callers')} (${incoming.length})</span>
        <span style="font-size: 10px; color: #64748b;">${t('inspector.who_calls_this')}</span>
      </div>
      <div style="max-height: 110px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px;">
        ${incoming.length ? incoming.map(i => renderRelationItem(i, 'caller', onSelectNode)).join('') : `<span style="font-size: 11px; color: #64748b; font-style: italic;">${t('inspector.no_callers')}</span>`}
      </div>
    </div>

    <!-- Outgoing Dependencies Details -->
    <div style="border-top: 1px solid var(--border-color); padding-top: 8px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span style="font-size: 11px; font-weight: 600; color: #10b981;">${t('inspector.dependencies')} (${outgoing.length})</span>
        <span style="font-size: 10px; color: #64748b;">${t('inspector.what_this_calls')}</span>
      </div>
      <div style="max-height: 110px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px;">
        ${outgoing.length ? outgoing.map(o => renderRelationItem(o, 'dep', onSelectNode)).join('') : `<span style="font-size: 11px; color: #64748b; font-style: italic;">${t('inspector.no_dependencies')}</span>`}
      </div>
    </div>
  `;

  // Wire copy button
  const copyBtn = container.querySelector('#btn-copy-symbol');
  if (copyBtn) {
    copyBtn.onclick = () => {
      navigator.clipboard.writeText(node.label).then(() => {
        copyBtn.textContent = '✅';
        setTimeout(() => { copyBtn.textContent = '📋'; }, 1500);
      });
    };
  }

  // Wire interactive node jumper links
  container.querySelectorAll('.node-jumper').forEach(el => {
    el.onclick = (e) => {
      e.preventDefault();
      const targetId = el.getAttribute('data-node-id');
      if (targetId && onSelectNode) {
        const targetNode = nodeMap.get(targetId);
        if (targetNode) onSelectNode(targetNode);
      }
    };
  });
}

function renderRelationItem(rel, direction, onSelect) {
  const n = rel.node;
  const typeBadgeColor = rel.origin === 'semantic' ? '#ec4899' : '#38bdf8';
  return `
    <div class="node-jumper" data-node-id="${escapeHtml(n.id)}" style="display: flex; align-items: center; justify-content: space-between; background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 4px 8px; cursor: pointer; transition: background 0.15s;" onmouseover="this.style.background='#1e293b'" onmouseout="this.style.background='var(--bg-tertiary)'">
      <div style="min-width: 0; flex: 1;">
        <div style="font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: #f1f5f9; text-overflow: ellipsis; overflow: hidden; white-space: nowrap;">
          ${escapeHtml(n.label)}
        </div>
        <div style="font-size: 9px; color: #64748b; text-overflow: ellipsis; overflow: hidden; white-space: nowrap;">
          ${escapeHtml((n.file || '').split(/[\\/]/).slice(-2).join('/'))}
        </div>
      </div>
      <span style="font-size: 9px; font-weight: 600; color: ${typeBadgeColor}; background: ${typeBadgeColor}18; padding: 1px 5px; border-radius: 3px; border: 1px solid ${typeBadgeColor}30; margin-left: 6px;">
        ${rel.type}
      </span>
    </div>
  `;
}

function inferFunctionalRole(node) {
  const f = (node.file || '').toLowerCase();
  const name = (node.label || '').toLowerCase();

  if (f.includes('daemon') || name.includes('daemon') || name.includes('ipc')) {
    return { tier: t('inspector.tier_daemon'), description: t('inspector.desc_daemon') };
  }
  if (f.includes('mcp_logger') || f.includes('logs_api') || name.includes('log')) {
    return { tier: t('inspector.tier_diagnostics'), description: t('inspector.desc_diagnostics') };
  }
  if (f.includes('dashboard') || name.includes('dashboard') || name.includes('graph') || name.includes('canvas')) {
    return { tier: t('inspector.tier_presentation'), description: t('inspector.desc_presentation') };
  }
  if (f.includes('ml') || f.includes('embedding') || f.includes('candle') || name.includes('bert')) {
    return { tier: t('inspector.tier_neural'), description: t('inspector.desc_neural') };
  }
  if (f.includes('db') || f.includes('sqlite') || name.includes('db')) {
    return { tier: t('inspector.tier_database'), description: t('inspector.desc_database') };
  }
  if (f.includes('tools') || f.includes('mcp') || name.includes('tool')) {
    return { tier: t('inspector.tier_mcp'), description: t('inspector.desc_mcp') };
  }
  if (f.includes('scanner') || f.includes('indexer') || f.includes('parser') || name.includes('ast')) {
    return { tier: t('inspector.tier_context'), description: t('inspector.desc_context') };
  }
  if (f.includes('gate') || f.includes('guard') || f.includes('circuit')) {
    return { tier: t('inspector.tier_governance'), description: t('inspector.desc_governance') };
  }
  if (f.includes('skills') || name.includes('skill')) {
    return { tier: t('inspector.tier_skills'), description: t('inspector.desc_skills') };
  }
  return { tier: t('inspector.tier_app'), description: t('inspector.desc_app', { kind: node.kind || 'symbol' }) };
}

function getBlastRadiusRisk(node, incomingLen, outgoingLen) {
  const total = (node.inDeg || incomingLen || 0) * 1.5 + (node.outDeg || outgoingLen || 0);
  if (total >= 8) return { label: t('inspector.blast_critical'), color: '#ef4444' };
  if (total >= 4) return { label: t('inspector.blast_moderate'), color: '#f59e0b' };
  if (total >= 1 || (node.kind === 'module' && node.loc > 20)) return { label: t('inspector.blast_low'), color: '#10b981' };
  return { label: t('inspector.blast_isolated'), color: '#94a3b8' };
}

function escapeHtml(str) {
  return String(str || '').replace(/[&<>"']/g, m => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[m]));
}
