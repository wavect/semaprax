// HN-10: opt-in adoption of a user-owned graft index. Real graft, every provisioned install.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { chmodSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { before, describe, test } from 'node:test';
import { treeDigest } from '../lib/adopt.mjs';
import { ensureFresh, loadConfig, probeIdentity } from '../lib/project.mjs';
import { ADOPT, Adapter, GIT, GRAFT, GRAFT_NEW, buildUserIndex, makeProject, tmp, treeHash, versionOf } from './helpers.mjs';

const INSTALLS = [['global', GRAFT], ['newer', GRAFT_NEW]].filter(([, b]) => b);
const refresh = (r) => r.payload.metadata.refresh;
const ownedDir = (cache) => join(cache, 'graft-context');

for (const [label, bin] of INSTALLS) describe(`index adoption (real graft, ${label})`, () => {
  before(() => { Adapter.upstream = bin; });

  test('off by default: a present user graft/ index is ignored and never read', async () => {
    const root = makeProject('ad-off'); buildUserIndex(bin, root);
    const before = treeHash(join(root, 'graft'));
    const a = new Adapter({ root, cache: tmp('c') });
    const r = await a.call('orient', {});
    assert.equal(refresh(r).outcome, 'rebuilt');
    assert.equal(r.payload.metadata.index_adoption, undefined);
    assert.equal(treeHash(join(root, 'graft')), before);
    await a.close();
  });

  test('read-only reuse: no rebuild, no owned index, user index byte-identical after queries', async () => {
    const root = makeProject('ad-ro'); buildUserIndex(bin, root);
    const before = treeHash(join(root, 'graft'));
    const cache = tmp('c');
    const a = new Adapter({ root, cache, env: ADOPT('read-only') });
    for (const [op, payload] of [['orient', {}], ['search', { query: 'greetUser' }], ['search', { query: 'greetUser', mode: 'exact' }], ['skeleton', { path: 'src/greet.ts' }], ['references', { symbol: 'greetUser' }]]) {
      const r = await a.call(op, payload);
      assert.equal(r.status, 'complete', `${op}: ${JSON.stringify(r.diagnostics)}`);
      assert.equal(refresh(r).outcome, 'reused-user-index');
      assert.equal(refresh(r).index_ms, 0, 'no index construction');
      assert.equal(refresh(r).copied_bytes, 0);
      assert.ok(refresh(r).files_verified >= 3, 'verification work is reported separately');
      assert.equal(r.payload.metadata.index_adoption.ownership, 'read-only');
      assert.equal(r.payload.metadata.index_adoption.schema, 'semaprax.harness-index-adoption.v1');
      assert.match(r.payload.metadata.index_adoption.inputs_digest, /^[0-9a-f]{64}$/);
    }
    assert.equal(treeHash(join(root, 'graft')), before, 'user index unchanged');
    assert.ok(!existsSync(join(ownedDir(cache), readdirSync(ownedDir(cache))[0] ?? 'x', 'gen')), 'no owned generation was built');
    await a.close();
  });

  test('copied snapshot: immutable validated copy, second call copies nothing, user index untouched', async () => {
    const root = makeProject('ad-cp'); buildUserIndex(bin, root);
    const before = treeHash(join(root, 'graft'));
    const cache = tmp('c');
    const a = new Adapter({ root, cache, env: ADOPT('copied-snapshot') });
    const one = await a.call('search', { query: 'greetUser' });
    assert.equal(refresh(one).outcome, 'copied-validated-index');
    assert.ok(refresh(one).copied_bytes > 0);
    const two = await a.call('references', { symbol: 'greetUser' });
    assert.equal(refresh(two).outcome, 'copied-validated-index');
    assert.equal(refresh(two).copied_bytes, 0, 'snapshot reused');
    const work = join(ownedDir(cache), readdirSync(ownedDir(cache))[0]);
    const snaps = readdirSync(join(work, 'adopted'));
    assert.equal(snaps.length, 1);
    assert.equal(statSync(join(work, 'adopted', snaps[0], '.graph', 'wiring.json')).mode & 0o222, 0, 'snapshot files are read-only');
    assert.equal(treeHash(join(root, 'graft')), before);
    await a.close();
  });

  test('stale same-size edit with preserved mtime cannot serve as current evidence; falls back to an owned rebuild', async () => {
    const root = makeProject('ad-stale'); buildUserIndex(bin, root);
    const before = treeHash(join(root, 'graft'));
    const f = join(root, 'util.py'); const st = statSync(f);
    writeFileSync(f, readFileSync(f, 'utf8').replace('parse_amount', 'parse_amoumt'));
    utimesSync(f, st.atime, st.mtime);
    const a = new Adapter({ root, cache: tmp('c'), env: ADOPT('read-only') });
    const r = await a.call('search', { query: 'parse_amoumt', mode: 'exact' });
    assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    assert.equal(refresh(r).outcome, 'incompatible');
    assert.equal(refresh(r).served_by, 'rebuilt');
    assert.match(r.payload.metadata.index_adoption.reasons, /util\.py differs from its indexed content/);
    assert.ok(r.payload.items.length >= 1, 'answer reflects the edit');
    assert.ok(r.diagnostics.some((d) => d.code === 'graft.index-incompatible'));
    assert.equal(treeHash(join(root, 'graft')), before, 'user index untouched on failure');
    await a.close();
  });

  test('an index from another worktree (different content) is refused', async () => {
    const a0 = makeProject('ad-wt-a'); buildUserIndex(bin, a0);
    const wt = join(tmp('wt'), 'worktree');
    cpSync(a0, wt, { recursive: true });
    writeFileSync(join(wt, 'src/greet.ts'), readFileSync(join(wt, 'src/greet.ts'), 'utf8').replace('hello', 'howdy'));
    const before = treeHash(join(wt, 'graft'));
    const a = new Adapter({ root: wt, cache: tmp('c'), env: ADOPT('copied-snapshot') });
    const r = await a.call('orient', {});
    assert.equal(refresh(r).outcome, 'incompatible');
    assert.match(r.payload.metadata.index_adoption.reasons, /src\/greet\.ts differs/);
    assert.equal(treeHash(join(wt, 'graft')), before);
    await a.close();
  });

  test('an index that contains a private/ignored path is refused', async () => {
    const root = makeProject('ad-priv');
    const noIgnore = join(tmp('copy'), 'p');
    cpSync(root, noIgnore, { recursive: true });
    rmSync(join(noIgnore, '.gitignore'));
    buildUserIndex(bin, noIgnore, join(root, 'graft')); // indexes gen/ignored.py, which root git-ignores
    assert.match(readFileSync(join(root, 'graft', '.graph', 'wiring.json'), 'utf8'), /gen\/ignored\.py/);
    const a = new Adapter({ root, cache: tmp('c'), env: ADOPT('read-only') });
    const r = await a.call('orient', {});
    assert.equal(refresh(r).outcome, 'incompatible');
    assert.match(r.payload.metadata.index_adoption.reasons, /gen\/ignored\.py is excluded, ignored or absent/);
    assert.ok(!r.payload.items.some((i) => i.path.startsWith('gen/')), 'private path never served');
    await a.close();
  });

  test('a richer (deep) index with summaries is refused by the code-only policy', async () => {
    const root = makeProject('ad-deep'); buildUserIndex(bin, root);
    const wf = join(root, 'graft', '.graph', 'wiring.json');
    const w = JSON.parse(readFileSync(wf, 'utf8'));
    w.nodes.find((n) => n.kind === 'function').summary = 'LLM written text';
    writeFileSync(wf, JSON.stringify(w));
    const a = new Adapter({ root, cache: tmp('c'), env: ADOPT('read-only') });
    const r = await a.call('orient', {});
    assert.equal(refresh(r).outcome, 'incompatible');
    assert.match(r.payload.metadata.index_adoption.reasons, /--deep/);
    await a.close();
  });

  test('a missing source file, a symlinked index and a bad setting are refused without touching anything', async () => {
    const root = makeProject('ad-misc'); buildUserIndex(bin, root);
    const a = new Adapter({ root, cache: tmp('c'), env: ADOPT('write-through') });
    const bad = await a.call('orient', {});
    assert.equal(refresh(bad).outcome, 'incompatible');
    assert.match(bad.payload.metadata.index_adoption.reasons, /invalid adoption setting/);
    await a.close();
    const root2 = makeProject('ad-link'); const real = buildUserIndex(bin, root2, join(tmp('elsewhere'), 'idx'));
    symlinkSync(real, join(root2, 'graft'));
    const b = new Adapter({ root: root2, cache: tmp('c'), env: ADOPT('read-only') });
    const r = await b.call('orient', {});
    assert.match(r.payload.metadata.index_adoption.reasons, /not a plain directory/);
    await b.close();
    const root3 = makeProject('ad-gone'); buildUserIndex(bin, root3);
    rmSync(join(root3, 'src/main.ts'));
    const c = new Adapter({ root: root3, cache: tmp('c'), env: ADOPT('read-only') });
    const g = await c.call('orient', {});
    assert.match(g.payload.metadata.index_adoption.reasons, /src\/main\.ts is unreadable/);
    await c.close();
  });

  test('a new source file after the index was built makes the user index incomplete evidence', async () => {
    const root = makeProject('ad-new'); buildUserIndex(bin, root);
    writeFileSync(join(root, 'src/extra.ts'), 'export const x = 1;\n');
    const a = new Adapter({ root, cache: tmp('c'), env: ADOPT('read-only') });
    const r = await a.call('orient', {});
    assert.equal(refresh(r).outcome, 'incompatible');
    assert.match(r.payload.metadata.index_adoption.reasons, /src\/extra\.ts is not in the index/);
    await a.close();
  });
});

