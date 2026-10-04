// HN-10: opt-in adoption of a user-owned graft index. Real graft, every provisioned install.
import assert from 'node:assert/strict';
import { cpSync, existsSync, readFileSync, readdirSync, renameSync, rmSync, statSync, symlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { before, describe, test } from 'node:test';
import { ADOPT, Adapter, GRAFT, GRAFT_NEW, buildUserIndex, makeProject, tmp, treeHash, versionOf } from './helpers.mjs';

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
