'use strict';
// The Configure Compiler flow against a fake `vscode`: unset path, selected
// file, cancel, valid binary, moved binary, bad identity, untrusted workspace,
// workspace-scope isolation and no repeated prompting. Real direct-process
// probes run against stub executables (POSIX; skipped on Windows).
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { activateCompilerSetup } = require('../compiler-ui');

const posix = process.platform !== 'win32';
const IDENTITY = '{"schema":"semaprax.version.v1","version":"9.9.9","commit":"abc","maturity":"beta","rust_min":"1.88"}';
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'spx ui Ünï '));
test.after(() => fs.rmSync(root, { recursive: true, force: true }));
let counter = 0;
function stub(versionBody) {
  const directory = path.join(root, `d${counter++}`); fs.mkdirSync(directory);
  const file = path.join(directory, 'semaprax');
  fs.writeFileSync(file, `#!/bin/sh\ncase "$1" in\n  version) ${versionBody} ;;\n  help) printf 'Usage:\\nsemaprax check x\\nsemaprax serve-workspace-mcp a b\\n' ;;\n  *) exit 3 ;;\nesac\n`, { mode: 0o755 });
  return file;
}
const good = () => stub(`printf '%s\\n' '${IDENTITY}'`);

function harness({ trusted = true, global, workspace, folder, manifest, policy, quickPick, openDialog, errorChoice, env = { PATH: '' } } = {}) {
  const values = { compilerPath: global, manifestPath: manifest, hostPolicyPath: policy };
  const log = { updates: [], quickPicks: 0, errors: [], infos: [], opened: [], dialogs: 0, spawned: [] };
  const vscode = {
    StatusBarAlignment: { Left: 1 }, ConfigurationTarget: { Global: 1 },
    Uri: { parse: value => ({ url: value }), file: value => ({ fsPath: value }) },
    env: { openExternal: async uri => { log.opened.push(uri.url); return true; } },
    workspace: {
      isTrusted: trusted,
      getConfiguration: () => ({
        inspect: key => ({ globalValue: values[key], workspaceValue: key === 'compilerPath' ? workspace : undefined, workspaceFolderValue: key === 'compilerPath' ? folder : undefined }),
        update: async (key, value, target) => { log.updates.push([key, value, target]); values[key] = value; }
      })
    },
    window: {
      createStatusBarItem: () => ({ show() {}, text: '', tooltip: '', command: '' }),
      showQuickPick: async (items, options) => { log.quickPicks++; log.items = items; return quickPick ? quickPick(items) : undefined; },
      showOpenDialog: async () => { log.dialogs++; return openDialog; },
      showErrorMessage: async (message, ...buttons) => { log.errors.push(message); return errorChoice; },
      showInformationMessage: async message => { log.infos.push(message); }
    }
  };
  const countingSpawn = (...args) => { log.spawned.push(args[0]); return spawn(...args); };
  const ui = activateCompilerSetup(vscode, { spawn: countingSpawn, fs, platform: process.platform, env });
  return { ui, log, values };
}
const browse = items => items.find(item => item.action === 'browse');

test('unset path shows an actionable state, executes nothing and never prompts', async () => {
  const { ui, log } = harness();
  const state = await ui.refresh();
  assert.equal(state.state, 'missing'); assert.match(state.text, /select compiler/); assert.equal(ui.item.command, 'semaprax.configureCompiler');
  assert.deepEqual(log.spawned, []); assert.equal(log.quickPicks, 0); assert.deepEqual(log.errors, []);
});

test('selecting a file with the picker verifies, persists globally and reports ready', { skip: !posix }, async () => {
  const file = good();
  const { ui, log } = harness({ quickPick: items => browse(items), openDialog: [{ fsPath: file }] });
  assert.equal(await ui.configure(), file);
  assert.deepEqual(log.updates, [['compilerPath', file, 1]]);
  assert.equal(ui.state().state, 'ready'); assert.match(ui.item.text, /9\.9\.9/); assert.match(ui.state().detail, /manifestPath is not set/);
  assert.equal(log.errors.length, 0);
});

test('cancel at the menu or at the file dialog changes nothing and executes nothing', async () => {
  let h = harness({ quickPick: () => undefined }); assert.equal(await h.ui.configure(), undefined);
  assert.deepEqual(h.log.updates, []); assert.deepEqual(h.log.spawned, []);
  h = harness({ quickPick: items => browse(items), openDialog: undefined }); assert.equal(await h.ui.configure(), undefined);
  assert.deepEqual(h.log.updates, []); assert.deepEqual(h.log.spawned, []);
  h = harness({ quickPick: items => browse(items), openDialog: [] }); assert.equal(await h.ui.configure(), undefined);
  assert.deepEqual(h.log.updates, []); assert.deepEqual(h.log.spawned, []);
});

