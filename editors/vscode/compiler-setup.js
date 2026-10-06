'use strict';
// Compiler selection and repair: the pure half. Nothing here touches VS Code.
// A binary is executed only after the user explicitly selected it, directly
// (shell:false), with a time bound and an output byte cap; candidate locations
// and PATH are listed by file-system inspection and never executed.
//
// Identity: `semaprax version --json` prints one JSON object
// {"schema":"semaprax.version.v1","version","commit","maturity","rust_min"}.
// Capabilities come from the selected binary's own `help all` catalog, whose
// usage lines read `semaprax <command> ...`; a file name decides nothing.
const path = require('node:path');
const { TextDecoder } = require('node:util');

const PROBE_TIMEOUT_MS = 10 * 1000;
const PROBE_MAX_BYTES = 256 * 1024;
const VERSION_SCHEMA = 'semaprax.version.v1';
const INSTALL_GUIDE_URL = 'https://github.com/wavect/semaprax/blob/main/handbook/getting-started/install.md';
const ADVANCED_COMMAND = 'serve-workspace-mcp';
const CONTROL = /[\u0000-\u001f\u007f]/;

// A selection is usable only as an absolute path without control characters.
function normalizeSelection(value) {
  if (typeof value !== 'string' || !value || CONTROL.test(value)) return null;
  const flavor = /^[A-Za-z]:[\\/]|^\\\\/.test(value) ? path.win32 : path;
  return flavor.isAbsolute(value) ? flavor.normalize(value) : null;
}

function strictUtf8(buffer) {
  try { return new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(buffer); } catch { return null; }
}

// One bounded direct run. Resolves { code, stdout (Buffer), timedOut, truncated, error, errorCode }.
function runBounded(spawnFn, binary, args, options = {}) {
  const maxBytes = options.maxBytes ?? PROBE_MAX_BYTES, timeoutMs = options.timeoutMs ?? PROBE_TIMEOUT_MS;
  return new Promise(resolve => {
    const out = []; let bytes = 0, settled = false, timedOut = false, truncated = false, child;
    const finish = result => { if (settled) return; settled = true; clearTimeout(timer); resolve({ stdout: Buffer.concat(out), timedOut, truncated, ...result }); };
    const kill = () => {
      try { child?.kill(); } catch {}
      const escalation = setTimeout(() => { try { child?.kill('SIGKILL'); } catch {} }, 2000);
      escalation.unref?.();
    };
    const timer = setTimeout(() => { timedOut = true; kill(); finish({ code: null }); }, timeoutMs);
    try {
      child = spawnFn(binary, args, { shell: false, windowsHide: true, cwd: options.cwd || path.dirname(binary), stdio: ['ignore', 'pipe', 'ignore'] });
    } catch (error) { return finish({ code: null, error: String(error.message || error), errorCode: error.code }); }
    child.stdout.on('data', chunk => {
      if (truncated || timedOut) return;
      bytes += chunk.length;
      if (bytes > maxBytes) { truncated = true; kill(); finish({ code: null }); return; }
      out.push(chunk);
    });
    child.on('error', error => finish({ code: null, error: String(error.message || error), errorCode: error.code }));
    child.on('close', code => finish({ code }));
  });
}

function parseIdentity(text) {
  let value;
  try { value = JSON.parse(text); } catch { return null; }
  if (!value || typeof value !== 'object' || Array.isArray(value) || value.schema !== VERSION_SCHEMA) return null;
  for (const key of ['version', 'commit', 'maturity', 'rust_min']) {
    if (typeof value[key] !== 'string' || !value[key] || CONTROL.test(value[key]) || value[key].length > 128) return null;
  }
  return { version: value.version, commit: value.commit, maturity: value.maturity, rustMin: value.rust_min };
}

// The command names the catalog (`help all`) advertises as usage lines.
function parseCapabilities(text) {
  const commands = new Set();
  for (const line of text.split('\n')) {
    const match = /^\s*semaprax(?:-full)? ([a-z][a-z0-9-]*)(?:\s|$)/.exec(line);
    if (match) commands.add(match[1]);
  }
  return commands;
}

// Outcome: { ok:true, identity, capabilities } or { ok:false, kind:'unusable'|'incompatible', reason }.
async function probeCompiler(spawnFn, binary, options = {}) {
  const seconds = Math.round((options.timeoutMs ?? PROBE_TIMEOUT_MS) / 1000);
  const ran = async (args, what) => {
    const result = await runBounded(spawnFn, binary, args, options);
    const fail = (kind, reason) => ({ fail: { ok: false, kind, reason } });
    if (result.error) return fail('unusable', /ENOENT|ENOTDIR/.test(`${result.errorCode} ${result.error}`) ? 'The executable no longer exists at the selected path' : `The executable could not be started (${result.errorCode || 'error'})`);
    if (result.timedOut) return fail('unusable', `${what} did not finish within ${seconds} seconds`);
    if (result.truncated) return fail('incompatible', `${what} output exceeded its size limit`);
    if (result.code !== 0) return fail('incompatible', `${what} exited with status ${result.code}`);
    const text = strictUtf8(result.stdout);
    return text === null ? fail('incompatible', `${what} output is not valid UTF-8`) : { text };
  };
  const version = await ran(['version', '--json'], 'The version query');
  if (version.fail) return version.fail;
  const identity = parseIdentity(version.text.trim());
  if (!identity) return { ok: false, kind: 'incompatible', reason: `The executable does not answer \`version --json\` with a ${VERSION_SCHEMA} record` };
  const help = await ran(['help', 'all'], 'The command catalog');
  if (help.fail) return help.fail;
  const commands = parseCapabilities(help.text);
  if (!commands.has('check')) return { ok: false, kind: 'incompatible', reason: 'This compiler does not advertise the `check` command used for diagnostics' };
  return { ok: true, identity, capabilities: { check: true, advancedSessions: commands.has(ADVANCED_COMMAND), hotReload: commands.has('dev') } };
}

