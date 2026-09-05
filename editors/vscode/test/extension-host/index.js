'use strict';
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vscode = require('vscode');

// The exact command inventory this extension contributes, in manifest order.
// A removed, renamed, added or reordered command fails here, and every entry
// must also be registered with VS Code; neither half is a count alone.
const CONTRIBUTED = [
  'semaprax.start', 'semaprax.stop', 'semaprax.openCandidate', 'semaprax.selectTarget',
  'semaprax.changeCatalog', 'semaprax.newIntent', 'semaprax.applyIntent', 'semaprax.tryIntent',
  'semaprax.attemptSummary', 'semaprax.attemptDiagnostics', 'semaprax.repairCatalog',
  'semaprax.applyRepair', 'semaprax.discardAttempt', 'semaprax.previewSourceDiff',
  'semaprax.runCandidateTests', 'semaprax.cancelCandidateTests', 'semaprax.openHole',
  'semaprax.selectHole', 'semaprax.holeSummary', 'semaprax.holeFacet', 'semaprax.holeContext',
  'semaprax.showHoleConstructors', 'semaprax.newHoleFillScratch', 'semaprax.suggestHoleFill',
  'semaprax.fillHole', 'semaprax.completeDraft', 'semaprax.discardDraft', 'semaprax.refresh',
  'semaprax.checkProject', 'semaprax.goToDeclaration', 'semaprax.showReferences',
  'semaprax.showDocumentation', 'semaprax.showOwnership', 'semaprax.inspectAgent',
  'semaprax.safeRename', 'semaprax.showCleanupPlan', 'semaprax.runAgentTranscript'
];
// Authority this extension must never contribute or register, whatever a host
// selects. Build, commit and publication stay outside the editor entirely.
const FORBIDDEN = [
  'semaprax.build', 'semaprax.commit', 'semaprax.publish', 'semaprax.approve',
  'semaprax.gitCommit', 'semaprax.installPackage', 'semaprax.runNative'
];

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
  assert.deepEqual(contributed, CONTRIBUTED, 'the contributed command inventory is exact');
  assert.equal(contributed.length, CONTRIBUTED.length);
  assert.equal(new Set(contributed).size, contributed.length, 'no command may be contributed twice');
  for (const command of contributed) assert.ok(registered.has(command), `${command} must be registered`);
  for (const command of FORBIDDEN) {
    assert.equal(contributed.includes(command), false, `${command} must not be contributed`);
    assert.equal(registered.has(command), false, `${command} must not be registered`);
  }
  // Every registered `semaprax.` command must be one this manifest declares:
  // an unlisted registration is as much an inventory break as a missing one.
  assert.deepEqual([...registered].filter(name => name.startsWith('semaprax.')).sort(), [...CONTRIBUTED].sort());

  // Check-on-save and navigation by meaning, against the real compiler. The
  // probe file lives outside the fixture workspace so the workspace bytes stay
  // exactly as they were, which the runner verifies independently.
  const probeDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'semaprax-vscode-probe-'));
  try {
    // An astral character before the reported token: the compiler's byte span
    // and Unicode-scalar column and VS Code's UTF-16 columns all differ here.
    const probe = path.join(probeDirectory, 'astral.spx');
    fs.writeFileSync(probe, 'module probe;\n\n@id("probe.main")\nfn main() -> i64\n{\n    let greeting: string = "\u{1F600}"; undefined_call()\n}\n');
    const failing = await api.checks.check(probe, compiler);
    assert.equal(failing.failure, undefined, 'an ordinary error stream is usable');
    assert.deepEqual(failing.records.map(record => ({ code: record.code, range: record.range })), [
      { code: 'SPX-T203', range: { startLine: 5, startColumn: 33, endLine: 5, endColumn: 49 } }
    ], 'the diagnostic underlines `undefined_call()` in UTF-16 columns');
    const probeText = fs.readFileSync(probe, 'utf8').split('\n')[5];
    assert.equal(probeText.slice(33, 49), 'undefined_call()');
    assert.equal(api.checks.collection.get(vscode.Uri.file(probe)).length, 1);

    // A run whose output the adapter cannot classify must not clear it.
    const broken = path.join(probeDirectory, 'broken-compiler');
    fs.writeFileSync(broken, "#!/bin/sh\necho '{broken json'\nexit 1\n", { mode: 0o700 });
    const unusable = await api.checks.check(probe, broken);
    assert.match(unusable.failure, /neither a diagnostic nor a verified record/);
    assert.equal(unusable.retained, true);
    assert.equal(api.checks.collection.get(vscode.Uri.file(probe)).length, 1, 'a failed check keeps the previous diagnostics');

    const silent = path.join(probeDirectory, 'silent-compiler');
    fs.writeFileSync(silent, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
    const empty = await api.checks.check(probe, silent);
    assert.equal(empty.failure, 'check exited 0 without printing a verified record');
    assert.equal(api.checks.collection.get(vscode.Uri.file(probe)).length, 1);

    // Only a believable verified run clears them.
    fs.writeFileSync(probe, 'module probe;\n\n@id("probe.main")\nfn main() -> i64\n{\n    0\n}\n');
    const verified = await api.checks.check(probe, compiler);
    assert.equal(verified.failure, undefined);
    assert.deepEqual(verified.records, []);
    assert.equal(api.checks.collection.get(vscode.Uri.file(probe)), undefined);

    // The project route: an importing module has no standalone meaning, so
    // `app.spx` resolves its declarations, callers and lenses through the
    // project that owns it and reaches the other two files.
    const app = path.join(path.dirname(manifest), 'src', 'app.spx');
    const appDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(app));
    await vscode.window.showTextDocument(appDocument, { preview: false });
    api.enqueuePick('add');
    const declarations = await api.execute('goToDeclaration');
    const files = [...new Set(declarations.map(item => item.file))].sort();
    assert.equal(files.length, 3, `project navigation must reach every source: ${files}`);
    for (const name of ['app.spx', 'core.spx', 'tests.spx']) {
      assert.ok(files.some(file => path.basename(file) === name), `${name} must be reachable`);
    }
    const chosen = declarations.find(item => item.id === 'calculator.add');
    assert.ok(chosen && path.basename(chosen.file) === 'core.spx');
    assert.equal(path.basename(vscode.window.activeTextEditor.document.uri.fsPath), 'core.spx', 'the selection opens the file the match lives in');
    assert.equal(vscode.window.activeTextEditor.document.getText(vscode.window.activeTextEditor.selection), 'add');

    // Callers cross files through the project's persistent call index.
    api.enqueuePick('add');
    api.enqueuePick('main');
    const callers = await api.execute('showReferences');
    assert.deepEqual(callers.map(item => item.id).sort(), ['calculator.app.main', 'calculator.tests.main']);

    // Code lenses for the importing module come from the project, filtered to
    // the file, without spawning an inevitably failing standalone query.
    const lenses = await api.checks.lensProvider.provideCodeLenses(appDocument);
    assert.equal(lenses.length, 1);
    assert.equal(lenses[0].command.title, '@id calculator.app.main');

    // Navigation reads saved source: a dirty buffer is refused, not guessed.
    const dirtyEditor = await vscode.window.showTextDocument(appDocument, { preview: false });
    assert.equal(await dirtyEditor.edit(edit => edit.insert(appDocument.lineAt(0).range.end, ' ')), true);
    await assert.rejects(api.execute('goToDeclaration'), /Save the file first/);
    assert.deepEqual(await api.checks.lensProvider.provideCodeLenses(appDocument), []);
    await vscode.commands.executeCommand('workbench.action.files.revert');
    // A rename of a project-owned file belongs to the session's typed intent.
    await assert.rejects(api.execute('safeRename'), /saved-source session/);
  } finally {
    fs.rmSync(probeDirectory, { recursive: true, force: true });
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