test('a bad selection never replaces an existing valid one', { skip: !posix }, async () => {
  const valid = good(), bad = stub(`printf 'not json\\n'`);
  const { ui, log, values } = harness({ global: valid, quickPick: items => browse(items), openDialog: [{ fsPath: bad }] });
  assert.equal(await ui.configure(), undefined);
  assert.deepEqual(log.updates, []); assert.equal(values.compilerPath, valid); assert.match(log.errors[0], /version --json/); assert.match(log.errors[0], /was not changed/);
});

test('a relative or control-character selection is refused before any execution', async () => {
  const { ui, log } = harness({ quickPick: items => browse(items), openDialog: [{ fsPath: 'relative/semaprax' }] });
  assert.equal(await ui.configure(), undefined); assert.deepEqual(log.spawned, []); assert.deepEqual(log.updates, []); assert.equal(log.errors.length, 1);
});

test('a removed binary is reported as unavailable and repairable, without a prompt', { skip: !posix }, async () => {
  const file = good(); const { ui, log } = harness({ global: file });
  assert.equal((await ui.refresh()).state, 'ready');
  fs.rmSync(file);
  const state = await ui.refresh();
  assert.equal(state.state, 'unusable'); assert.match(state.detail, /Select the compiler again to repair/);
  assert.equal(log.quickPicks, 0); assert.deepEqual(log.errors, []); assert.deepEqual(log.updates, []);
});

test('repair after relocation selects the new location', { skip: !posix }, async () => {
  const old = good(); fs.rmSync(old); const moved = good();
  const { ui, log } = harness({ global: old, quickPick: items => browse(items), openDialog: [{ fsPath: moved }] });
  assert.equal((await ui.refresh()).state, 'unusable');
  assert.equal(await ui.configure(), moved); assert.equal(ui.state().state, 'ready'); assert.deepEqual(log.updates, [['compilerPath', moved, 1]]);
});

test('an untrusted workspace executes nothing and cannot change settings', async () => {
  const { ui, log } = harness({ trusted: false, global: '/some/semaprax' });
  assert.equal((await ui.refresh()).state, 'untrusted');
  assert.equal(await ui.configure(), undefined);
  assert.deepEqual(log.spawned, []); assert.deepEqual(log.updates, []); assert.equal(log.quickPicks, 0); assert.equal(log.infos.length, 1);
});

test('workspace and folder values cannot choose the executable', async () => {
  for (const scope of [{ workspace: '/evil/semaprax' }, { folder: '/evil/semaprax' }]) {
    const { ui, log } = harness({ global: '/user/semaprax', ...scope });
    const state = await ui.refresh();
    assert.equal(state.state, 'missing'); assert.deepEqual(log.spawned, []);
  }
});

test('candidates are listed from inspected locations and not executed until selected', { skip: !posix }, async () => {
  const file = good(); const dir = path.dirname(file);
  const { ui, log } = harness({ env: { PATH: dir, HOME: path.join(root, 'nohome') }, quickPick: items => items.find(item => item.action === 'candidate') });
  assert.equal(await ui.configure(), file);
  assert.ok(log.items.some(item => item.path === file && item.description === 'PATH'));
  assert.ok(log.spawned.every(bin => bin === file));
  assert.equal(log.items.filter(item => item.action === 'guide').length, 1);
});

test('Open installation guide opens the canonical guide and changes nothing', async () => {
  const { ui, log } = harness({ quickPick: items => items.find(item => item.action === 'guide') });
  assert.equal(await ui.configure(), undefined);
  assert.deepEqual(log.opened, ['https://github.com/wavect/semaprax/blob/main/handbook/getting-started/install.md']); assert.deepEqual(log.updates, []); assert.deepEqual(log.spawned, []);
});

test('a PATH installation that differs from the selected compiler is explained', { skip: !posix }, async () => {
  const selected = good(), other = good();
  const { ui } = harness({ global: selected, env: { PATH: path.dirname(other) } });
  const state = await ui.refresh();
  assert.equal(state.state, 'ready'); assert.match(state.detail, new RegExp(`PATH resolves to ${other.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`));
});

test('repeated refreshes never prompt', { skip: !posix }, async () => {
  const { ui, log } = harness({ global: stub(`printf 'garbage'`) });
  for (let i = 0; i < 3; i++) await ui.refresh();
  assert.equal(ui.state().state, 'incompatible'); assert.equal(log.quickPicks, 0); assert.deepEqual(log.errors, []); assert.deepEqual(log.infos, []);
});
