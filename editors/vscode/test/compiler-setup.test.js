'use strict';
// Compiler selection and repair, pure half plus real direct-process probes
// against stub executables (POSIX shell scripts; skipped on Windows).
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { spawn } = require('node:child_process');
const setup = require('../compiler-setup');

const posix = process.platform !== 'win32';
const IDENTITY = JSON.stringify({ schema: 'semaprax.version.v1', version: '9.9.9', commit: 'a'.repeat(40), maturity: 'beta', rust_min: '1.88' });
const CATALOG = 'SEMAPRAX\n\nUsage:\nsemaprax check [<file>] [--json]\nsemaprax dev <semaprax.toml> --jsonl\nsemaprax serve-workspace-mcp <manifest> <policy>\n';
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'spx compiler Ünï '));
test.after(() => fs.rmSync(root, { recursive: true, force: true }));
let counter = 0;
// A stub whose `version` and `help` answers are the given shell fragments.
function stub(versionBody, helpBody, name = 'semaprax') {
  const directory = path.join(root, `d${counter++} ß`); fs.mkdirSync(directory);
  const file = path.join(directory, name);
  fs.writeFileSync(file, `#!/bin/sh\ncase "$1" in\n  version) test "$2" = "--json" || exit 2; ${versionBody} ;;\n  help) test "$2" = "all" || exit 2; ${helpBody} ;;\n  *) exit 3 ;;\nesac\n`, { mode: 0o755 });
  return file;
}
const printf = text => `printf '%s' '${text.replace(/'/g, "'\\''")}'`;
const ok = () => stub(printf(IDENTITY + '\n'), printf(CATALOG));

test('normalizeSelection admits only absolute paths without control characters', () => {
  assert.equal(setup.normalizeSelection(''), null);
  assert.equal(setup.normalizeSelection('semaprax'), null);
  assert.equal(setup.normalizeSelection('./bin/semaprax'), null);
  assert.equal(setup.normalizeSelection('/a/b\nc'), null);
  assert.equal(setup.normalizeSelection(null), null);
  assert.equal(setup.normalizeSelection('/opt/my tools/Ünï/semaprax'), '/opt/my tools/Ünï/semaprax');
  assert.equal(setup.normalizeSelection('C:\\Users\\Jörg Müller\\semaprax.exe'), 'C:\\Users\\Jörg Müller\\semaprax.exe');
});

test('known locations and PATH are candidates by file-system inspection only', () => {
  assert.deepEqual(setup.knownLocations('linux', { HOME: '/home/u' }), ['/home/u/.semaprax/bin/semaprax', '/usr/local/bin/semaprax']);
  assert.deepEqual(setup.knownLocations('darwin', { HOME: '/Users/u' }), ['/Users/u/.semaprax/bin/semaprax', '/opt/homebrew/bin/semaprax', '/usr/local/bin/semaprax']);
  assert.deepEqual(setup.knownLocations('win32', { LOCALAPPDATA: 'C:\\Users\\u\\AppData\\Local' }), ['C:\\Users\\u\\AppData\\Local\\Programs\\Semaprax\\bin\\semaprax.exe']);
  assert.deepEqual(setup.knownLocations('linux', {}), ['/usr/local/bin/semaprax']);
  const present = new Set(['/home/u/.semaprax/bin/semaprax', '/x/semaprax', '/usr/local/bin/semaprax']);
  const rows = setup.discoverCandidates({ platform: 'linux', env: { HOME: '/home/u', PATH: 'relative:/x:/usr/local/bin:/y' }, isFile: candidate => present.has(candidate) });
  assert.deepEqual(rows, [
    { path: '/home/u/.semaprax/bin/semaprax', origin: 'known install location' },
    { path: '/usr/local/bin/semaprax', origin: 'known install location' },
    { path: '/x/semaprax', origin: 'PATH' }]);
});

test('capabilities come from the catalog lines, not from file names', () => {
  assert.deepEqual([...setup.parseCapabilities(CATALOG)].sort(), ['check', 'dev', 'serve-workspace-mcp']);
  assert.deepEqual([...setup.parseCapabilities('  semaprax-full build x\nnot semaprax check\n')], ['build']);
});

test('identity record is strict', () => {
  assert.equal(setup.parseIdentity(IDENTITY).version, '9.9.9');
  for (const bad of ['', 'nope', '[]', '{"schema":"other","version":"1","commit":"c","maturity":"m","rust_min":"1"}', '{"schema":"semaprax.version.v1","version":1,"commit":"c","maturity":"m","rust_min":"1"}', '{"schema":"semaprax.version.v1","version":"1\\n2","commit":"c","maturity":"m","rust_min":"1"}']) assert.equal(setup.parseIdentity(bad), null, bad);
});

test('probe of a valid binary in a path with spaces and non-ASCII yields identity and capabilities', { skip: !posix }, async () => {
  const file = ok(); assert.match(file, / /); assert.match(file, /Ünï/);
  const result = await setup.probeCompiler(spawn, file);
  assert.equal(result.ok, true); assert.equal(result.identity.version, '9.9.9'); assert.deepEqual(result.capabilities, { check: true, advancedSessions: true, hotReload: true });
});

test('capability messages follow the binary, not its file name', { skip: !posix }, async () => {
  const lacking = stub(printf(IDENTITY + '\n'), printf('Usage:\nsemaprax check x\n'), 'semaprax-full');
  const result = await setup.probeCompiler(spawn, lacking);
  assert.equal(result.ok, true); assert.equal(result.capabilities.advancedSessions, false);
  assert.match(setup.describeSetup({ selected: lacking, trusted: true, probe: result, manifestSet: true, policySet: true }).detail, /does not advertise `serve-workspace-mcp`/);
  const noCheck = stub(printf(IDENTITY + '\n'), printf('Usage:\nsemaprax build x\n'));
  assert.equal((await setup.probeCompiler(spawn, noCheck)).kind, 'incompatible');
});

