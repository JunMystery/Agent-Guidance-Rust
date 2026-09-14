import { t } from '../i18n/index.js';

function escapeHtml(str) {
  if (!str) return '';
  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

export function renderFileInspector(container, node, edges = [], allNodes = [], onJumpFile, onDrilldown) {
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

  const filePath = node.file || node.path || node.id;
  const fileName = node.label || filePath.split(/[/\\]/).pop() || filePath;

  // 1-N Dependencies (Outgoing: node -> target)
  const outgoingEdges = edges.filter(e => e.source === node.id);
  // N-1 Dependents (Incoming: source -> node)
  const incomingEdges = edges.filter(e => e.target === node.id);

  container.innerHTML = `
    <!-- File Header & Metadata -->
    <div style="display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; margin-bottom: 12px;">
      <div style="flex: 1; min-width: 0;">
        <span style="font-size: 10px; text-transform: uppercase; font-weight: 700; color: #38bdf8; background: #0284c718; border: 1px solid #0284c740; padding: 2px 6px; border-radius: 4px; display: inline-block; margin-bottom: 4px;">
          FILE
        </span>
        <h4 style="color: #f8fafc; font-size: 14px; margin: 0; font-family: var(--font-mono); word-break: break-all; line-height: 1.3;">
          ${escapeHtml(fileName)}
        </h4>
        <div style="font-size: 11px; color: #64748b; font-family: var(--font-mono); word-break: break-all; margin-top: 2px;">
          ${escapeHtml(filePath)}
        </div>
      </div>
      <button id="btn-copy-file-path" title="${t('inspector.copy_id_title')}" style="background: var(--bg-tertiary); border: 1px solid var(--border-color); color: #94a3b8; border-radius: 4px; padding: 4px 8px; font-size: 11px; cursor: pointer;">
        📋
      </button>
    </div>

    <!-- Quick Action: Drill-down Functions -->
    <div style="margin-bottom: 12px;">
      <button id="btn-action-drilldown" class="btn btn-primary" style="width: 100%; display: flex; align-items: center; justify-content: center; gap: 6px; padding: 6px 12px; font-size: 12px; font-weight: 600;">
        ${t('graph.drilldown_action')}
      </button>
    </div>

    <!-- Stats Grid -->
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin-bottom: 12px;">
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">LOC</div>
        <div style="font-size: 15px; font-weight: 700; color: #38bdf8;">${node.loc || 0}</div>
        <div style="font-size: 9px; color: #64748b;">${t('graph.lines_of_code')}</div>
      </div>
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">SYMBOLS</div>
        <div style="font-size: 15px; font-weight: 700; color: #a78bfa;">${node.total_symbols ?? node.symbol_count ?? 0}</div>
        <div style="font-size: 9px; color: #64748b;">${t('graph.functions_types')}</div>
      </div>
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">${t('graph.dependencies_1_n')}</div>
        <div style="font-size: 15px; font-weight: 700; color: #10b981;">${outgoingEdges.length}</div>
        <div style="font-size: 9px; color: #64748b;">${t('graph.outgoing_files')}</div>
      </div>
      <div style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px;">
        <div style="font-size: 10px; color: #94a3b8;">${t('graph.dependents_n_1')}</div>
        <div style="font-size: 15px; font-weight: 700; color: #f59e0b;">${incomingEdges.length}</div>
        <div style="font-size: 9px; color: #64748b;">${t('graph.calling_files')}</div>
      </div>
    </div>

    <!-- 1-N Dependencies Section -->
    <div style="margin-bottom: 12px; border-top: 1px solid var(--border-color); padding-top: 10px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span style="font-size: 11px; font-weight: 600; color: #10b981;">${t('graph.dependencies_1_n')} (${outgoingEdges.length})</span>
        <span style="font-size: 10px; color: #64748b;">${t('graph.files_called_by_this')}</span>
      </div>
      <div style="max-height: 150px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px;">
        ${outgoingEdges.length ? outgoingEdges.map(e => renderEdgeItem(e, 'target', onJumpFile)).join('') : `
          <span style="font-size: 11px; color: #64748b; font-style: italic;">${t('graph.no_dependencies_file')}</span>
        `}
      </div>
    </div>

    <!-- N-1 Dependents Section -->
    <div style="border-top: 1px solid var(--border-color); padding-top: 10px;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span style="font-size: 11px; font-weight: 600; color: #f59e0b;">${t('graph.dependents_n_1')} (${incomingEdges.length})</span>
        <span style="font-size: 10px; color: #64748b;">${t('graph.files_calling_this')}</span>
      </div>
      <div style="max-height: 150px; overflow-y: auto; display: flex; flex-direction: column; gap: 4px;">
        ${incomingEdges.length ? incomingEdges.map(e => renderEdgeItem(e, 'source', onJumpFile)).join('') : `
          <span style="font-size: 11px; color: #64748b; font-style: italic;">${t('graph.no_dependents_file')}</span>
        `}
      </div>
    </div>
  `;

  // Attach event handlers
  const drilldownBtn = container.querySelector('#btn-action-drilldown');
  if (drilldownBtn && onDrilldown) {
    drilldownBtn.onclick = () => onDrilldown(filePath);
  }

  const copyBtn = container.querySelector('#btn-copy-file-path');
  if (copyBtn) {
    copyBtn.onclick = () => {
      navigator.clipboard.writeText(filePath).then(() => {
        copyBtn.textContent = '✅';
        setTimeout(() => { copyBtn.textContent = '📋'; }, 1500);
      });
    };
  }

  container.querySelectorAll('.file-edge-link').forEach(elLink => {
    elLink.onclick = () => {
      const targetId = elLink.getAttribute('data-target-file');
      if (targetId && onJumpFile) onJumpFile(targetId);
    };
  });
}

function renderEdgeItem(edge, targetProp, onJumpFile) {
  const fileKey = edge[targetProp];
  const shortName = fileKey ? fileKey.split(/[/\\]/).pop() : fileKey;
  const calls = edge.calls || [];
  const weight = edge.weight || calls.length || 1;
  const callsLabel = weight > 1 ? t('graph.calls_plural') : t('graph.calls_singular');

  return `
    <div class="file-edge-link" data-target-file="${escapeHtml(fileKey)}" style="background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px; padding: 6px 8px; cursor: pointer;">
      <div style="display: flex; align-items: center; justify-content: space-between;">
        <span style="font-family: var(--font-mono); font-size: 11px; color: #e2e8f0; font-weight: 500;">
          📁 ${escapeHtml(shortName)}
        </span>
        <span style="font-size: 10px; color: #38bdf8; background: #0284c718; padding: 1px 5px; border-radius: 3px;">
          ${weight} ${callsLabel}
        </span>
      </div>
      ${calls.length ? `
        <div style="margin-top: 4px; font-size: 9px; color: #94a3b8; font-family: var(--font-mono); display: flex; flex-direction: column; gap: 2px;">
          ${calls.slice(0, 3).map(c => {
            const caller = c.caller || c.source_func || c.caller_fn || '?';
            const callee = c.callee || c.target_func || c.callee_fn || '?';
            const line = c.call_line ?? c.line ?? '?';
            return `<div>• ${escapeHtml(caller)}() ➔ ${escapeHtml(callee)}() <span style="color: #64748b;">(L${escapeHtml(line)})</span></div>`;
          }).join('')}
          ${calls.length > 3 ? `<div style="color: #64748b; font-style: italic;">+ ${calls.length - 3} ${t('graph.more_calls')}</div>` : ''}
        </div>
      ` : ''}
    </div>
  `;
}
