'use strict';
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

const digest = body => `sha256:${crypto.createHash('sha256').update(body).digest('hex')}`;
const required = name => {
  const value = process.env[name];
  if (!value || !path.isAbsolute(value)) throw new Error(`${name} must be an absolute path`);
  return value;
};

async function replaceActiveDocument(value) {
  const editor = vscode.window.activeTextEditor;
  assert.ok(editor, 'typed intent scratch must be active');
  const end = editor.document.lineAt(editor.document.lineCount - 1).range.end;
  assert.equal(await editor.edit(edit => edit.replace(new vscode.Range(new vscode.Position(0, 0), end), value)), true);
}

async function run() {
  const compiler = required('SEMAPRAX_VSCODE_COMPILER');
  const manifest = required('SEMAPRAX_VSCODE_MANIFEST');
  const policy = required('SEMAPRAX_VSCODE_POLICY');
  const source = required('SEMAPRAX_VSCODE_SOURCE');
  assert.equal(vscode.workspace.isTrusted, true, 'isolated fixture workspace must be trusted');
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder, 'fixture workspace must be open');
  assert.equal(path.resolve(folder.uri.fsPath), path.resolve(path.dirname(manifest)));

  const settings = vscode.workspace.getConfiguration('semaprax');
  for (const [key, expected] of [['compilerPath', compiler], ['manifestPath', manifest], ['hostPolicyPath', policy]]) {
    const inspected = settings.inspect(key);
    assert.equal(inspected.globalValue, expected, `${key} must be selected globally`);
    assert.equal(inspected.workspaceValue, undefined, `${key} must not be selected by the workspace`);
    assert.equal(inspected.workspaceFolderValue, undefined, `${key} must not be selected by the workspace folder`);
  }

  const extension = vscode.extensions.getExtension('semaprax.semaprax-saved-source');
  assert.ok(extension, 'development extension must be installed');
  assert.equal(extension.packageJSON.version, '0.1.0');
  const api = await extension.activate();
  assert.ok(api && typeof api.execute === 'function', 'test-only extension API must be available');

  const registered = new Set(await vscode.commands.getCommands(true));
  const contributed = extension.packageJSON.contributes.commands.map(row => row.command);
  assert.equal(contributed.length, 26);
  for (const command of contributed) assert.ok(registered.has(command), `${command} must be registered`);

  const sourceBefore = fs.readFileSync(source);
  await api.execute('start');
  let state = api.state();
  assert.equal(state.running, true);
  assert.equal(state.stale, false);
  assert.match(state.status, /^SEMAPRAX: saved source ready$/);
  assert.match(state.image, /^sha256:[0-9a-f]{64}$/);

  await api.execute('openCandidate');
  api.enqueueInput('calculator.add');
  await api.execute('selectTarget');
  api.enqueuePick('rename_declaration');
  await api.execute('newIntent');
  await replaceActiveDocument(JSON.stringify({ kind: 'rename_declaration', target: 'calculator.add', name: 'addition' }, null, 2) + '\n');
  await api.execute('applyIntent');
  api.enqueuePick('src/core.spx');
  await api.execute('previewSourceDiff');

  state = api.state();
  assert.equal(state.target, 'calculator.add');
  assert.match(state.candidate, /^sha256:[0-9a-f]{64}$/);
  assert.equal(state.documents.length >= 3, true);
  const sourceViews = state.documents.filter(row => row.uri.endsWith('/base.spx') || row.uri.endsWith('/candidate.spx'));
  assert.equal(sourceViews.length, 2);
  const base = sourceViews.find(row => row.uri.endsWith('/base.spx'));
  const candidate = sourceViews.find(row => row.uri.endsWith('/candidate.spx'));
  assert.match(base.text, /fn add\(/);
  assert.match(candidate.text, /fn addition\(/);
  assert.deepEqual(fs.readFileSync(source), sourceBefore, 'candidate review must not write canonical source');
  const workflow = state;

  const sourceDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(source));
  const sourceEditor = await vscode.window.showTextDocument(sourceDocument, { preview: false });
  const first = sourceDocument.lineAt(0).range.end;
  assert.equal(await sourceEditor.edit(edit => edit.insert(first, ' ')), true);
  state = api.state();
  assert.equal(state.stale, true);
  assert.equal(state.candidate, null);
  assert.ok(state.documents.every(row => row.text.startsWith('SEMAPRAX view invalidated.')));
  assert.deepEqual(fs.readFileSync(source), sourceBefore, 'dirty-buffer invalidation must not write canonical source');
  await vscode.commands.executeCommand('workbench.action.files.revert');
  assert.deepEqual(fs.readFileSync(source), sourceBefore);

  await api.execute('stop');
  state = api.state();
  assert.equal(state.running, false);
  assert.equal(state.documents.length, 0);
  console.log('SEMAPRAX_VSCODE_HOST_RESULT=' + JSON.stringify({
    schema: 'semaprax.vscode-extension-host-result.v1',
    vscode_version: vscode.version,
    app_name: vscode.env.appName,
    extension_host_exec_path: process.execPath,
    extension_version: extension.packageJSON.version,
    registered_commands: contributed.length,
    image_revision: workflow.image,
    candidate_revision: workflow.candidate,
    source_sha256: digest(sourceBefore),
    typed_intent: 'rename_declaration',
    target: 'calculator.add',
    verified_virtual_diff: true,
    dirty_buffer_invalidated: true,
    source_bytes_unchanged: true
  }));
}

module.exports = { run };