describe('index adoption across versions', { skip: GRAFT && GRAFT_NEW ? false : 'needs both HARNESS_GRAFT and HARNESS_GRAFT_NEW installs' }, () => {
  test('an index built by the other graft version (changed parser/config) is refused, in both directions', async () => {
    for (const [maker, user] of [[GRAFT, GRAFT_NEW], [GRAFT_NEW, GRAFT]]) {
      Adapter.upstream = user;
      const root = makeProject('ad-xver'); buildUserIndex(maker, root);
      const before = treeHash(join(root, 'graft'));
      const a = new Adapter({ root, cache: tmp('c'), env: ADOPT('read-only') });
      const r = await a.call('orient', {});
      assert.equal(refresh(r).outcome, 'incompatible', `${versionOf(maker)} index under ${versionOf(user)}`);
      assert.match(r.payload.metadata.index_adoption.reasons, /extractor .* differs .* \(changed parser or configuration\)/);
      assert.equal(treeHash(join(root, 'graft')), before);
      await a.close();
    }
    Adapter.upstream = null;
  });
});

// MC-01 / MC-02: the copied snapshot is an explicit, link-free inventory, staged once, validated as staged,
// and only then published. In-process cases drive ensureFresh with a controlled schedule (`cfg.adoptHooks`).
const modeOf = (p) => lstatSync(p).mode & 0o7777;
const adoptedDirs = (cfg) => (existsSync(join(cfg.work, 'adopted')) ? readdirSync(join(cfg.work, 'adopted')) : []);
async function inproc(bin, root, cache, adoptHooks = null) {
  const env = {
    SEMAPRAX_HARNESS_UPSTREAM: bin, SEMAPRAX_HARNESS_PROJECT_ROOT: root, SEMAPRAX_HARNESS_CACHE_DIR: cache,
    ...(GIT ? { SEMAPRAX_HARNESS_GIT: GIT } : {}), ...ADOPT('copied-snapshot'),
  };
  const cfg = loadConfig(env);
  cfg.adoptHooks = adoptHooks;
  const identity = await probeIdentity(cfg);
  const run = () => ensureFresh(cfg, identity, { deadline: Date.now() + 120000 });
  return { cfg, identity, run };
}
const workOf = (cache) => join(ownedDir(cache), readdirSync(ownedDir(cache))[0]);

