import { t } from '../i18n/index.js';

function escapeHtml(str) {
  if (!str) return '';
  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

export function renderFunctionInspector(container, node, edges = [], allNodes = [], onJumpNode) {
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

  const fnName = node.label || node.id;
  const isInternal = Boolean(node.is_internal ?? (!node.is_external || node.scope === 'internal'));
  const filePath = node.file || '--';
  const startLine = node.start_line || 1;
  const endLine = node.end_line || (startLine + (node.loc || 1) - 1);
  const loc = node.loc || (endLine - startLine + 1);

  // Incoming callers (calls where target is this function)
  const incoming = edges.filter(e => e.target === node.id);
  // Outgoing callees (calls where source is this function)
  const outgoing = edges.filter(e => e.source === node.id);

  const nodeMap = new Map(allNodes.map(n => [n.id, n]));

  container.innerHTML = `
    <!-- Header -->
    <div style="display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; margin-bottom: 10px;">
      <div style="flex: 1; min-width: 0;">
        <div style="display: flex; gap: 6px; align-items: center; margin-bottom: 4px;">
          <span style="font-size: 10px; text-transform: uppercase; font-weight: 700; color: ${isInternal ? '#38bdf8' : '#a855f7'}; background: ${isInternal ? '#0284c718' : '#7c3aed18'}; border: 1px solid ${isInternal ? '#0284c740' : '#7c3aed40'}; padding: 2px 6px; border-radius: 4px;">
            ${isInternal ? t('graph.internal_badge') : t('graph.external_badge')}
          </span>
          <span style="font-size: 10px; text-transform: uppercase; font-weight: 700; color: #10b981; background: #05966918; border: 1px solid #05966940; padding: 2px 6px; border-radius: 4px;">
            FUNCTION
          </span>
        </div>
        <h4 style="color: #f8fafc; font-size: 14px; margin: 0; font-family: var(--font-mono); word-break: break-all; line-height: 1.3;">
          ${escapeHtml(fnName)}()
        </h4>
        <div style="font-size: 11px; color: #94a3b8; font-family: var(--font-mono); word-break: break-all; margin-top: 4px;">
          📁 ${escapeHtml(filePath)}
        </div>
        <div style="font-size: 10px; color: #64748b; font-family: var(--font-mono); margin-top: 2px;">
          ${t('graph.lines_range', { start: startLine, end: endLine })} (${loc} LOC)
        </div>
      </div>
      <button id="btn-copy-fn-id" title="${t('inspector.copy_id_title')}" style="background: var(--bg-tertiary); border: 1px solid var(--border-color); color: #94a3b8; border-radius: 4px; padding: 4px 8px; font-size: 11px; cursor: pointer;">
        📋
      </button>
    </div>

    <!-- Quick Stats -->
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin-bottom: 12px;">
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">${t('graph.external_incoming')}</div>
        <div style="font-size: 15px; font-weight: 700; color: #a855f7;">${incoming.length}</div>
        <div style="font-size: 9px; color: #64748b;">${t('graph.callers_label')}</div>
      </div>
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">${t('graph.external_outgoing')}</div>
        <div style="font-size: 15px; font-weight: 700; color: #10b981;">${outgoing.length}</div>
        <div style="font-size: 9px; color: #64748b;">${t('graph.callees_label')}</div>
      </div>
    </div>

    <!-- Incoming Callers -->
    <div style="margin-bottom: 12px; border-top: 1px solid var(--border-color); padding-top: 8px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span style="font-size: 11px; font-weight: 600; color: #a855f7;">${t('inspector.callers')} (${incoming.length})</span>
      </div>
      <div style="max-height: 140px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px;">
        ${incoming.length ? incoming.map(e => renderFnRelation(e.source, e.call_line, nodeMap, onJumpNode)).join('') : `
          <span style="font-size: 11px; color: #64748b; font-style: italic;">${t('inspector.no_callers')}</span>
        `}
      </div>
    </div>

    <!-- Outgoing Callees -->
    <div style="border-top: 1px solid var(--border-color); padding-top: 8px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span style="font-size: 11px; font-weight: 600; color: #10b981;">${t('inspector.dependencies')} (${outgoing.length})</span>
      </div>
      <div style="max-height: 140px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px;">
        ${outgoing.length ? outgoing.map(e => renderFnRelation(e.target, e.call_line, nodeMap, onJumpNode)).join('') : `
          <span style="font-size: 11px; color: #64748b; font-style: italic;">${t('inspector.no_dependencies')}</span>
        `}
      </div>
    </div>
  `;

  const copyBtn = container.querySelector('#btn-copy-fn-id');
  if (copyBtn) {
    copyBtn.onclick = () => {
      navigator.clipboard.writeText(fnName).then(() => {
        copyBtn.textContent = '✅';
        setTimeout(() => { copyBtn.textContent = '📋'; }, 1500);
      });
    };
  }

  container.querySelectorAll('.fn-jump-link').forEach(link => {
    link.onclick = () => {
      const targetId = link.getAttribute('data-fn-id');
      if (targetId && onJumpNode) onJumpNode(targetId);
    };
  });
}

function renderFnRelation(nodeId, callLine, nodeMap, onJumpNode) {
  const target = nodeMap.get(nodeId) || { id: nodeId, label: nodeId };
  const shortFile = target.file ? target.file.split(/[/\\]/).pop() : '';

  return `
    <div class="fn-jump-link" data-fn-id="${escapeHtml(target.id)}" style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 5px 8px; cursor: pointer; display: flex; justify-content: space-between; align-items: center;">
      <div style="min-width: 0;">
        <div style="font-family: var(--font-mono); font-size: 11px; color: #e2e8f0; font-weight: 500;">
          ${escapeHtml(target.label || target.id)}()
        </div>
        ${shortFile ? `<div style="font-size: 9px; color: #64748b; font-family: var(--font-mono);">${escapeHtml(shortFile)}</div>` : ''}
      </div>
      ${callLine ? `<span style="font-size: 9px; color: #94a3b8; font-family: var(--font-mono);">L${callLine}</span>` : ''}
    </div>
  `;
}
