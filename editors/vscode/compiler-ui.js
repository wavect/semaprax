'use strict';
// The visible half of compiler onboarding: the status item and the
// `SEMAPRAX: Configure Compiler` command. `vscode`, `spawn` and the file system
// are arguments so the whole flow runs under `node --test` with fakes.
//
// Trust model: only the user (global) `semaprax.compilerPath` is read or
// written; a workspace or folder value is ignored. Nothing is executed before
// the user selected it, nothing is persisted unless its identity probe passed,
// and cancelling leaves every setting untouched. No prompt is shown on save:
// a missing or broken setup only changes the status item.
const path = require('node:path');
const setup = require('./compiler-setup');

function activateCompilerSetup(vscode, { spawn, fs, platform = process.platform, env = process.env, probeOptions = {} }) {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  let probe, probing = 0, last;
  const config = () => vscode.workspace.getConfiguration('semaprax');
  const machineValue = key => {
    const inspected = config().inspect(key);
    if (!inspected || inspected.workspaceValue !== undefined || inspected.workspaceFolderValue !== undefined) return undefined;
    return typeof inspected.globalValue === 'string' && inspected.globalValue ? inspected.globalValue : undefined;
  };
  const selected = () => setup.normalizeSelection(machineValue('compilerPath')) || undefined;
  const isFile = candidate => { try { return fs.statSync(candidate).isFile(); } catch { return false; } };
  const realpath = candidate => fs.realpathSync(candidate);
  const pathFirst = () => setup.discoverCandidates({ platform, env, isFile }).find(row => row.origin === 'PATH')?.path;

  function render() {
    const binary = selected(), trusted = vscode.workspace.isTrusted;
    last = setup.describeSetup({
      selected: binary, trusted, probe: binary && trusted && probe?.binary === binary ? probe.value : undefined,
      pathFirst: pathFirst(), realpath, manifestSet: Boolean(machineValue('manifestPath')), policySet: Boolean(machineValue('hostPolicyPath'))
    });
    item.text = last.text; item.tooltip = last.detail; item.command = last.command; item.show();
    return last;
  }

  // Re-probes the saved user selection. Never runs in an untrusted workspace
  // and never when nothing is selected. A newer refresh supersedes an older one.
  async function refresh() {
    const binary = selected();
    if (!binary || !vscode.workspace.isTrusted) { probe = undefined; return render(); }
    const ticket = ++probing; probe = { binary }; render();
    const value = await setup.probeCompiler(spawn, binary, probeOptions);
    if (ticket !== probing) return last;
    probe = { binary, value };
    return render();
  }

  async function openGuide() { await vscode.env.openExternal(vscode.Uri.parse(setup.INSTALL_GUIDE_URL)); }

  // Returns the persisted absolute path, or undefined when nothing changed.
  async function configure() {
    if (!vscode.workspace.isTrusted) { void vscode.window.showInformationMessage('Trust this workspace before selecting a SEMAPRAX compiler. Nothing was executed or changed.'); return undefined; }
    const current = selected();
    const candidates = setup.discoverCandidates({ platform, env, isFile }).filter(row => row.path !== current);
    const items = [
      { label: '$(folder-opened) Select installed compiler...', action: 'browse', detail: 'Choose the semaprax executable with a file picker' },
      ...candidates.map(row => ({ label: row.path, description: row.origin, detail: 'Found on this machine, not run until you select it', action: 'candidate', path: row.path })),
      ...(current ? [{ label: '$(refresh) Re-check current compiler', detail: current, action: 'recheck' }] : []),
      { label: '$(book) Open installation guide', action: 'guide' }
    ];
    const choice = await vscode.window.showQuickPick(items, { title: 'SEMAPRAX: Configure Compiler', placeHolder: (last || render()).detail.split('\n')[0], ignoreFocusOut: false });
    if (!choice) return undefined;
    if (choice.action === 'guide') { await openGuide(); return undefined; }
    if (choice.action === 'recheck') { await refresh(); return undefined; }
    let target = choice.path;
    if (choice.action === 'browse') {
      const start = current ? vscode.Uri.file(path.dirname(current)) : undefined;
      const picked = await vscode.window.showOpenDialog({ canSelectFiles: true, canSelectFolders: false, canSelectMany: false, defaultUri: start, openLabel: 'Select compiler', title: 'Select the semaprax executable' });
      if (!picked || !picked.length) return undefined;
      target = picked[0].fsPath;
    }
    const absolute = setup.normalizeSelection(target);
    if (!absolute) { void vscode.window.showErrorMessage('The selected compiler path must be absolute and contain no control characters. No setting was changed.'); return undefined; }
    const result = await setup.probeCompiler(spawn, absolute, probeOptions);
    if (!result.ok) {
      // Keep any existing valid selection; offer the next step without a modal.
      const next = await vscode.window.showErrorMessage(`${result.reason}: ${absolute}. The compiler setting was not changed.`, 'Select Another Compiler', 'Open Installation Guide');
      if (next === 'Select Another Compiler') return configure();
      if (next === 'Open Installation Guide') await openGuide();
      return undefined;
    }
    await config().update('compilerPath', absolute, vscode.ConfigurationTarget.Global);
    probe = { binary: absolute, value: result }; probing++;
    render();
    void vscode.window.showInformationMessage(`SEMAPRAX compiler ${result.identity.version} selected.${last.advanced ? '' : ' Diagnostics are ready; saved-source sessions need extra setup.'}`);
    return absolute;
  }

  render();
  return { item, configure, refresh, render, state: () => last };
}

module.exports = { activateCompilerSetup };
