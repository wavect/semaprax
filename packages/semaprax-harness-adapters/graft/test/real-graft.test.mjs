// Real-graft tests: spawn the adapter over a mixed-language fixture project using the installed graft.
import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { after, before, describe, test } from 'node:test';
import { Adapter, GRAFT, GRAFT_NEW, makeProject, sha, snapshot, tmp, versionOf } from './helpers.mjs';

// Every upstream install under test: the user's global graft and, when provisioned, the newer qualified one.
const INSTALLS = [['global', GRAFT], ['newer', GRAFT_NEW]].filter(([, b]) => b);

// sha256 of lines a..b (1-based, inclusive) joined by LF without a trailing terminator (broker convention).
const lines = (root, rel, a, b) => 'sha256:' + sha(Buffer.from(readFileSync(join(root, rel), 'utf8').split('\n').slice(a - 1, b).join('\n')));

for (const [label, bin] of INSTALLS) describe(`graft context adapter (real graft, ${label})`, () => {
  let root; let cache; let ad; let VERSION;
  before(() => {
    Adapter.upstream = bin; VERSION = versionOf(bin);
    root = makeProject('real'); cache = tmp('cache');
    ad = new Adapter({ root, cache });
  });
  after(async () => { await ad.close(); });

  test('initialize accepts exactly the implemented operations; unknown kinds are not accepted', async () => {
    const r = await ad.init([{ kind: 'context.repository', version: 1 }, { kind: 'command.view', version: 1 }, { kind: 'context.repository', version: 2 }]);
    assert.deepEqual(r.accepted, [{ kind: 'context.repository', version: 1, operations: ['orient', 'search', 'skeleton', 'references'] }]);
  });

  test('orient: lazy first build, structural provenance, relative paths, .spx reported as skipped', async () => {
    const r = await ad.call('orient', {});
    assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    assert.equal(r.provenance.upstream_version, VERSION);
    const p = r.payload;
    assert.equal(p.metadata.refresh.action, 'build');
    assert.equal(p.metadata.refresh.outcome, 'rebuilt');
    assert.ok(p.metadata.refresh.index_ms >= 0 && p.metadata.refresh.verification_ms === 0, 'cold build reports construction apart from verification');
    assert.equal(p.metadata.refresh.files_indexed, 3, 'git-ignored gen/ must not be indexed');
    assert.ok(p.items.length > 0);
    for (const it of p.items) {
      assert.equal(it.provenance, 'structural');
      assert.ok(!it.path.startsWith('/') && !it.path.includes('..'));
      assert.match(it.digest, /^sha256:[0-9a-f]{64}$/);
    }
    assert.ok(p.items.some((i) => i.path === 'src/greet.ts'));
    assert.equal(p.coverage.indexed_files, 3);
    assert.equal(p.coverage.complete, false);
    assert.deepEqual(p.coverage.skipped.map((s) => s.path), ['app.spx'], 'git-ignored files are neither indexed nor reported');
    const spx = p.coverage.skipped.find((s) => s.path === 'app.spx');
    assert.match(spx.reason, /^unsupported: Semaprax source/);
    assert.equal(p.metadata.absence_proven, false);
    assert.ok(p.items.every((i) => i.language !== 'semaprax'));
  });

  test('search (ranked) returns exact spans with an independently computed digest, rank preserved', async () => {
    const r = await ad.call('search', { query: 'greetUser' });
    assert.equal(r.status, "complete", JSON.stringify(r.diagnostics));
    assert.equal(r.payload.metadata.refresh.action, 'reuse');
    const top = r.payload.items[0];
    assert.equal(top.path, 'src/greet.ts');
    assert.deepEqual(top.span, { start_line: 1, end_line: 3 });
    assert.equal(top.digest, lines(root, 'src/greet.ts', 1, 3));
    assert.deepEqual(r.payload.items.map((i) => i.rank), r.payload.items.map((_, i) => i + 1));
    assert.equal(r.payload.coverage.exhaustive, false, 'ranked retrieval is top-N, never exhaustive');
  });

  test('search (exact) is exhaustive over indexed files, lexical provenance, honours .gitignore', async () => {
    const r = await ad.call('search', { query: 'greetUser', mode: 'exact' });
    const keys = r.payload.items.map((i) => `${i.path}:${i.span.start_line}`).sort();
    assert.deepEqual(keys, ['src/greet.ts:1', 'src/greet.ts:5', 'src/main.ts:1', 'src/main.ts:2']);
    assert.ok(r.payload.items.every((i) => i.provenance === 'inferred'));
    assert.equal(r.payload.coverage.exhaustive, true);
    assert.equal(r.payload.coverage.complete, false, '.spx is skipped');
    assert.equal(r.payload.metadata.absence_proven, false);
    const hit = r.payload.items.find((i) => i.path === 'src/main.ts' && i.span.start_line === 2);
    assert.equal(hit.digest, lines(root, 'src/main.ts', 2, 2));
  });

  test('search: truncated exact result is partial, incomplete and non-exhaustive', async () => {
    const r = await ad.call('search', { query: 'greetUser', mode: 'exact', limit: 1 });
    assert.equal(r.status, 'partial');
    assert.equal(r.payload.items.length, 1);
    assert.equal(r.payload.coverage.exhaustive, false);
    assert.equal(r.payload.coverage.complete, false);
    assert.ok(r.diagnostics.some((d) => d.code === 'graft.truncated'));
  });

  test('result budget shrinks items and marks the result partial', async () => {
    const r = await ad.call('search', { query: 'greetUser', mode: 'exact' }, { budget: { max_result_bytes: 2300, remaining_calls: 1 } });
    assert.equal(r.status, 'partial');
    assert.ok(Buffer.byteLength(JSON.stringify(r.payload)) < 2300);
    assert.equal(r.payload.coverage.exhaustive, false);
  });

  test('skeleton lists signatures with spans', async () => {
    const r = await ad.call('skeleton', { path: 'src/greet.ts' });
    assert.equal(r.status, "complete", JSON.stringify(r.diagnostics));
    assert.deepEqual(r.payload.items.map((i) => [i.span.start_line, i.span.end_line, i.text]), [
      [1, 3, 'function greetUser(name: string): string'], [4, 6, 'class Greeter'], [5, 5, 'hi(n: string)'],
    ]);
    assert.equal(r.payload.items[0].digest, lines(root, 'src/greet.ts', 1, 3));
  });

  test('skeleton of .spx is unsupported, names the file, and never reaches graft', async () => {
    const fresh = new Adapter({ root, cache: tmp('cache-spx') });
    const r = await fresh.call('skeleton', { path: 'app.spx' });
    assert.equal(r.status, 'unsupported');
    assert.deepEqual(r.payload.coverage.skipped.map((s) => s.path), ['app.spx']);
    assert.equal(r.payload.coverage.complete, false);
    assert.equal(r.provenance.upstream_version, 'unknown', 'upstream was never probed');
    await fresh.close();
  });

  test('skeleton refuses traversal and absolute paths', async () => {
    for (const p of ['../outside.ts', '/etc/passwd', 'src/../../x.ts']) {
      const r = await ad.call('skeleton', { path: p });
      assert.equal(r.status, 'refused', p);
    }
  });

  test('references: call edges are structural and never exhaustive; absence is not claimed', async () => {
    const r = await ad.call('references', { symbol: 'greetUser' });
    assert.equal(r.status, "complete", JSON.stringify(r.diagnostics));
    assert.deepEqual(r.payload.items.map((i) => i.path).sort(), ['src/greet.ts', 'src/main.ts']);
    assert.ok(r.payload.items.every((i) => i.provenance === 'structural'));
    assert.equal(r.payload.coverage.exhaustive, false);
    const none = await ad.call('references', { symbol: 'neverDefined' });
    assert.equal(none.status, 'complete');
    assert.deepEqual(none.payload.items, []);
    assert.equal(none.payload.coverage.exhaustive, false);
    assert.equal(none.payload.metadata.absence_proven, false);
    assert.ok(none.diagnostics.some((d) => d.code === 'graft.symbol-not-found'));
  });

  test('references with exhaustive=true adds a complete textual scan and sets exhaustive', async () => {
    const r = await ad.call('references', { symbol: 'greetUser', exhaustive: true });
    assert.equal(r.payload.coverage.exhaustive, true);
    const keys = new Set(r.payload.items.map((i) => `${i.path}:${i.span.start_line}`));
    for (const k of ['src/greet.ts:5', 'src/main.ts:1', 'src/main.ts:2']) assert.ok(keys.has(k), k);
    assert.equal(keys.size, r.payload.items.length, 'no duplicate sites');
    assert.equal(r.payload.metadata.absence_proven, false, '.spx skipped keeps the result incomplete');
  });

  test('references rejects option-like symbols', async () => {
    const r = await ad.call('references', { symbol: '--deep' });
    assert.equal(r.status, 'refused');
  });

  test('stale index after an edit is refreshed and cost is reported; renamed symbol follows the rename', async () => {
    for (const f of ['src/greet.ts', 'src/main.ts']) writeFileSync(join(root, f), readFileSync(join(root, f), 'utf8').replaceAll('greetUser', 'greetPerson'));
    const fresh = await ad.call('search', { query: 'greetPerson', mode: 'exact' });
    assert.equal(fresh.payload.metadata.refresh.action, 'refresh');
    assert.ok(fresh.payload.metadata.refresh.files_changed >= 2);
    assert.equal(typeof fresh.payload.metadata.refresh.ms, 'number');
    assert.equal(fresh.payload.items.length, 4);
    const old = await ad.call('search', { query: 'greetUser', mode: 'exact' });
    assert.deepEqual(old.payload.items, []);
    assert.equal(old.payload.metadata.absence_proven, false, 'absence is never proven while .spx is uncovered');
    const refs = await ad.call('references', { symbol: 'greetUser' });
    assert.deepEqual(refs.payload.items, []);
    assert.ok(refs.diagnostics.some((d) => d.code === 'graft.symbol-not-found'));
    const callers = await ad.call('references', { symbol: 'greetPerson' });
    assert.deepEqual(callers.payload.items.map((i) => i.path).sort(), ['src/greet.ts', 'src/main.ts']);
  });

  test('same-size edit with preserved mtime is still detected (content digest, not stat)', async () => {
    const f = join(root, 'util.py');
    const st = statSync(f);
    writeFileSync(f, readFileSync(f, 'utf8').replace('parse_amount', 'parse_amoumt'));
    utimesSync(f, st.atime, st.mtime);
    const r = await ad.call('search', { query: 'parse_amoumt', mode: 'exact' });
    assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    assert.ok(r.payload.items.length >= 1, 'edit visible');
    assert.equal(r.payload.metadata.refresh.action, 'refresh');
    const l = r.payload.items[0].span.start_line;
    assert.equal(r.payload.items[0].digest, lines(root, 'util.py', l, l));
  });

  test('the user project is never written to (including a pre-existing user graft/ index)', async () => {
    const r2 = makeProject('userindex');
    mkdirSync(join(r2, 'graft'), { recursive: true });
    writeFileSync(join(r2, 'graft', 'INDEX.md'), 'user-owned index\n');
    const snap = snapshot(r2);
    const a = new Adapter({ root: r2, cache: tmp('cache-ui') });
    const r = await a.call('orient', {});
    assert.equal(r.status, "complete", JSON.stringify(r.diagnostics));
    assert.deepEqual(snapshot(r2), snap);
    assert.equal(readFileSync(join(r2, 'graft', 'INDEX.md'), 'utf8'), 'user-owned index\n');
    await a.close();
  });

  test('worktree switch: a second copy of the project gets its own index and its own answers', async () => {
    const wt = join(tmp('wt'), 'worktree');
    cpSync(root, wt, { recursive: true });
    writeFileSync(join(wt, 'src/greet.ts'), readFileSync(join(wt, 'src/greet.ts'), 'utf8').replaceAll('greetPerson', 'greetWorktree'));
    const shared = tmp('cache-wt');
    const a = new Adapter({ root, cache: shared });
    const b = new Adapter({ root: wt, cache: shared });
    const ra = await a.call('search', { query: 'greetWorktree', mode: 'exact' });
    const rb = await b.call('search', { query: 'greetWorktree', mode: 'exact' });
    assert.deepEqual(ra.payload.items, []);
    assert.ok(rb.payload.items.length >= 1);
    assert.equal(rb.payload.metadata.refresh.action, 'build');
    assert.notEqual(ra.payload.metadata.index_digest, rb.payload.metadata.index_digest);
    assert.equal(readdirSync(join(shared, 'graft-context')).length, 2, 'one private index per project root');
    await a.close(); await b.close();
  });

  test('an index in the cache that the adapter does not own is never overwritten', async () => {
    const r2 = makeProject('notowned');
    const c = tmp('cache-no');
    const first = new Adapter({ root: r2, cache: c });
    await first.call('orient', {});
    await first.close();
    const work = join(c, 'graft-context', readdirSync(join(c, 'graft-context'))[0]);
    rmSync(join(work, 'owner.json'));
    const live = join(work, 'gen', readFileSync(join(work, 'CURRENT'), 'utf8').trim());
    writeFileSync(join(live, 'USER-FILE'), 'precious');
    const second = new Adapter({ root: r2, cache: c });
    const r = await second.call('orient', {});
    assert.equal(r.status, 'refused');
    assert.equal(r.diagnostics[0].code, 'graft.index-not-owned');
    assert.equal(readFileSync(join(live, 'USER-FILE'), 'utf8'), 'precious');
    await second.close();
  });

  test('missing host configuration is unavailable, not a crash', async () => {
    const a = new Adapter({ root: '', cache: '', upstream: '' });
    const r = await a.call('orient', {});
    assert.equal(r.status, 'unavailable');
    assert.equal(r.diagnostics[0].code, 'graft.config-missing');
    await a.close();
  });

  test('cache root inside the project is refused', async () => {
    const r2 = makeProject('cacheinside');
    const a = new Adapter({ root: r2, cache: join(r2, '.cache') });
    const r = await a.call('orient', {});
    assert.equal(r.status, 'refused');
    assert.equal(r.diagnostics[0].code, 'graft.cache-inside-project');
    assert.ok(!existsSync(join(r2, '.cache', 'graft-context')));
    await a.close();
  });
});
