'use strict';
// Real Extension Host journey for compiler selection and repair (INSTALL-09).
// Launched by scripts/run-compiler-host-journey.js against a clean editor
// profile, a real installed compiler and a freshly generated starter project.
// No advanced-session setting (manifestPath, hostPolicyPath) is ever set.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vscode = require('vscode');

const required = name => {
  const value = process.env[name];
  if (!value || !path.isAbsolute(value)) throw new Error(`${name} must be an absolute path`);
  return value;
};
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(what, predicate, attempts = 200) {
  for (let i = 0; i < attempts; i++) {
    const value = await predicate();
    if (value) return value;
    await sleep(50);
  }
  assert.fail(`timed out waiting for ${what}`);
}

async function run() {
  const compiler = required('SEMAPRAX_HOST_COMPILER');
  const project = required('SEMAPRAX_HOST_PROJECT');
  const userSettingsFile = required('SEMAPRAX_HOST_USER_SETTINGS');
  const expectedVersion = process.env.SEMAPRAX_HOST_VERSION || '';
  const extension = vscode.extensions.getExtension('wavect.semaprax');
  assert.ok(extension, 'extension under development must be discovered');
  const api = await extension.activate();
  assert.ok(api?.compilerSetup && api?.checks, 'test API must expose compiler setup and checks');
  assert.equal(vscode.workspace.isTrusted, true);
  assert.equal(path.resolve(vscode.workspace.workspaceFolders[0].uri.fsPath), path.resolve(project));
  const setup = api.compilerSetup;
  const settings = () => vscode.workspace.getConfiguration('semaprax');
  const inspect = () => settings().inspect('compilerPath');
  const savedUserSetting = () => { try { return JSON.parse(fs.readFileSync(userSettingsFile, 'utf8'))['semaprax.compilerPath']; } catch { return undefined; } };
  const stateName = () => setup.state().state;

  // Stubs: the real flow runs, only the two pickers and any message are scripted.
  const window = vscode.window;
  const original = { pick: window.showQuickPick, open: window.showOpenDialog, info: window.showInformationMessage, error: window.showErrorMessage, warn: window.showWarningMessage };
  const messages = [], picks = [];
  let nextPick, nextOpen;
  window.showQuickPick = async (items, options) => { picks.push({ items, options }); const choice = nextPick; nextPick = undefined; return typeof choice === 'function' ? choice(items) : choice; };
  window.showOpenDialog = async () => { const value = nextOpen; nextOpen = undefined; return value; };
  const record = kind => async (message, ...rest) => { messages.push({ kind, message, modal: Boolean(rest[0]?.modal) }); return undefined; };
  window.showInformationMessage = record('info'); window.showErrorMessage = record('error'); window.showWarningMessage = record('warning');
  const browse = items => items.find(item => item.action === 'browse');

  const app = path.join(project, 'src', 'app.spx');
  const good = fs.readFileSync(app, 'utf8');
  assert.ok(good.includes('add(19, 23)'));
  const bad = good.replace('add(19, 23)', 'add(19, true)');
  const appUri = vscode.Uri.file(app);
  const document = await vscode.workspace.openTextDocument(appUri);
  const editor = await vscode.window.showTextDocument(document, { preview: false });
  const writeAndSave = async text => {
    const end = editor.document.lineAt(editor.document.lineCount - 1).range.end;
    assert.equal(await editor.edit(edit => edit.replace(new vscode.Range(0, 0, end.line, end.character), text)), true);
    assert.equal(await editor.document.save(), true);
  };
  const diagnostics = () => api.checks.collection.get(appUri) || [];
  const quiet = async () => { messages.length = 0; await writeAndSave(bad); await sleep(1500); };

  try {
    // 1. Clean profile: nothing selected, status asks for a compiler, save is silent.
    assert.equal(inspect().globalValue, undefined, 'clean profile has no compiler selected');
    assert.equal(stateName(), 'missing');
    assert.match(setup.item.text, /select compiler/);
    assert.equal(setup.item.command, 'semaprax.configureCompiler');
    await quiet();
    assert.equal(diagnostics().length, 0, 'no compiler, no diagnostics');
    assert.deepEqual(messages, [], 'saving a .spx file without a compiler must show no prompt');
    assert.equal(stateName(), 'missing');

    // 2. A workspace-level compilerPath is ignored.
    // The setting is machine-scoped, so the API refuses a workspace write; a
    // checked-in .vscode/settings.json is the realistic way a project would try.
    const workspaceSettings = path.join(project, '.vscode', 'settings.json');
    fs.mkdirSync(path.dirname(workspaceSettings), { recursive: true });
    fs.writeFileSync(workspaceSettings, JSON.stringify({ 'semaprax.compilerPath': compiler }));
    try {
      await sleep(1000);
      await setup.refresh();
      assert.equal(inspect().globalValue, undefined);
      assert.equal(stateName(), 'missing', 'a workspace compilerPath must not select a compiler');
      assert.match(setup.item.text, /select compiler/);
      await quiet();
      assert.equal(diagnostics().length, 0, 'a workspace compilerPath must not run diagnostics');
      assert.deepEqual(messages, []);
    } finally {
      fs.rmSync(path.dirname(workspaceSettings), { recursive: true, force: true });
      await sleep(500);
    }

    // 3. Cancel changes nothing, at the list and at the file picker.
    nextPick = undefined;
    assert.equal(await vscode.commands.executeCommand('semaprax.configureCompiler'), undefined);
    assert.equal(picks.length >= 1, true, 'the real command opened the compiler list');
    assert.match(picks.at(-1).options.title, /Configure Compiler/);
    assert.equal(inspect().globalValue, undefined);
    nextPick = browse; nextOpen = undefined;
    assert.equal(await setup.configure(), undefined);
    nextPick = browse; nextOpen = [];
    assert.equal(await setup.configure(), undefined);
    assert.equal(inspect().globalValue, undefined);
    assert.equal(savedUserSetting(), undefined);
    assert.equal(stateName(), 'missing');

    // 4. A non-compiler selection is refused and persists nothing.
    const notCompiler = path.join(os.tmpdir(), `semaprax-host-notcompiler-${process.pid}`);
    fs.writeFileSync(notCompiler, '#!/bin/sh\necho nope\n', { mode: 0o700 });
    try {
      nextPick = browse; nextOpen = [vscode.Uri.file(notCompiler)];
      assert.equal(await setup.configure(), undefined);
      assert.equal(inspect().globalValue, undefined, 'a failed probe persists nothing');
      assert.ok(messages.some(row => row.kind === 'error' && /not changed/.test(row.message)));
    } finally { fs.rmSync(notCompiler, { force: true }); }

    // 5. Select the real installed compiler through the real command.
    nextPick = browse; nextOpen = [vscode.Uri.file(compiler)];
    assert.equal(await vscode.commands.executeCommand('semaprax.configureCompiler'), compiler);
    assert.equal(inspect().globalValue, compiler, 'selection persists in the user (global) setting');
    assert.equal(inspect().workspaceValue, undefined);
    await until('user settings.json to record the selection', () => savedUserSetting() === compiler);
    assert.equal(stateName(), 'ready');
    assert.match(setup.item.text, /compiler \d+\.\d+\.\d+/);
    if (expectedVersion) assert.ok(setup.item.text.includes(expectedVersion), `status shows version ${expectedVersion}: ${setup.item.text}`);
    assert.equal(setup.state().advanced, false, 'ready without any advanced session setting');
    assert.equal(settings().inspect('manifestPath').globalValue, undefined);
    assert.equal(settings().inspect('hostPolicyPath').globalValue, undefined);
    assert.ok(messages.some(row => row.kind === 'info' && /selected/.test(row.message)));

    // 6. Ordinary diagnostics on save, then clearing.
    messages.length = 0;
    await writeAndSave(bad);
    const found = await until('a diagnostic for the deliberate error', () => diagnostics().length > 0 && diagnostics());
    assert.equal(found.length, 1);
    assert.equal(found[0].source, 'semaprax');
    assert.match(String(found[0].code), /^SPX-/);
    assert.equal(found[0].severity, vscode.DiagnosticSeverity.Error);
    assert.equal(found[0].range.start.line, 6, 'the error is reported on the `add(19, true)` line');
    assert.deepEqual(messages, [], 'diagnostics appear without any prompt');
    await writeAndSave(good);
    await until('the diagnostic to clear', () => diagnostics().length === 0);
    assert.equal(stateName(), 'ready');

    // 7. Cancelling later leaves the valid selection intact.
    nextPick = undefined;
    assert.equal(await setup.configure(), undefined);
    nextPick = browse; nextOpen = undefined;
    assert.equal(await setup.configure(), undefined);
    assert.equal(inspect().globalValue, compiler);
    assert.equal(stateName(), 'ready');

    // 8. A moved binary yields the repair state; restoring it recovers.
    const moved = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'semaprax-host-moved-')), 'semaprax');
    fs.copyFileSync(compiler, moved); fs.chmodSync(moved, 0o755);
    nextPick = browse; nextOpen = [vscode.Uri.file(moved)];
    assert.equal(await setup.configure(), moved);
    assert.equal(stateName(), 'ready');
    const parked = `${moved}.parked`;
    fs.renameSync(moved, parked);
    try {
      messages.length = 0;
      await writeAndSave(bad);
      await until('the unavailable state after a save', () => stateName() === 'unusable');
      assert.match(setup.item.text, /compiler unavailable/);
      assert.match(setup.item.tooltip, /no longer exists/);
      assert.equal(setup.item.command, 'semaprax.configureCompiler');
      assert.deepEqual(messages, [], 'a missing binary is surfaced by the status item, not a prompt');
      await writeAndSave(good);
      await setup.refresh();
      assert.equal(stateName(), 'unusable', 'refresh keeps reporting the missing binary');
      // Repair: select the real compiler again through the same command.
      nextPick = browse; nextOpen = [vscode.Uri.file(compiler)];
      assert.equal(await setup.configure(), compiler);
      assert.equal(stateName(), 'ready');
      assert.equal(inspect().globalValue, compiler);
    } finally {
      fs.rmSync(path.dirname(moved), { recursive: true, force: true });
    }
  } finally {
    Object.assign(window, { showQuickPick: original.pick, showOpenDialog: original.open, showInformationMessage: original.info, showErrorMessage: original.error, showWarningMessage: original.warn });
    fs.writeFileSync(app, good);
  }
  console.log('SEMAPRAX_VSCODE_COMPILER_JOURNEY=' + JSON.stringify({ vscode_version: vscode.version, status: setup.item.text, steps: 8 }));
}
module.exports = { run };
