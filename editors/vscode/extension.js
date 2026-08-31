'use strict';
const vscode = require('vscode');
const path = require('node:path');
const { spawn } = require('node:child_process');
const crypto = require('node:crypto');
const { McpClient, parse, digest, invalidates } = require('./protocol');
const { fetchReview } = require('./review');
let stopActive = () => {};
function activate(context) {
  let client, config, image, candidate, target, stale = true, epoch = 0, busy = false;
  let watchers = [];
  const documents = new Map(), scratch = new Set(), changed = new vscode.EventEmitter();
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
  status.text = 'SEMAPRAX: stopped'; status.show();
  const clear = label => {
    epoch++; stale = true; candidate = undefined; target = undefined;
    for (const uri of documents.keys()) { documents.set(uri, 'SEMAPRAX view invalidated. Explicitly refresh saved source and open a new candidate.'); changed.fire(vscode.Uri.parse(uri)); }
    status.text = `SEMAPRAX: ${label}`;
  };
  const stop = () => {
    const old = client; client = undefined; image = undefined;
    for (const watcher of watchers) watcher.dispose(); watchers = [];
    clear('stopped'); documents.clear(); scratch.clear(); if (old) old.stop();
  };
  stopActive = stop;
  const discardError = message => { const error = new Error(message); error.discardOnly = true; return error; };
  function saved() {
    if (!vscode.workspace.isTrusted) throw new Error('A trusted workspace is required');
    if (vscode.workspace.textDocuments.some(doc => doc.isDirty && (doc.uri.path.endsWith('.spx') || doc.uri.fsPath === config?.manifest || path.basename(doc.uri.path) === 'semaprax.toml'))) {
      clear('unsaved source'); throw discardError('Save or revert all source and manifest buffers first');
    }
  }
  function configured() {
    const settings = vscode.workspace.getConfiguration('semaprax'); const result = {};
    for (const [key, field] of [['compilerPath', 'compiler'], ['manifestPath', 'manifest'], ['hostPolicyPath', 'policy']]) {
      const inspected = settings.inspect(key);
      if (!inspected || inspected.workspaceValue !== undefined || inspected.workspaceFolderValue !== undefined) throw new Error('Only user/machine SEMAPRAX path settings are permitted');
      const value = inspected.globalValue;
      if (typeof value !== 'string' || !path.isAbsolute(value) || /[\u0000-\u001f\u007f]/.test(value)) throw new Error(`Set an absolute user setting: semaprax.${key}`);
      result[field] = value;
    }
    if (path.basename(result.manifest) !== 'semaprax.toml') throw new Error('Select a saved semaprax.toml manifest');
    return result;
  }
  async function invoke(method, params, permitStale = false) {
    saved(); if (!client || client.closed || (!permitStale && stale)) throw discardError('Start or explicitly refresh the session');
    const current = epoch, selected = client;
    try {
      const response = await selected.call(method, params);
      if (epoch !== current || client !== selected) throw discardError('Source changed while the request was pending; result discarded');
      if (method !== 'workspace/open' && method !== 'workspace/refresh' && response.image_revision !== image) {
        const error = new Error('Unexpected image revision'); selected.fail(error); throw error;
      }
      return response;
    } catch (error) { if (invalidates(error)) clear(error.semantic ? 'source binding rejected; refresh required' : 'request failed'); throw error; }
  }
  const requireCandidate = () => { saved(); if (stale || !candidate) throw new Error('Open a current candidate first'); };
  const invalidResponse = message => { const error = new Error(message); if (client) client.fail(error); throw error; };
  function ensureEpoch(current) {
    saved(); if (epoch !== current || stale || !client || client.closed) throw discardError('Editor view invalidated while awaiting input');
  }
  async function virtual(text, suffix, language = 'plaintext', current = epoch) {
    ensureEpoch(current);
    const retained = [...documents.values()].reduce((sum, value) => sum + Buffer.byteLength(value), 0);
    if (Buffer.byteLength(text) > 16 * 1024 * 1024 || retained + Buffer.byteLength(text) > 32 * 1024 * 1024 || documents.size >= 64) throw new Error('Virtual document budget reached; restart session');
    const uri = vscode.Uri.from({ scheme: 'semaprax-review', path: '/' + crypto.randomUUID() + '/' + suffix });
    documents.set(uri.toString(), text);
    try {
      const doc = await vscode.workspace.openTextDocument(uri); ensureEpoch(current);
      await vscode.languages.setTextDocumentLanguage(doc, language); ensureEpoch(current); return uri;
    } catch (error) { documents.delete(uri.toString()); changed.fire(uri); throw error; }
  }
  async function catalog() {
    requireCandidate(); if (!target) throw new Error('Select a stable target ID first');
    const response = await invoke('change/catalog', { image_revision: image, candidate_revision: candidate, target });
    if (response.payload.schema !== 'semaprax.project-change-catalog.v1' || response.payload.candidate_digest !== candidate || response.payload.target !== target || response.payload.source_authority !== false || !Array.isArray(response.payload.operations)) invalidResponse('Unexpected target catalog');
    return response.payload;
  }
  const commands = {
    async start() {
      saved(); stop(); config = configured(); saved();
      const child = spawn(config.compiler, ['serve-workspace-mcp', config.manifest, config.policy], { shell: false, windowsHide: true, cwd: path.dirname(config.manifest), stdio: ['pipe', 'pipe', 'pipe'] });
      const selected = new McpClient(child, () => { if (client === selected) clear('terminal; restart required'); });
      client = selected;
      try {
        await selected.initialize();
        const response = await invoke('workspace/open', {}, true);
        if (response.payload.schema !== 'semaprax.image-agent-workspace.v1' || response.payload.state !== 'open' || response.payload.image_revision !== response.image_revision || response.payload.project_revision !== response.project_revision) invalidResponse('Invalid workspace handle');
        image = response.image_revision; stale = false;
        const watch = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(path.dirname(config.manifest), '**/*'));
        const hint = uri => { if (uri.fsPath.endsWith('.spx') || uri.fsPath === config.manifest) clear('disk changed; refresh required'); };
        watchers = [watch, watch.onDidChange(hint), watch.onDidCreate(hint), watch.onDidDelete(hint)];
        status.text = 'SEMAPRAX: saved source ready';
      } catch (error) { stop(); throw error; }
    },
    async stop() { stop(); },
    async openCandidate() {
      const response = await invoke('candidate/open', { image_revision: image });
      if (response.payload.schema !== 'semaprax.image-candidate-handle.v1' || !digest(response.payload.candidate_revision) || response.payload.source_authority !== false) invalidResponse('Invalid candidate handle');
      candidate = response.payload.candidate_revision; target = undefined; status.text = 'SEMAPRAX: candidate ' + candidate.slice(7, 19);
    },
    async selectTarget() {
      requireCandidate(); const current = epoch;
      const selection = await vscode.window.showInputBox({ prompt: 'Exact declaration stable ID (not its display name)', ignoreFocusOut: true });
      ensureEpoch(current);
      if (selection === undefined) return;
      if (!selection || Buffer.byteLength(selection) > 512 || /[\u0000-\u001f\u007f]/.test(selection)) throw new Error('Invalid stable ID');
      target = selection; await catalog(); ensureEpoch(current);
    },
    async changeCatalog() {
      const current = epoch; const value = await catalog(); ensureEpoch(current);
      const uri = await virtual(JSON.stringify(value, null, 2), 'change-catalog.json', 'json', current); ensureEpoch(current);
      await vscode.window.showTextDocument(uri, { preview: true }); ensureEpoch(current);
    },
    async newIntent() {
      const current = epoch; const value = await catalog(); ensureEpoch(current);
      const operation = await vscode.window.showQuickPick(value.operations.map(row => ({ label: row.kind, description: 'Compiler-described intention; fill required fields', row })), { title: 'Choose typed intention' });
      ensureEpoch(current);
      if (!operation) return;
      const uri = await virtual(JSON.stringify(operation.row, null, 2), 'constructor.json', 'json', current); ensureEpoch(current);
      await vscode.window.showTextDocument(uri, { preview: true }); ensureEpoch(current);
      const doc = await vscode.workspace.openTextDocument({ language: 'json', content: JSON.stringify({ kind: operation.label, target }, null, 2) + '\n' });
      ensureEpoch(current); scratch.add(doc.uri.toString());
      try { await vscode.window.showTextDocument(doc, { preview: false }); ensureEpoch(current); }
      catch (error) { scratch.delete(doc.uri.toString()); throw error; }
    },
    async applyIntent() {
      requireCandidate(); const doc = vscode.window.activeTextEditor?.document;
      if (!doc || !scratch.has(doc.uri.toString()) || doc.languageId !== 'json') throw new Error('Use New Typed Intent Scratch, then edit and apply that JSON document');
      const intent = parse(doc.getText(), 60 * 1024, true);
      const value = await catalog();
      if (intent.target !== target || !value.operations.some(row => row.kind === intent.kind)) throw new Error('Intent must target the selected ID and a catalogued operation');
      const response = await invoke('candidate/apply-intent', { image_revision: image, candidate_revision: candidate, intent });
      if (response.payload.schema !== 'semaprax.image-candidate-handle.v1' || !digest(response.payload.candidate_revision) || response.payload.source_authority !== false) invalidResponse('Invalid candidate result');
      candidate = response.payload.candidate_revision; status.text = 'SEMAPRAX: candidate ' + candidate.slice(7, 19);
    },
    async previewSourceDiff() {
      requireCandidate(); const current = epoch;
      let report;
      try { report = await fetchReview((method, params) => invoke(method, params), image, candidate); }
      catch (error) { if (!error.semantic && !error.discardOnly && client) client.fail(error); throw error; }
      const file = await vscode.window.showQuickPick(report.files.map(row => ({ label: row.path, row })), { title: 'Verified changed source files (read-only)' });
      if (!file) return; ensureEpoch(current);
      const left = await virtual(file.row.base_source, 'base.spx', 'plaintext', current), right = await virtual(file.row.candidate_source, 'candidate.spx', 'plaintext', current);
      ensureEpoch(current);
      await vscode.commands.executeCommand('vscode.diff', left, right, `SEMAPRAX ${file.label} · ${report.report_revision.slice(7, 19)} · read-only`);
      ensureEpoch(current);
    },
    async refresh() {
      saved(); if (!client || !image) throw new Error('Start a session first');
      const preview = await invoke('workspace/refresh-preview', { image_revision: image }, true);
      const observed = preview.payload.observed_project_revision;
      if (preview.payload.schema !== 'semaprax.image-workspace-refresh-preview.v1' || preview.payload.old_image_revision !== image || !digest(observed) || !digest(preview.payload.observed_image_revision) || preview.payload.source_authority !== false || preview.payload.current_state_replaced !== false || preview.payload.requires_explicit_refresh !== true) invalidResponse('Invalid refresh preview');
      const decision = await vscode.window.showInformationMessage('Replace the held image with this saved-source revision? Candidate selection and previews will be cleared.', { modal: true }, 'Refresh Saved Source');
      if (decision !== 'Refresh Saved Source') return;
      const response = await invoke('workspace/refresh', { image_revision: image, expected_new_project_revision: observed }, true);
      if (response.payload.schema !== 'semaprax.image-workspace-refresh.v1' || response.payload.image_revision !== response.image_revision || response.payload.old_image_revision !== image || response.payload.project_revision !== observed || response.project_revision !== observed || response.payload.source_authority !== false) invalidResponse('Invalid refreshed image');
      clear('saved source refreshed'); image = response.image_revision; stale = false;
    }
  };
  context.subscriptions.push(status, changed, vscode.workspace.registerTextDocumentContentProvider('semaprax-review', {
    onDidChange: changed.event,
    provideTextDocumentContent(uri) { if (!documents.has(uri.toString())) throw new Error('Unknown virtual source reference'); return documents.get(uri.toString()); }
  }), vscode.workspace.onDidChangeTextDocument(event => {
    if (event.document.isDirty && (event.document.uri.path.endsWith('.spx') || path.basename(event.document.uri.path) === 'semaprax.toml')) clear('unsaved source');
  }), vscode.workspace.onDidChangeConfiguration(event => { if (event.affectsConfiguration('semaprax')) stop(); }), { dispose: stop });
  for (const [name, command] of Object.entries(commands)) context.subscriptions.push(vscode.commands.registerCommand('semaprax.' + name, async () => {
    if (name === 'stop') { stop(); return; }
    if (busy) { void vscode.window.showWarningMessage('A SEMAPRAX command is already pending'); return; }
    busy = true;
    try { await command(); } catch (error) { void vscode.window.showErrorMessage(String(error.message || error).slice(0, 4096)); } finally { busy = false; }
  }));
}
function deactivate() { stopActive(); }
module.exports = { activate, deactivate };
