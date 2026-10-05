'use strict';
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const vscode = require('vscode');

// The exact command inventory this extension contributes, in manifest order.
// A removed, renamed, added or reordered command fails here, and every entry
// must also be registered with VS Code; neither half is a count alone.
const CONTRIBUTED = [
  'semaprax.start', 'semaprax.startHotReload', 'semaprax.stopHotReload', 'semaprax.hotReloadStatus',
  'semaprax.hotReloadDetail', 'semaprax.hotReloadPlan', 'semaprax.hotReloadActivate', 'semaprax.hotReloadInvoke',
  'semaprax.stop', 'semaprax.openCandidate', 'semaprax.selectTarget',
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
  ,'semaprax.openExplorer', 'semaprax.exploreSelection', 'semaprax.reviewCandidateGraph', 'semaprax.showTokenReport'
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
async function waitForExplorerRender(api, expected) {
  for (let attempt = 0; attempt < 100; attempt++) {
    const found = api.state().explorerRenders.find(render => render.mode === expected.mode && render.target === expected.target && render.side === expected.side && expected.loaded.every(view => render.loaded.includes(view)));
    if (found) return found;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.fail(`Explorer webview did not render ${JSON.stringify(expected)}; actions=${JSON.stringify(api.state().explorerActions)}; replies=${JSON.stringify(api.state().explorerReplies)}`);
}
async function waitForHotReload(api, event, accept = () => true) {
  for (let attempt = 0; attempt < 100; attempt++) {
    const detail = api.state().hotReload;
    if (detail?.event === event && accept(detail)) return detail;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.fail(`Hot reload did not report ${event}: ${JSON.stringify(api.state().hotReload)}`);
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

  const extension = vscode.extensions.getExtension('wavect.semaprax');
  assert.ok(extension, 'the installed extension must be discovered');
  const expectedExtensionPath = required('SEMAPRAX_VSCODE_EXPECTED_EXTENSION_PATH');
  assert.equal(fs.realpathSync(extension.extensionPath), fs.realpathSync(expectedExtensionPath), 'the Extension Host must load the isolated installed VSIX, never the development tree');
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

  // The installed Extension Host starts the source-built interpreter route
  // directly. The fixture exercises the actual child before any mocked
  // protocol row below, so a route/configuration mismatch cannot hide behind
  // the controller tests.
  await api.execute('startHotReload');
  const startedReload = await waitForHotReload(api, 'started');
  assert.match(startedReload.active, /^sha256:[0-9a-f]{64}$/);
  const startedDetail = await api.execute('hotReloadDetail');
  assert.equal(startedDetail.dirty, false);
  assert.equal(startedDetail.sourceChanged, false);
  const app = path.join(folder.uri.fsPath, 'src', 'app.spx');
  const originalApp = fs.readFileSync(app, 'utf8');
  try {
    // B preserves the public callable shape and checked behavior while changing
    // its source; C is deliberately malformed and must leave B active.
    fs.writeFileSync(app, originalApp.replace('multiply(6, 7)', 'multiply(6, 8)'));
    await api.execute('hotReloadPlan');
    const admittedB = await waitForHotReload(api, 'candidate_admitted');
    assert.match(admittedB.pending, /^sha256:[0-9a-f]{64}$/);
    assert.notEqual(admittedB.pending, startedReload.active);
    await api.execute('hotReloadActivate');
    const activatedB = await waitForHotReload(api, 'activated');
    assert.equal(activatedB.active, admittedB.pending);
    fs.writeFileSync(app, 'not valid SEMAPRAX source\n');
    await api.execute('hotReloadPlan');
    const rejectedC = await waitForHotReload(api, 'candidate_rejected');
    assert.equal(rejectedC.active, activatedB.active);
    assert.equal(rejectedC.pending, null);
  } finally {
    fs.writeFileSync(app, originalApp);
  }
  await api.execute('stopHotReload');
  assert.equal(api.state().hotReload, null);

  // Source-Agent is intentionally not selectable by this editor. This
  // scripted protocol child covers the adapter's visible refusal/migration,
  // safe-point wait, and terminal uncertainty states without claiming that
  // the unavailable lane ran.
const fakeCli = path.join(os.tmpdir(), `semaprax-hot-reload-${process.pid}.js`);
fs.writeFileSync(fakeCli, `#!/usr/bin/env node
const readline=require('node:readline');
const revision='sha256:${'f'.repeat(64)}';
let session=0,lastPlan;
const output=value=>console.log(JSON.stringify(value));
readline.createInterface({input:process.stdin}).on('line',line=>{const row=JSON.parse(line);if(row.op==='start')session++;let value={schema:'semaprax.hot-reload-control.v1',id:row.id,event:'started',generation:0,active_project_revision:revision,terminal_uncertainty:false};if(row.op==='status')value={schema:value.schema,id:row.id,event:'rejected',message:'source-Agent development sessions require the authenticated source-live migration adapter'};if(row.op==='plan'&&session===2){lastPlan=row.id;return;}if(row.op==='plan')value={...value,event:'waiting_safe_point'};if(row.op==='activate')value={...value,event:'terminal_uncertainty',terminal_uncertainty:true};if(row.op==='stop')value={schema:value.schema,id:row.id,event:'stopped'};output(value);});
process.on('SIGTERM',()=>{if(lastPlan)output({schema:'semaprax.hot-reload-control.v1',id:lastPlan,event:'status',generation:0,active_project_revision:revision,terminal_uncertainty:false});setTimeout(()=>process.exit(0),25);});
`, { mode: 0o700 });
  try {
    await settings.update('compilerPath', fakeCli, vscode.ConfigurationTarget.Global);
    await api.execute('startHotReload');
    await waitForHotReload(api, 'started');
    const dirtyFile = path.join(folder.uri.fsPath, 'hot-reload-dirty.spx');
    fs.writeFileSync(dirtyFile, 'module hot.reload;\n@id("hot.reload.main") fn main() -> i64 { 0 }\n');
    try {
      const dirtyDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(dirtyFile));
      const dirtyEditor = await vscode.window.showTextDocument(dirtyDocument, { preview: false });
      assert.equal(await dirtyEditor.edit(edit => edit.insert(dirtyDocument.lineAt(0).range.end, ' ')), true);
      assert.equal((await waitForHotReload(api, 'started', detail => detail.dirty)).dirty, true);
      await vscode.commands.executeCommand('workbench.action.files.save');
      assert.equal((await waitForHotReload(api, 'started', detail => detail.sourceChanged)).sourceChanged, true);
    } finally {
      await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
      fs.rmSync(dirtyFile, { force: true });
    }
    await api.execute('hotReloadStatus');
    const migration = await waitForHotReload(api, 'migration_required');
    assert.match(migration.detail, /interpreter sessions only/);
    assert.equal((await api.execute('hotReloadDetail')).event, 'migration_required');
    await api.execute('hotReloadPlan');
    assert.equal((await waitForHotReload(api, 'waiting_safe_point')).dirty, false);
    await api.execute('hotReloadActivate');
    assert.equal((await waitForHotReload(api, 'terminal_uncertainty')).event, 'terminal_uncertainty');
    await api.execute('stopHotReload');
    assert.match(api.state().status, /unknown/);
    // A planning/check reply may arrive after Stop. The controller has already
    // discarded the child, so the late response cannot recreate the session.
    await api.execute('startHotReload');
    await waitForHotReload(api, 'started');
    await api.execute('hotReloadPlan');
    await new Promise(resolve => setTimeout(resolve, 25));
    await api.execute('stopHotReload');
    await new Promise(resolve => setTimeout(resolve, 100));
    assert.equal(api.state().hotReload, null);
    assert.equal(api.state().status, 'SEMAPRAX hot reload: stopped');
    await settings.update('compilerPath', compiler, vscode.ConfigurationTarget.Global);
    assert.equal(api.state().hotReload, null, 'settings change keeps the stopped reload session disposed');
    assert.equal(api.state().status, 'SEMAPRAX: stopped');
  } finally {
    await settings.update('compilerPath', compiler, vscode.ConfigurationTarget.Global);
    fs.rmSync(fakeCli, { force: true });
  }

  // VS Code's real hover provider consumes the compiler's bounded selected
  // import context, even when no prepared index was selected by the host.
  const hoverFile = path.join(folder.uri.fsPath, 'rust-import-hover.spx');
  const hoverPath = 'regex::Regex::is_match';
  const hoverText = value => String(value).replaceAll('&nbsp;', ' ').replaceAll('&amp;', '&').replaceAll('\\_', '_');
  const fixture = path.resolve(__dirname, '../../../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json');
  const indexDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'semaprax-rust-index-host-'));
  const indexFile = path.join(indexDirectory, 'prepared-index.json');
  const fixtureIndex = JSON.parse(fs.readFileSync(fixture, 'utf8')).index;
  assert.equal(fixtureIndex.schema, 'semaprax.rust-api-index.v2');
  fs.writeFileSync(indexFile, JSON.stringify(fixtureIndex) + '\n');
  fs.writeFileSync(hoverFile, `module test.hover;\n@id("rust.host") interface RustHost permits { regex.read } {\n@id("rust.host.method") import rust selected fn is_match from "${hoverPath}" effects { regex.read } failure infallible;\n}\n@id("rust.host.main") fn main() -> i64 { 0 }\n`);
  try {
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(hoverFile));
    await vscode.window.showTextDocument(document, { preview: false });
    const line = document.lineAt(2).text;
    const position = new vscode.Position(2, line.indexOf(hoverPath) + 8);
    const hovers = await vscode.commands.executeCommand('vscode.executeHoverProvider', document.uri, position);
    const hoverTexts = hovers.flatMap(hover => hover.contents.map(part => String(part.value ?? part)));
    assert.ok(hoverTexts.some(value => hoverText(value).includes('Prepared Rust API index required')), `installed extension must show compiler-owned selected-import setup status; observed ${JSON.stringify(hoverTexts)}`);
    await settings.update('rustIndexPath', indexFile, vscode.ConfigurationTarget.Global);
    assert.equal(settings.inspect('rustIndexPath').globalValue, indexFile);
    const cli = spawnSync(compiler, ['context', hoverFile, hoverPath, '--max-bytes', '4096', '--rust-index', indexFile], { encoding: 'utf8', maxBuffer: 8192 });
    assert.equal(cli.status, 0, cli.stderr);
    const selected = JSON.parse(cli.stdout);
    assert.equal(selected.schema, 'semaprax.rust-api-context.v1');
    assert.equal(selected.index.status, 'prepared_metadata');
    assert.equal(selected.selected_import.path, hoverPath);
    assert.equal(selected.selected_import.signature, 'fn is_match(&self, haystack: &str) -> bool');
    assert.equal(selected.selected_import.ownership, 'shared');
    assert.equal(selected.package.name, 'regex');
    assert.equal(selected.package.version, '1.13.1');
    assert.equal(selected.package.cargo_alias, 'regex_alias');
    const preparedHovers = await vscode.commands.executeCommand('vscode.executeHoverProvider', document.uri, position);
    const preparedHoverTexts = preparedHovers.flatMap(hover => hover.contents.map(part => String(part.value ?? part)));
    assert.ok(preparedHovers.some(hover => hover.contents.some(part => {
      const value = hoverText(part.value ?? part);
      return value.includes(selected.selected_import.signature) && value.includes(selected.package.name) && value.includes(selected.package.version) && value.includes(selected.package.cargo_alias) && value.includes(selected.selected_import.ownership);
    })), `installed extension hover must agree with the compiler-owned prepared Regex method context; observed ${JSON.stringify(preparedHoverTexts).slice(0, 4096)}`);
  } finally {
    await settings.update('rustIndexPath', undefined, vscode.ConfigurationTarget.Global);
    fs.unlinkSync(hoverFile);
    fs.rmSync(indexDirectory, { recursive: true, force: true });
  }
  if (process.env.SEMAPRAX_VSCODE_RUST_INDEX_ONLY === '1') {
    console.log('SEMAPRAX_RUST_INDEX_HOST_RESULT=' + JSON.stringify({
      schema: 'semaprax.vscode-rust-index-host-result.v1',
      app_name: vscode.env.appName,
      extension_path: fs.realpathSync(extension.extensionPath),
      installed_vsix: true,
      selected_path: hoverPath,
      signature: 'fn is_match(&self, haystack: &str) -> bool',
      receiver: 'shared',
      package: 'regex 1.13.1',
      cargo_alias: 'regex_alias',
      authority: { build: false, publication: false }
    }));
    return;
  }

  // Token reports are selected local snapshots. This path deliberately runs
  // before any compiler session exists, proving it neither starts one nor
  // asks the MCP host to refresh, test, or otherwise inspect source.
  const reportDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'semaprax-token-report-host-'));
  try {
    const reportSourceBefore = fs.readFileSync(source);
    const sha = character => 'sha256:' + character.repeat(64);
    const projection = counts => ({
      schema: 'semaprax.token-comparison.v1', comparison_identity: sha('a'), report_kind: 'projection', profile: 'graph', root_sha256: sha('b'), selection_sha256: sha('c'), source_revision: '<script>old-revision</script>', producer_options_sha256: sha('d'), baseline: { sha256: sha('e'), utf8_bytes: 100 }, actual: { sha256: sha('f'), utf8_bytes: 120 }, tokenizer: counts.measurement_status === 'measured' ? { name: 'cl100k_base' } : null, counts, baseline_kind: 'same_selected_json', actual_kind: 'compact_model-text', display_lf_in_measurement: false, compiler: {}
    });
    const write = (name, value) => { const file = path.join(reportDirectory, name); fs.writeFileSync(file, JSON.stringify(value)); return vscode.Uri.file(file); };
    const measured = write('growth.json', projection({ measurement_status: 'measured', baseline_tokens: 10, actual_tokens: 12, delta_tokens: -2, delta_fraction: { numerator: -2, denominator: 10 }, delta_percentage: -20 }));
    api.enqueueReport(measured); await api.execute('showTokenReport');
    assert.match(vscode.window.activeTextEditor.document.getText(), /\+2 tokens used versus reference/);
    assert.match(vscode.window.activeTextEditor.document.getText(), /Current revision not verified/);
    assert.match(vscode.window.activeTextEditor.document.getText(), /<script>old-revision<\/script>/, 'untrusted strings stay literal text');
    api.setReportBinding(sha('9'));
    api.enqueueReport(measured); await api.execute('showTokenReport');
    assert.match(vscode.window.activeTextEditor.document.getText(), /Stale\/mismatched report/);
    assert.match(vscode.window.activeTextEditor.document.getText(), /not current-session savings/);
    api.setReportBinding(undefined);
    const unavailable = write('unavailable.json', projection({ measurement_status: 'tokenizer_unavailable', baseline_tokens: null, actual_tokens: null, delta_tokens: null, delta_fraction: null, delta_percentage: null }));
    api.enqueueReport(unavailable); await api.execute('showTokenReport');
    assert.match(vscode.window.activeTextEditor.document.getText(), /Model tokens unavailable/);
    const partial = write('partial.json', { schema: 'semaprax.token-comparison-session.v1', comparison_identity: sha('a'), report_kind: 'session', event_stream_sha256: sha('b'), malformed_events: 0, events: 3, groups: [{ tokenizer: 'cl100k_base', tokenizer_fingerprint: sha('c'), boundary: 'response', reference_kind: 'paired', coverage: { events: 3, token_measured: 2, baseline_available: 2, paired: 1 }, outcomes: { success: 1, error: 1 }, statuses: { measured: 2 }, bytes: 20, tokens: 999, baseline_tokens: 999, paired_actual_tokens: 8, paired_baseline_tokens: 10 }] });
    api.enqueueReport(partial); await api.execute('showTokenReport');
    assert.match(vscode.window.activeTextEditor.document.getText(), /Measured pairs: 1\/3 responses/);
    assert.match(vscode.window.activeTextEditor.document.getText(), /2 tokens saved versus reference/);
    const hostile = path.join(reportDirectory, 'hostile.json'); fs.writeFileSync(hostile, '{"schema":"x","schema":"y"}');
    api.enqueueReport(vscode.Uri.file(hostile)); await assert.rejects(api.execute('showTokenReport'), /Duplicate JSON key/);
    assert.equal(api.state().running, false, 'report snapshots must not start a compiler or MCP session');
    assert.deepEqual(fs.readFileSync(source), reportSourceBefore, 'report snapshots must not write source');
  } finally { fs.rmSync(reportDirectory, { recursive: true, force: true }); }

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

    // A verified record carrying a raw malformed byte is a transport failure:
    // it is neither replacement-decoded nor allowed to clear the diagnostics.
    const corrupt = path.join(probeDirectory, 'corrupt-compiler');
    fs.writeFileSync(corrupt, `#!/bin/sh\nprintf '{"status":"verified","path":"/fixture/\\377.spx","revision":"sha256:${'a'.repeat(64)}"}\\n'\nexit 0\n`, { mode: 0o700 });
    const undecodable = await api.checks.check(probe, corrupt);
    assert.equal(undecodable.failure, 'check output is not valid UTF-8');
    assert.equal(undecodable.retained, true);
    assert.equal(api.checks.collection.get(vscode.Uri.file(probe)).length, 1, 'an undecodable check keeps the previous diagnostics');

    // Only a believable verified run clears them.
    fs.writeFileSync(probe, 'module probe;\n\n@id("probe.main")\nfn main() -> i64\n{\n    0\n}\n');
    const verified = await api.checks.check(probe, compiler);
    assert.equal(verified.failure, undefined);
    assert.deepEqual(verified.records, []);
    // `DiagnosticCollection.get` is declared `Diagnostic[] | undefined`, but the
    // real host returns a frozen `[]` for a URI it holds no entry for. Absence
    // is therefore only observable through `has`, which is the entry predicate.
    assert.equal(api.checks.collection.has(vscode.Uri.file(probe)), false, 'the entry is removed, not emptied');
    assert.deepEqual(api.checks.collection.get(vscode.Uri.file(probe)), []);

    // Overlapping subjects: a second standalone subject also reports the probe
    // file. Each contribution is retained, a clean result for one subject
    // keeps the other's diagnostic, a failed run changes nothing, and only the
    // last owner's clean result removes the entry.
    const other = path.join(probeDirectory, 'other.spx');
    fs.writeFileSync(other, 'module other;\n');
    const overlapError = path.join(probeDirectory, 'overlap-error-compiler');
    fs.writeFileSync(overlapError, `#!/bin/sh\nprintf '%s\\n' '${JSON.stringify({ code: 'SPX-T999', severity: 'error', message: 'overlap', path: probe, location: null, help: null })}'\nexit 1\n`, { mode: 0o700 });
    const overlapClean = path.join(probeDirectory, 'overlap-clean-compiler');
    fs.writeFileSync(overlapClean, `#!/bin/sh\nprintf '%s\\n' '${JSON.stringify({ status: 'verified', path: other, revision: 'sha256:' + 'c'.repeat(64) })}'\nexit 0\n`, { mode: 0o700 });
    fs.writeFileSync(probe, 'module probe;\n\n@id("probe.main")\nfn main() -> i64\n{\n    let greeting: string = "\u{1F600}"; undefined_call()\n}\n');
    assert.equal((await api.checks.check(probe, compiler)).failure, undefined);
    assert.equal((await api.checks.check(other, overlapError)).failure, undefined);
    const codes = () => api.checks.collection.get(vscode.Uri.file(probe)).map(diagnostic => String(diagnostic.code)).sort();
    assert.deepEqual(codes(), ['SPX-T203', 'SPX-T999'], 'both subjects contribute to the shared file');
    fs.writeFileSync(probe, 'module probe;\n\n@id("probe.main")\nfn main() -> i64\n{\n    0\n}\n');
    assert.equal((await api.checks.check(probe, compiler)).failure, undefined);
    assert.deepEqual(codes(), ['SPX-T999'], 'a clean probe check keeps the other subject\'s diagnostic');
    assert.match((await api.checks.check(other, broken)).failure, /neither a diagnostic nor a verified record/);
    assert.deepEqual(codes(), ['SPX-T999'], 'a failed check leaves every contribution unchanged');
    assert.equal((await api.checks.check(other, overlapClean)).failure, undefined);
    assert.equal(api.checks.collection.has(vscode.Uri.file(probe)), false, 'the last owner\'s clean result removes the entry');

    // A query with a malformed row is an invalid result: the commands report
    // that, never "declares nothing" or "nothing calls", and lenses decline.
    const malformedQuery = path.join(probeDirectory, 'malformed-query-compiler');
    const malformedRow = { kind: 'function', id: 'probe.main', name: 'main', persistent: true, signature: 'fn main() -> i64', location: { line: 4, column: 4, start: 34, end: 38 }, effects: 'clock.read', calls: [], called_by: [] };
    fs.writeFileSync(malformedQuery, `#!/bin/sh\nprintf '%s\\n' '${JSON.stringify({ schema: 'semaprax.query.v1', module: 'probe', revision: 'sha256:' + 'b'.repeat(64), filters: {}, matches: [malformedRow] })}'\nexit 0\n`, { mode: 0o700 });
    await settings.update('compilerPath', malformedQuery, vscode.ConfigurationTarget.Global);
    try {
      const probeDocument = await vscode.workspace.openTextDocument(vscode.Uri.file(probe));
      await vscode.window.showTextDocument(probeDocument, { preview: false });
      await assert.rejects(api.execute('goToDeclaration'), /invalid query result/);
      await assert.rejects(api.execute('showReferences'), /invalid query result/);
      assert.deepEqual(await api.checks.lensProvider.provideCodeLenses(probeDocument), []);
    } finally {
      await settings.update('compilerPath', compiler, vscode.ConfigurationTarget.Global);
      await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
    }

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

  // These are actual WebviewPanel instances in the selected Extension Host.
  // The webview bootstrap issues its read-only summary/page requests after the
  // panel is shown; command construction itself must not mutate source or
  // manufacture a candidate.
  const currentExplorer = await api.execute('openExplorer');
  assert.equal(currentExplorer.viewType, 'semapraxExplorer');
  assert.equal(currentExplorer.title, 'SEMAPRAX Explorer');
  assert.match(currentExplorer.webview.html, /default-src 'none'/);
  await waitForExplorerRender(api, { mode: 'overview', target: null, side: 'current', loaded: ['modules', 'declarations'] });
  api.enqueueInput('calculator.add');
  const selectedExplorer = await api.execute('exploreSelection');
  assert.equal(selectedExplorer.viewType, 'semapraxExplorer');
  assert.match(selectedExplorer.webview.html, /context · current/);
  await waitForExplorerRender(api, { mode: 'context', target: 'calculator.add', side: 'current', loaded: ['modules', 'declarations', 'relations', 'frontier'] });

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
  const candidateExplorer = await api.execute('reviewCandidateGraph');
  assert.equal(candidateExplorer.viewType, 'semapraxExplorer');
  assert.match(candidateExplorer.webview.html, /overview · candidate/);
  await waitForExplorerRender(api, { mode: 'overview', target: null, side: 'candidate', loaded: ['modules', 'declarations'] });
  const webviewRenders = api.state().explorerRenders;

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
    webview_rendered_views: webviewRenders,
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
    source_bytes_unchanged: true,
    hot_reload: {
      interpreter_child: true,
      migration_required: true,
      waiting_safe_point: true,
      terminal_unknown: true,
      stale_after_stop: true,
      stop_while_plan_pending: true,
      source_agent_selected: false
    }
  }));
}

module.exports = { run };
