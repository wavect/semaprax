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
  const hostPolicy = JSON.parse(fs.readFileSync(policy, 'utf8'));
  assert.deepEqual(hostPolicy.test_policy, {
    max_steps: 100000,
    max_execution_bytes: 65536,
    max_report_bytes: 262144
  }, 'candidate test limits must be selected by the startup host policy');
  assert.equal(hostPolicy.candidate_prepare, true);
  assert.equal(hostPolicy.build_enabled, false);
  assert.equal(hostPolicy.git_commit, null);
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
  assert.equal(contributed.length, 28);
  for (const command of contributed) assert.ok(registered.has(command), `${command} must be registered`);
  for (const command of ['semaprax.build', 'semaprax.commit', 'semaprax.publish']) {
    assert.equal(contributed.includes(command), false, `${command} must not be contributed`);
  }

  const sourceBefore = fs.readFileSync(source);
  await api.execute('start');
  let state = api.state();
  assert.equal(state.running, true);
  assert.equal(state.stale, false);
  assert.match(state.status, /^SEMAPRAX: saved source ready$/);
  assert.match(state.image, /^sha256:[0-9a-f]{64}$/);
  const discoveredTaskTools = [
    'candidate/test-task-start',
    'candidate/test-task-status',
    'candidate/test-task-cancel',
    'candidate/test-task-result'
  ];
  for (const method of discoveredTaskTools) assert.ok(state.tools.includes(method), `${method} must be selected at startup`);
  for (const method of ['candidate/test', 'candidate/build', 'candidate/commit']) {
    assert.equal(state.tools.includes(method), false, `${method} must remain outside the editor catalogue`);
  }

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

  const documentsBeforeCancellation = state.documents.length;
  const cancelledRun = api.execute('runCandidateTests');
  await api.execute('cancelCandidateTests');
  const cancelledStatus = await cancelledRun;
  assert.equal(cancelledStatus.schema, 'semaprax.image-candidate-test-task-cancel.v1');
  assert.equal(cancelledStatus.state, 'cancelled');
  assert.equal(cancelledStatus.cancellation_requested, true);
  assert.equal(cancelledStatus.before_step, 1);
  assert.equal(cancelledStatus.steps_used, 0);
  assert.equal(cancelledStatus.report_digest, null);
  assert.equal(cancelledStatus.passed, null);
  assert.equal(cancelledStatus.source_authority, false);
  assert.deepEqual(cancelledStatus.authority, {
    source_write: false,
    process: false,
    network: false,
    target_runtime: false,
    publication: false
  });
  state = api.state();
  assert.equal(state.status, 'SEMAPRAX: candidate tests cancelled');
  assert.equal(state.testTask, null);
  assert.equal(state.testTaskUsed, true);
  assert.equal(state.documents.length, documentsBeforeCancellation, 'cancelled task must not expose a report');
  assert.deepEqual(fs.readFileSync(source), sourceBefore, 'task cancellation must not write canonical source');

  await api.execute('stop');
  await api.execute('start');
  await api.execute('openCandidate');
  const sourceDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(source));
  const sourceEditor = await vscode.window.showTextDocument(sourceDocument, { preview: false });
  const first = sourceDocument.lineAt(0).range.end;
  const invalidatedRun = api.execute('runCandidateTests');
  assert.equal(await sourceEditor.edit(edit => edit.insert(first, ' ')), true);
  await assert.rejects(invalidatedRun, /Source or candidate changed while the test task was pending/);
  state = api.state();
  assert.equal(state.stale, true);
  assert.equal(state.candidate, null);
  assert.equal(state.testTask, null);
  assert.equal(state.testTaskUsed, false);
  assert.ok(state.documents.every(row => row.text.startsWith('SEMAPRAX view invalidated.')));
  assert.deepEqual(fs.readFileSync(source), sourceBefore, 'dirty-buffer invalidation must not write canonical source');
  await vscode.commands.executeCommand('workbench.action.files.revert');
  assert.deepEqual(fs.readFileSync(source), sourceBefore);

  await api.execute('stop');
  state = api.state();
  assert.equal(state.running, false);
  assert.equal(state.documents.length, 0);
  console.log('SEMAPRAX_VSCODE_HOST_RESULT=' + JSON.stringify({
    schema: 'semaprax.vscode-extension-host-result.v2',
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
    startup_test_grant: hostPolicy.test_policy,
    discovered_task_tools: discoveredTaskTools,
    explicit_cooperative_cancellation: true,
    cancellation: {
      state: cancelledStatus.state,
      before_step: cancelledStatus.before_step,
      steps_used: cancelledStatus.steps_used,
      report_released: false,
      source_authority: cancelledStatus.source_authority
    },
    test_task_authority: cancelledStatus.authority,
    pending_task_dirty_buffer_invalidated: true,
    authority: {
      source_write: false,
      build: false,
      commit: false,
      publication: false
    },
    dirty_buffer_invalidated: true,
    source_bytes_unchanged: true
  }));
}

module.exports = { run };