// Known per-user install locations. Candidates only: nothing is executed.
function knownLocations(platform, env) {
  const rows = [];
  if (platform === 'win32') {
    if (env.LOCALAPPDATA) rows.push(path.win32.join(env.LOCALAPPDATA, 'Programs', 'Semaprax', 'bin', 'semaprax.exe'));
  } else {
    if (env.HOME && path.isAbsolute(env.HOME)) rows.push(path.join(env.HOME, '.semaprax', 'bin', 'semaprax'));
    if (platform === 'darwin') rows.push('/opt/homebrew/bin/semaprax');
    rows.push('/usr/local/bin/semaprax');
  }
  return rows;
}

function pathCandidates(platform, env) {
  const win = platform === 'win32', flavor = win ? path.win32 : path;
  const names = win ? ['semaprax.exe', 'semaprax.cmd', 'semaprax.bat'] : ['semaprax'];
  const rows = [];
  for (const directory of String((win ? env.Path ?? env.PATH : env.PATH) ?? '').split(win ? ';' : ':')) {
    if (!directory || !flavor.isAbsolute(directory) || CONTROL.test(directory)) continue;
    for (const name of names) rows.push(flavor.join(directory, name));
  }
  return rows;
}

// Existing candidates as { path, origin }, de-duplicated, known locations first.
function discoverCandidates({ platform = process.platform, env = process.env, isFile }) {
  const seen = new Set(), rows = [];
  const add = (candidate, origin) => { if (!seen.has(candidate) && isFile(candidate)) { seen.add(candidate); rows.push({ path: candidate, origin }); } };
  for (const candidate of knownLocations(platform, env)) add(candidate, 'known install location');
  for (const candidate of pathCandidates(platform, env)) add(candidate, 'PATH');
  return rows;
}

function sameFile(left, right, realpath) {
  if (left === right) return true;
  try { return realpath(left) === realpath(right); } catch { return false; }
}

// The status model: { state, text, detail, command }.
// input: { selected, trusted, probe, pathFirst, manifestSet, policySet, realpath }.
function describeSetup(input) {
  const { selected, trusted, probe } = input;
  const base = { command: 'semaprax.configureCompiler' };
  if (!trusted) return { ...base, state: 'untrusted', text: '$(shield) SEMAPRAX: untrusted workspace', detail: 'Trust this workspace to run diagnostics. No compiler is executed in an untrusted workspace.' };
  if (!selected) return { ...base, state: 'missing', text: '$(warning) SEMAPRAX: select compiler', detail: 'No compiler is selected. Select an installed compiler to get diagnostics on save, or open the installation guide.' };
  if (!probe) return { ...base, state: 'checking', text: '$(sync~spin) SEMAPRAX: checking compiler', detail: `Checking ${selected}` };
  if (!probe.ok && probe.kind === 'unusable') return { ...base, state: 'unusable', text: '$(error) SEMAPRAX: compiler unavailable', detail: `${probe.reason}: ${selected}. Select the compiler again to repair this.` };
  if (!probe.ok) return { ...base, state: 'incompatible', text: '$(error) SEMAPRAX: incompatible compiler', detail: `${probe.reason}: ${selected}. Select a current Semaprax compiler.` };
  const { identity, capabilities } = probe;
  const lines = [`Compiler ${identity.version} (${identity.maturity}) at ${selected}. Diagnostics on save are ready.`];
  if (input.pathFirst && !sameFile(input.pathFirst, selected, input.realpath || (value => value))) lines.push(`Note: \`semaprax\` on PATH resolves to ${input.pathFirst}, which is not the selected compiler.`);
  const missing = [];
  if (!capabilities.advancedSessions) missing.push('this compiler does not advertise `serve-workspace-mcp`');
  if (!input.manifestSet) missing.push('semaprax.manifestPath is not set');
  if (!input.policySet) missing.push('semaprax.hostPolicyPath is not set');
  lines.push(missing.length ? `Advanced saved-source sessions are not ready: ${missing.join('; ')}. Basic diagnostics do not need them.` : 'Advanced saved-source session prerequisites are present.');
  return { ...base, state: 'ready', advanced: missing.length === 0, text: `$(check) SEMAPRAX: compiler ${identity.version}`, detail: lines.join('\n') };
}

module.exports = {
  PROBE_TIMEOUT_MS, PROBE_MAX_BYTES, VERSION_SCHEMA, INSTALL_GUIDE_URL,
  normalizeSelection, runBounded, parseIdentity, parseCapabilities, probeCompiler, knownLocations, pathCandidates, discoverCandidates, describeSetup
};