for (const [label, bin] of INSTALLS) describe(`copied snapshot inventory and binding (real graft, ${label})`, () => {
  before(() => { Adapter.upstream = bin; });

  for (const kind of ['index file', 'project source file', 'outside-project file']) {
    test(`a nested symlink to an ${kind} is refused; no original is chmod-ed or changed`, async () => {
      const root = makeProject('mc1-link'); buildUserIndex(bin, root);
      const outside = join(tmp('outside'), 'original.py'); writeFileSync(outside, 'x = 1\n');
      const target = { 'index file': join(root, 'graft', '.graph', 'wiring.json'), 'project source file': join(root, 'util.py'), 'outside-project file': outside }[kind];
      symlinkSync(target, join(root, 'graft', 'alias.txt'));
      const bytes = readFileSync(target); const mode = modeOf(target);
      const cache = tmp('c');
      const a = new Adapter({ root, cache, env: ADOPT('copied-snapshot') });
      const r = await a.call('orient', {});
      assert.equal(refresh(r).outcome, 'incompatible');
      assert.equal(refresh(r).served_by, 'rebuilt', 'falls back to an owned build');
      assert.match(r.payload.metadata.index_adoption.reasons, /symlink/);
      assert.ok(r.payload.metadata.index_adoption.reasons.length < 1500, 'bounded diagnostic');
      assert.deepEqual(readFileSync(target), bytes); assert.equal(modeOf(target), mode, 'original mode untouched');
      const adopted = join(workOf(cache), 'adopted');
      assert.ok(!existsSync(adopted) || readdirSync(adopted).length === 0, 'no snapshot, sealed or partial');
      assert.ok(lstatSync(join(root, 'graft', 'alias.txt')).isSymbolicLink());
      await a.close();
    });
  }

  test('a directory symlink and a dangling link give a bounded diagnostic and leave the target directory mode alone', async () => {
    for (const make of [(root) => symlinkSync(join(root, 'src'), join(root, 'graft', 'dirlink')), (root) => symlinkSync(join(root, 'nowhere'), join(root, 'graft', 'dangling'))]) {
      const root = makeProject('mc1-dir'); buildUserIndex(bin, root);
      make(root);
      const srcMode = modeOf(join(root, 'src')); const srcFile = modeOf(join(root, 'src', 'greet.ts'));
      const { cfg, run } = await inproc(bin, root, tmp('c'));
      const r = await run();
      assert.equal(r.adoption.outcome, 'incompatible');
      assert.match(r.adoption.reasons.join(';'), /symlink/);
      assert.ok(r.adoption.reasons.length <= 8);
      assert.equal(modeOf(join(root, 'src')), srcMode); assert.equal(modeOf(join(root, 'src', 'greet.ts')), srcFile);
      assert.deepEqual(adoptedDirs(cfg), [], 'no partial sealed snapshot');
    }
  });

  test('a special file (fifo) anywhere in the index is refused', async () => {
    const root = makeProject('mc1-fifo'); buildUserIndex(bin, root);
    mkdirSync(join(root, 'graft', 'deep', 'er'), { recursive: true });
    execFileSync('mkfifo', [join(root, 'graft', 'deep', 'er', 'pipe')]);
    const { cfg, run } = await inproc(bin, root, tmp('c'));
    const r = await run();
    assert.equal(r.adoption.outcome, 'incompatible');
    assert.match(r.adoption.reasons.join(';'), /special file/);
    assert.deepEqual(adoptedDirs(cfg), []);
  });

  test('a regular file turned into a link while staging cannot pass; nothing is published or chmod-ed', async () => {
    const root = makeProject('mc1-race'); buildUserIndex(bin, root);
    const outside = join(tmp('outside'), 'victim.txt'); writeFileSync(outside, 'secret\n'); chmodSync(outside, 0o644);
    const hooks = { beforeFile(rel, src) { if (rel === '.graph/wiring.json') { rmSync(src); symlinkSync(outside, src); } } };
    const { cfg, run } = await inproc(bin, root, tmp('c'), hooks);
    const r = await run();
    assert.equal(r.adoption.outcome, 'incompatible');
    assert.deepEqual(adoptedDirs(cfg), []);
    assert.equal(modeOf(outside), 0o644); assert.equal(readFileSync(outside, 'utf8'), 'secret\n');
  });

  test('final inventory validation rejects a staged file that became a link before sealing', async () => {
    const root = makeProject('mc1-final'); buildUserIndex(bin, root);
    const outside = join(tmp('outside'), 'victim.txt'); writeFileSync(outside, 'secret\n'); chmodSync(outside, 0o644);
    const hooks = { afterCopy(stage) { const f = join(stage, '.graph', 'wiring.json'); rmSync(f); symlinkSync(outside, f); } };
    const { cfg, run } = await inproc(bin, root, tmp('c'), hooks);
    const r = await run();
    assert.equal(r.adoption.outcome, 'incompatible');
    assert.deepEqual(adoptedDirs(cfg), []);
    assert.equal(modeOf(outside), 0o644);
  });

  test('a valid snapshot is independent regular files matching the admitted digest', async () => {
    const root = makeProject('mc1-ok'); buildUserIndex(bin, root);
    const treeBefore = treeHash(join(root, 'graft'));
    const { run } = await inproc(bin, root, tmp('c'));
    const r = await run();
    assert.equal(r.outcome, 'copied-validated-index');
    assert.equal(r.snapshotDigest, treeDigest(r.dir).digest);
    assert.equal(r.snapshotDigest, treeDigest(join(root, 'graft')).digest);
    assert.equal(treeHash(r.dir), treeBefore);
    const walk = (d) => readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(d, e.name)) : [join(d, e.name)]));
    for (const f of walk(r.dir)) {
      const st = lstatSync(f); assert.ok(st.isFile() && !st.isSymbolicLink() && st.nlink === 1, f); assert.equal(st.mode & 0o222, 0);
      assert.notEqual(st.ino, lstatSync(join(root, 'graft', f.slice(r.dir.length + 1))).ino, 'independent copy, not a hard link');
    }
    assert.equal(modeOf(join(root, 'graft', '.graph', 'wiring.json')) & 0o200, 0o200, 'user file stays writable');
  });

  const markB = (root, tag = 'B') => {
    const f = join(root, 'graft', '.graph', 'wiring.json');
    const w = JSON.parse(readFileSync(f, 'utf8')); w.meta.audit_marker = tag; writeFileSync(f, JSON.stringify(w));
  };

  test('an edit after validation but before copy is never reported as the validated digest (MC-02 schedule)', async () => {
    const root = makeProject('mc2-pause'); buildUserIndex(bin, root);
    let fired = 0;
    const hooks = { afterDigest() { if (!fired++) markB(root); } };
    const { run } = await inproc(bin, root, tmp('c'), hooks);
    const r = await run();
    assert.equal(r.outcome, 'copied-validated-index', JSON.stringify(r.adoption));
    assert.equal(r.snapshotDigest, treeDigest(r.dir).digest, 'returned digest describes the returned directory');
    assert.equal(r.snapshotDigest, treeDigest(join(root, 'graft')).digest, 'and the index that was admitted');
    const w = JSON.parse(readFileSync(join(r.dir, '.graph', 'wiring.json'), 'utf8'));
    assert.equal(w.meta.audit_marker, 'B');
    const fileNodes = new Map(w.nodes.filter((n) => n.kind === 'file').map((n) => [n.path, n.body_hash]));
    assert.deepEqual([...r.files].sort(), [...fileNodes].sort(), 'source-binding map comes from the admitted snapshot');
    assert.equal(r.adoption.coverage.files, fileNodes.size);
    assert.equal(r.index_digest, r.adoption.inputs_digest);
    assert.ok(fired >= 2, 'the first attempt was retried');
  });

  test('drift between copied files (graph/manifest metadata) is retried within a fixed bound', async () => {
    const root = makeProject('mc2-mid'); buildUserIndex(bin, root);
    let fired = 0;
    const hooks = { afterFile(rel) { if (rel.startsWith('.cache/') && !fired++) markB(root); } };
    const { run } = await inproc(bin, root, tmp('c'), hooks);
    const r = await run();
    assert.equal(r.outcome, 'copied-validated-index');
    assert.equal(treeHash(r.dir), treeHash(join(root, 'graft')), 'published bytes equal the final user index');
    assert.equal(r.snapshotDigest, treeDigest(r.dir).digest);
    assert.equal(JSON.parse(readFileSync(join(r.dir, '.graph', 'wiring.json'), 'utf8')).meta.audit_marker, 'B');
  });

  test('a source that never settles falls back to an owned build after a bounded number of attempts; last good snapshot and user index survive', async () => {
    const root = makeProject('mc2-move'); buildUserIndex(bin, root);
    const cache = tmp('c');
    const good = await (await inproc(bin, root, cache)).run();
    assert.equal(good.outcome, 'copied-validated-index');
    const goodTree = treeHash(good.dir);
    let n = 0;
    const hooks = { afterDigest() { markB(root, `m${++n}`); } };
    const { cfg, run } = await inproc(bin, root, cache, hooks);
    const r = await run();
    assert.equal(r.adoption.outcome, 'incompatible');
    assert.match(r.adoption.reasons.join(';'), /changing/);
    assert.ok(n >= 2 && n <= 5, `bounded attempts, saw ${n}`);
    assert.ok(['rebuilt', 'incremental-refresh'].includes(r.outcome));
    assert.equal(treeHash(good.dir), goodTree, 'last good snapshot intact');
    assert.ok(!adoptedDirs(cfg).some((d) => d.includes('partial')), 'no staging leftovers');
    assert.equal(JSON.parse(readFileSync(join(root, 'graft', '.graph', 'wiring.json'), 'utf8')).meta.audit_marker, `m${n}`, 'user index not repaired or reverted');
  });

  test('a staged copy that fails source binding is not served even when the index validated before the copy', async () => {
    const root = makeProject('mc2-bind'); buildUserIndex(bin, root);
    const hooks = { afterDigest() {
      const f = join(root, 'graft', '.graph', 'wiring.json'); const w = JSON.parse(readFileSync(f, 'utf8'));
      w.nodes.find((x) => x.kind === 'file').body_hash = 'f'.repeat(64); writeFileSync(f, JSON.stringify(w));
    } };
    const { cfg, run } = await inproc(bin, root, tmp('c'), hooks);
    const r = await run();
    assert.equal(r.adoption?.outcome, 'incompatible');
    assert.deepEqual(adoptedDirs(cfg), []);
  });

  test('a reused destination with altered contents is verified, not trusted by name', async () => {
    const root = makeProject('mc2-reuse'); buildUserIndex(bin, root);
    const cache = tmp('c');
    const one = await (await inproc(bin, root, cache)).run();
    const again = await (await inproc(bin, root, cache)).run();
    assert.equal(again.work.copied_bytes, 0, 'an intact snapshot is reused');
    assert.equal(again.dir, one.dir);
    for (const tamper of [
      (d) => { const f = join(d, '.graph', 'wiring.json'); chmodSync(f, 0o644); writeFileSync(f, '{"meta":{"version":1},"nodes":[]}'); },
      (d) => { writeFileSync(join(d, 'extra.txt'), 'planted'); },
    ]) {
      tamper(one.dir);
      const r = await (await inproc(bin, root, cache)).run();
      assert.equal(r.outcome, 'copied-validated-index');
      assert.ok(r.work.copied_bytes > 0, 'altered destination was not reused');
      assert.equal(treeHash(r.dir), treeHash(join(root, 'graft')));
      assert.equal(r.dir, one.dir);
      assert.ok(!existsSync(join(r.dir, 'extra.txt')));
      assert.equal(readdirSync(join(workOf(cache), 'adopted')).length, 1);
    }
  });
});