test('missing or moved binary is unusable', { skip: !posix }, async () => {
  const file = ok(); fs.rmSync(file);
  const result = await setup.probeCompiler(spawn, file);
  assert.equal(result.ok, false); assert.equal(result.kind, 'unusable'); assert.match(result.reason, /no longer exists/);
});

test('a non-executable file is unusable', { skip: !posix || process.getuid?.() === 0 }, async () => {
  const file = ok(); fs.chmodSync(file, 0o644);
  assert.equal((await setup.probeCompiler(spawn, file)).kind, 'unusable');
});

test('bad identity output is incompatible', { skip: !posix }, async () => {
  for (const body of [printf('hello\n'), printf('{"schema":"x"}\n'), `printf '\\377\\n'`, 'exit 5']) {
    const result = await setup.probeCompiler(spawn, stub(body, 'true'));
    assert.equal(result.ok, false, body); assert.equal(result.kind, 'incompatible', body);
  }
});

test('oversized output is cut off and rejected', { skip: !posix }, async () => {
  const file = stub('yes aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | head -c 5000000', 'true');
  const result = await setup.probeCompiler(spawn, file, { maxBytes: 4096 });
  assert.equal(result.kind, 'incompatible'); assert.match(result.reason, /size limit/);
});

test('a hung binary is killed at the time bound', { skip: !posix }, async () => {
  const file = stub('exec sleep 30', 'true');
  const started = Date.now();
  const result = await setup.probeCompiler(spawn, file, { timeoutMs: 300 });
  assert.equal(result.kind, 'unusable'); assert.match(result.reason, /did not finish/); assert.ok(Date.now() - started < 5000);
});

test('probe passes arguments directly with shell:false', async () => {
  let seen;
  const fake = (bin, args, options) => {
    seen = { bin, args, options };
    const child = new EventEmitter(); child.stdout = new EventEmitter(); child.kill = () => {};
    setImmediate(() => child.emit('error', Object.assign(new Error('spawn ENOENT'), { code: 'ENOENT' })));
    return child;
  };
  const result = await setup.probeCompiler(fake, '/x y/semaprax; rm -rf /');
  assert.equal(seen.options.shell, false); assert.deepEqual(seen.args, ['version', '--json']); assert.equal(seen.bin, '/x y/semaprax; rm -rf /'); assert.equal(result.kind, 'unusable');
});

test('status model distinguishes missing, untrusted, unusable, incompatible and ready with advanced prerequisites', () => {
  const good = { ok: true, identity: { version: '1.2.3', maturity: 'beta' }, capabilities: { check: true, advancedSessions: true } };
  assert.equal(setup.describeSetup({ trusted: true }).state, 'missing');
  assert.equal(setup.describeSetup({ selected: '/a', trusted: false }).state, 'untrusted');
  assert.equal(setup.describeSetup({ selected: '/a', trusted: true }).state, 'checking');
  assert.equal(setup.describeSetup({ selected: '/a', trusted: true, probe: { ok: false, kind: 'unusable', reason: 'gone' } }).state, 'unusable');
  assert.equal(setup.describeSetup({ selected: '/a', trusted: true, probe: { ok: false, kind: 'incompatible', reason: 'odd' } }).state, 'incompatible');
  const basic = setup.describeSetup({ selected: '/a', trusted: true, probe: good, manifestSet: false, policySet: false });
  assert.equal(basic.state, 'ready'); assert.equal(basic.advanced, false); assert.match(basic.detail, /manifestPath is not set/); assert.match(basic.detail, /Basic diagnostics do not need them/);
  assert.equal(setup.describeSetup({ selected: '/a', trusted: true, probe: good, manifestSet: true, policySet: true }).advanced, true);
});

test('a PATH entry that is a different installation is explained', () => {
  const good = { ok: true, identity: { version: '1', maturity: 'beta' }, capabilities: { check: true, advancedSessions: true } };
  const differ = setup.describeSetup({ selected: '/a/semaprax', trusted: true, probe: good, pathFirst: '/b/semaprax', realpath: value => value });
  assert.match(differ.detail, /PATH resolves to \/b\/semaprax, which is not the selected compiler/);
  assert.doesNotMatch(setup.describeSetup({ selected: '/a/semaprax', trusted: true, probe: good, pathFirst: '/link/semaprax', realpath: () => '/same' }).detail, /PATH resolves/);
});

 test('CLI null commit identity reaches capability discovery with honest provenance', { skip: !posix }, async () => {
  const value = { schema: 'semaprax.version.v1', version: '0.9.0', commit: null, maturity: 'beta', rust_min: '1.88' };
  const file = stub(printf(JSON.stringify(value)), printf(CATALOG));
  const result = await setup.probeCompiler(spawn, file);
  assert.equal(result.ok, true);
  assert.equal(result.identity.commit, null);
  assert.equal(result.capabilities.check, true);
  assert.match(setup.describeSetup({ selected: file, trusted: true, probe: result }).detail, /provenance is unknown/);
  for (const commit of [undefined, '', 'abc', 'A'.repeat(40), 42, {}, 'a'.repeat(41)]) {
    assert.equal(setup.parseIdentity(JSON.stringify({ ...value, commit })), null);
  }
  assert.equal((await setup.probeCompiler(spawn, stub(printf(JSON.stringify(value)), printf('semaprax build x')))).ok, false);
});
