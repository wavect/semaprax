// HN-08: language coverage on a mixed Semaprax application, update-check/telemetry containment, and
// readers during a real refresh, for every provisioned graft install.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { before, describe, test } from 'node:test';
import { loadProfiles } from '../lib/compat.mjs';
import { Adapter, GIT, GRAFT, GRAFT_NEW, makeProject, tmp, versionOf } from './helpers.mjs';

const INSTALLS = [['global', GRAFT], ['newer', GRAFT_NEW]].filter(([, b]) => b);

function mixedApp() {
  const root = tmp('mixed');
  const files = {
    'src/lib.rs': 'pub fn add(a: i32, b: i32) -> i32 { a + b }\npub fn twice(x: i32) -> i32 { add(x, x) }\n',
    'src/app.spx': 'fn main() -> i32 { 0 }\n', 'patches/p.spatch': 'patch\n',
    'web/render.ts': 'export function renderTotal(t: number): string { return String(t); }\n',
    'tools/report.py': 'def parse_total(s):\n    return int(s)\n',
    'db/schema.sql': 'create table t(a int);\n', 'scripts/run.sh': 'run(){ :; }\n', 'api/svc.proto': 'message M {}\n',
    'target/gen.rs': 'pub fn ignored() {}\n', '.gitignore': 'secret/\n', 'secret/key.py': 'def private_key(): pass\n',
  };
  for (const [p, c] of Object.entries(files)) { mkdirSync(dirname(join(root, p)), { recursive: true }); writeFileSync(join(root, p), c); }
  const g = (...a) => execFileSync(GIT, ['-C', root, '-c', 'user.email=t@t', '-c', 'user.name=t', ...a], { stdio: 'ignore' });
  g('init', '-q'); g('add', '-A'); g('commit', '-qm', 'init');
  return root;
}

for (const [label, bin] of INSTALLS) describe(`mixed Semaprax app (real graft, ${label})`, () => {
  let version;
  before(() => { Adapter.upstream = bin; version = versionOf(bin); });

  test('positive: Rust, TypeScript and Python are indexed with exact spans; the profile names the parsers', async () => {
    const root = mixedApp();
    const a = new Adapter({ root, cache: tmp('c') });
    const sk = await a.call('skeleton', { path: 'src/lib.rs' });
    assert.equal(sk.payload.coverage.complete, false, 'a mixed project is never complete');
    assert.deepEqual(sk.payload.items.map((i) => [i.span.start_line, i.span.end_line, i.language]), [[1, 1, 'rust'], [2, 2, 'rust']]);
    const rs = await a.call('search', { query: 'add', mode: 'exact' });
    assert.ok(rs.payload.items.some((i) => i.path === 'src/lib.rs' && i.span.start_line === 2));
    const ts = await a.call('search', { query: 'renderTotal', mode: 'exact' });
    assert.ok(ts.payload.items.some((i) => i.path === 'web/render.ts'));
    const py = await a.call('skeleton', { path: 'tools/report.py' });
    assert.ok(py.payload.items.some((i) => i.path === 'tools/report.py'));
    const profile = loadProfiles().profiles[version];
    for (const e of ['.rs', '.ts', '.py']) assert.ok(profile.parsed_extensions.includes(e));
    await a.close();
  });

  test('negative: .spx/.spatch and parser-less .sql/.sh/.proto are reported unsupported, never indexed; ignored paths absent', async () => {
    const root = mixedApp();
    const a = new Adapter({ root, cache: tmp('c') });
    const r = await a.call('orient', {});
    const why = Object.fromEntries(r.payload.coverage.skipped.map((s) => [s.path, s.reason]));
    assert.match(why['src/app.spx'], /^unsupported: Semaprax source/);
    assert.match(why['patches/p.spatch'], /^unsupported: Semaprax source/);
    for (const p of ['db/schema.sql', 'scripts/run.sh', 'api/svc.proto']) assert.match(why[p], /^unsupported: the installed graft parser indexes no \./, p);
    assert.equal(r.payload.coverage.complete, false);
    assert.equal(r.payload.metadata.absence_proven, false);
    const all = JSON.stringify(r.payload.items);
    assert.ok(!all.includes('secret/') && !all.includes('target/gen.rs'), 'git-ignored and target/ paths never surface');
    assert.ok(!('secret/key.py' in why) && !('target/gen.rs' in why), 'ignored files are neither indexed nor listed');
    const leak = await a.call('search', { query: 'private_key', mode: 'exact' });
    assert.deepEqual(leak.payload.items, []);
    await a.close();
  });

  test('update-check and telemetry stay contained: no npm on PATH, private HOME, opt-out forced, nothing written outside the cache', async () => {
    const root = makeProject('upd'); const cache = tmp('c-upd');
    const a = new Adapter({ root, cache, env: { DO_NOT_TRACK: '0', CI: '' } });
    const r = await a.call('orient', {});
    assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    await a.close();
    await new Promise((res) => setTimeout(res, 1500)); // a detached update-check child would have finished by now
    const work = join(cache, 'graft-context', readdirSync(join(cache, 'graft-context'))[0]);
    assert.deepEqual(readdirSync(join(work, 'bin')).sort(), ['git', 'node'], 'PATH holds only node and git (no npm)');
    const f = join(work, 'home', '.graft', 'update-check.json');
    if (existsSync(f)) assert.equal(JSON.parse(readFileSync(f, 'utf8')).latest, null, 'an update check that ran learned nothing: it had no npm and no network');
    for (const d of readdirSync(work)) assert.ok(['bin', 'home', 'tmp', 'cwd', 'gen', 'CURRENT', 'owner.json'].includes(d), `unexpected cache entry ${d}`);
  });

  test('two processes during a real refresh: the reader gets a complete old or complete new generation, never an error', async () => {
    const root = makeProject('during'); const cache = tmp('c-during');
    const w = new Adapter({ root, cache });
    assert.equal((await w.call('orient', {})).status, 'complete');
    for (const f of ['src/greet.ts', 'src/main.ts']) writeFileSync(join(root, f), readFileSync(join(root, f), 'utf8').replaceAll('greetUser', 'greetNow'));
    const r = new Adapter({ root, cache });
    const writer = w.call('search', { query: 'greetNow', mode: 'exact' });
    const seen = [];
    for (let i = 0; i < 6; i++) { seen.push(await r.call('search', { query: 'greetNow', mode: 'exact', refresh: 'never' })); }
    const done = await writer;
    assert.equal(done.status, 'complete');
    assert.equal(done.payload.items.length, 4);
    for (const x of seen) {
      if (x.status === 'complete') assert.equal(x.payload.items.length, 4, 'new generation is complete');
      else { assert.equal(x.status, 'stale', JSON.stringify(x.diagnostics)); assert.equal(x.diagnostics[0].code, 'graft.index-stale'); }
    }
    await w.close(); await r.close();
  });
});
