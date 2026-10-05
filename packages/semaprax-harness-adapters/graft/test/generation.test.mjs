// HN-10: single-flight refresh and atomic generation swap for the adapter-owned index (no real graft needed).
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, test } from 'node:test';
import * as G from '../lib/generation.mjs';
import { beginGen, currentGen, publish, withRefreshLock } from '../lib/generation.mjs';
import { Adapter, makeProject, makeShim, tmp } from './helpers.mjs';

const wiring = (n) => JSON.stringify({ meta: { version: 1, gen: n }, nodes: Array.from({ length: 400 }, (_, i) => ({ id: `n${n}-${i}`, kind: 'file', path: `f${i}`, body_hash: 'a'.repeat(64) })), end: n });

describe('generation helpers', () => {
  test('a reader process polling during many publishes never sees a torn or missing generation', async () => {
    const work = tmp('gen');
    const first = beginGen(work, null);
    writeFileSync(join(first.dir, 'x'), 'x'); mkdirSync(join(first.dir, '.graph')); writeFileSync(join(first.dir, '.graph', 'wiring.json'), wiring(1));
    publish(work, first);
    const reader = spawn(process.execPath, ['--input-type=module', '-e', `
      import { acquireGen } from ${JSON.stringify(new URL('../lib/generation.mjs', import.meta.url).href)};
      import { readFileSync } from 'node:fs'; import { join } from 'node:path';
      const work = ${JSON.stringify(work)}; let reads = 0; const seen = new Set(); let bad = 0; const why = [];
      const stop = Date.now() + 4000;
      while (Date.now() < stop) {
        const g = acquireGen(work); if (!g) { bad++; why.push('no generation'); continue; }
        try { const w = JSON.parse(readFileSync(join(g.dir, '.graph', 'wiring.json'), 'utf8')); if (w.nodes.length !== 400 || w.end !== w.meta.gen) bad++; seen.add(w.meta.gen); reads++; } catch (e) { bad++; why.push(String(e.message).slice(0, 120)); } finally { g.release(); }
      }
      console.log(JSON.stringify({ reads, bad, gens: seen.size, why: why.slice(0, 3) }));`], { stdio: ['ignore', 'pipe', 'inherit'] });
    let out = ''; reader.stdout.on('data', (b) => { out += b; });
    const done = new Promise((r) => reader.on('close', r));
    for (let n = 2; n <= 40; n++) {
      await withRefreshLock(work, async () => {
        const g = beginGen(work, currentGen(work));
        mkdirSync(join(g.dir, '.graph'), { recursive: true }); writeFileSync(join(g.dir, '.graph', 'wiring.json'), wiring(n));
        publish(work, g);
      });
      await new Promise((r) => setTimeout(r, 20));
    }
    await done;
    const r = JSON.parse(out);
    assert.equal(r.bad, 0, out); assert.ok(r.reads > 100 && r.gens > 3, out);
    // Reclamation contract: with no live lease, only the current generation remains (no fixed retention count);
    // the generation the reader still pinned at the last publish goes at the next one.
    await refreshTo(work, 41);
    assert.deepEqual(readdirSync(join(work, 'gen')), [currentGen(work).name], 'unleased old generations are reclaimed');
  });

  test('a crashed holder (dead pid) does not wedge the lock; a live holder is waited for', async () => {
    const work = tmp('lock');
    mkdirSync(join(work, 'refresh.lock'), { recursive: true });
    writeFileSync(join(work, 'refresh.lock', 'owner.json'), JSON.stringify({ pid: 2 ** 22 + 12345, at: Date.now() }));
    const order = [];
    await withRefreshLock(work, async ({ coalesced }) => { order.push(['recovered', coalesced]); });
    assert.deepEqual(order, [['recovered', false]]);
    const a = withRefreshLock(work, async () => { order.push('a'); await new Promise((r) => setTimeout(r, 200)); order.push('a-done'); });
    const b = withRefreshLock(work, async ({ coalesced }) => { order.push(['b', coalesced]); });
    await Promise.all([a, b]);
    assert.deepEqual(order.slice(1), ['a', 'a-done', ['b', true]]);
  });
});

describe('two adapter processes over one cache', () => {
  test('concurrent cold refreshes coalesce into one build; both answer from a complete generation', async () => {
    const shim = makeShim({ mode: { buildDelayMs: 700 } });
    const root = makeProject('sf'); const cache = tmp('cache-sf');
    const a = new Adapter({ root, cache, upstream: shim.bin });
    const b = new Adapter({ root, cache, upstream: shim.bin });
    const [ra, rb] = await Promise.all([a.call('orient', {}), (async () => { await new Promise((r) => setTimeout(r, 150)); return b.call('orient', {}); })()]);
    for (const r of [ra, rb]) assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    assert.equal(shim.calls().filter((c) => c.argv[0] === 'build').length, 1, 'single flight: exactly one build ran');
    const outcomes = [ra, rb].map((r) => r.payload.metadata.refresh.outcome).sort();
    assert.deepEqual(outcomes, ['rebuilt', 'reused-owned-index']);
    assert.equal([ra, rb].find((r) => r.payload.metadata.refresh.outcome === 'reused-owned-index').payload.metadata.refresh.coalesced, true);
    await a.close(); await b.close();
  });

  test('a failed build leaves the previous generation current and no partial directory behind', async () => {
    const shim = makeShim();
    const root = makeProject('fail'); const cache = tmp('cache-fail');
    const a = new Adapter({ root, cache, upstream: shim.bin });
    assert.equal((await a.call('orient', {})).status, 'complete');
    const work = join(cache, 'graft-context', readdirSync(join(cache, 'graft-context'))[0]);
    const live = currentGen(work);
    writeFileSync(join(shim.dir, 'mode.json'), JSON.stringify({ failBuild: true }));
    writeFileSync(join(root, 'util.py'), 'def other(): pass\n');
    const r = await a.call('orient', {}, { payload: undefined });
    assert.notEqual(r.status, 'complete');
    assert.equal(currentGen(work).name, live.name, 'live generation unchanged');
    assert.ok(!readdirSync(join(work, 'gen')).some((e) => e.endsWith('.partial')));
    await a.close();
  });
});

// ---- MC-03: refresh lock recovery ---------------------------------------------------------------------
const ago = (p, ms) => { const t = new Date(Date.now() - ms); utimesSync(p, t, t); };
const HOUR = 3600 * 1000;
const lockDirs = (work) => readdirSync(work).filter((n) => n.startsWith('refresh.lock'));

describe('refresh lock: ownerless and malformed records', () => {
  test('an old ownerless refresh.lock (crash before owner.json) is recovered by a later process', async () => {
    const work = tmp('lock-orphan');
    mkdirSync(join(work, 'refresh.lock')); ago(join(work, 'refresh.lock'), HOUR);
    let entered = 0;
    await withRefreshLock(work, async () => { entered++; }, { timeoutMs: 2000 });
    assert.equal(entered, 1);
    await withRefreshLock(work, async () => { entered++; }, { timeoutMs: 2000 });
    assert.equal(entered, 2, 'the cache stays usable for the next refresh');
  });

  test('an old malformed owner record is recovered', async () => {
    for (const junk of ['{', '', '{"pid":"x"}', 'null']) {
      const work = tmp('lock-malformed');
      const lock = join(work, 'refresh.lock'); mkdirSync(lock);
      writeFileSync(join(lock, 'owner.json'), junk);
      ago(join(lock, 'owner.json'), HOUR); ago(lock, HOUR);
      let entered = 0;
      await withRefreshLock(work, async () => { entered++; }, { timeoutMs: 2000 });
      assert.equal(entered, 1, `owner.json=${JSON.stringify(junk)}`);
    }
  });

  test('a fresh ownerless or malformed lock is not abandoned: waiters time out with a diagnostic and never enter', async () => {
    for (const junk of [null, '{']) {
      const work = tmp('lock-fresh');
      const lock = join(work, 'refresh.lock'); mkdirSync(lock);
      if (junk) writeFileSync(join(lock, 'owner.json'), junk);
      let entered = 0;
      await assert.rejects(withRefreshLock(work, async () => { entered++; }, { timeoutMs: 300 }), /timed out waiting for the refresh lock \(owner record not yet written/);
      assert.equal(entered, 0);
      assert.ok(existsSync(lock) && !existsSync(join(lock, 'done')), 'the fresh lock was left alone');
    }
  });

  test('a process that crashes between creating the lock and publishing its owner is recoverable', async () => {
    const work = tmp('lock-crash');
    const child = spawn(process.execPath, ['--input-type=module', '-e', `
      import { withRefreshLock } from ${JSON.stringify(new URL('../lib/generation.mjs', import.meta.url).href)};
      await withRefreshLock(${JSON.stringify(work)}, async () => {}, { hooks: { afterCreate() { process.kill(process.pid, 'SIGKILL'); } } });`], { stdio: 'ignore' });
    const [, signal] = await new Promise((r) => child.on('close', (code, sig) => r([code, sig])));
    assert.equal(signal, 'SIGKILL', 'the holder died inside the acquisition window');
    assert.ok(lockDirs(work).length >= 1);
    let entered = 0;
    await assert.rejects(withRefreshLock(work, async () => { entered++; }, { timeoutMs: 150 }), /timed out/, 'still within the grace period');
    assert.equal(entered, 0);
    await withRefreshLock(work, async () => { entered++; }, { timeoutMs: 3000, initGraceMs: 200 });
    assert.equal(entered, 1);
  });

  test('a live process paused during owner initialization is not admitted alongside a second holder', async () => {
    const work = tmp('lock-paused');
    const child = spawn(process.execPath, ['--input-type=module', '-e', `
      import { withRefreshLock } from ${JSON.stringify(new URL('../lib/generation.mjs', import.meta.url).href)};
      await withRefreshLock(${JSON.stringify(work)}, async () => { console.log('in'); await new Promise((r) => setTimeout(r, 300)); console.log('out'); }, { hooks: { afterCreate: () => new Promise((r) => setTimeout(r, 1200)) } });`], { stdio: ['ignore', 'pipe', 'inherit'] });
    let out = ''; child.stdout.on('data', (b) => { out += b; });
    const closed = new Promise((r) => child.on('close', r));
    for (let i = 0; i < 100 && !lockDirs(work).length; i++) await new Promise((r) => setTimeout(r, 20));
    let entered = 0;
    await assert.rejects(withRefreshLock(work, async () => { entered++; assert.ok(!out.includes('in') || out.includes('out')); }, { timeoutMs: 500 }), /timed out/);
    assert.equal(entered, 0, 'not admitted while the owner is initializing');
    await closed;
    await withRefreshLock(work, async () => { entered++; }, { timeoutMs: 3000 });
    assert.equal(entered, 1);
    assert.match(out, /in\s+out/);
  });

  test('simultaneous recoverers of an abandoned lock never overlap and every one of them runs', async () => {
    for (const orphan of ['ownerless', 'dead-pid']) {
      const work = tmp('lock-race');
      const lock = join(work, 'refresh.lock'); mkdirSync(lock);
      if (orphan === 'dead-pid') writeFileSync(join(lock, 'owner.json'), JSON.stringify({ pid: 2 ** 22 + 777, at: Date.now() }));
      ago(lock, HOUR);
      let active = 0; let max = 0; const done = [];
      await Promise.all(Array.from({ length: 8 }, (_, i) => withRefreshLock(work, async () => {
        active++; max = Math.max(max, active);
        await new Promise((r) => setTimeout(r, 25));
        active--; done.push(i);
      }, { timeoutMs: 10000 })));
      assert.equal(max, 1, `${orphan}: critical section was shared`);
      assert.equal(done.length, 8);
    }
  });

  test('recoverers in separate processes are mutually exclusive over one abandoned lock', async () => {
    const work = tmp('lock-procs');
    const lock = join(work, 'refresh.lock'); mkdirSync(lock); ago(lock, HOUR);
    const log = join(work, 'log.txt');
    const src = `
      import { withRefreshLock } from ${JSON.stringify(new URL('../lib/generation.mjs', import.meta.url).href)};
      import { appendFileSync } from 'node:fs';
      for (let i = 0; i < 4; i++) await withRefreshLock(${JSON.stringify(work)}, async () => {
        appendFileSync(${JSON.stringify(log)}, 'in ' + process.pid + '\\n'); await new Promise((r) => setTimeout(r, 15)); appendFileSync(${JSON.stringify(log)}, 'out ' + process.pid + '\\n');
      }, { timeoutMs: 20000 });`;
    const kids = Array.from({ length: 4 }, () => spawn(process.execPath, ['--input-type=module', '-e', src], { stdio: 'inherit' }));
    const codes = await Promise.all(kids.map((k) => new Promise((r) => k.on('close', r))));
    assert.deepEqual(codes, [0, 0, 0, 0]);
    const lines = readFileSync(log, 'utf8').trim().split('\n');
    assert.equal(lines.length, 32);
    for (let i = 0; i < lines.length; i += 2) {
      assert.match(lines[i], /^in (\d+)$/); assert.equal(lines[i + 1], lines[i].replace('in', 'out'), 'sections never interleave');
    }
  });
});

// ---- MC-04: generation leases ----------------------------------------------------------------------------
const publishGen = (work, n) => {
  const g = beginGen(work, null);
  mkdirSync(join(g.dir, '.graph'), { recursive: true }); writeFileSync(join(g.dir, '.graph', 'wiring.json'), wiring(n)); writeFileSync(join(g.dir, 'asset'), `asset-${n}`);
  return publish(work, g);
};
const refreshTo = (work, n) => withRefreshLock(work, async () => publishGen(work, n));
// Reader selection: the lease API when present; the bare CURRENT read the old code offered otherwise.
const select = (work, opts) => (G.acquireGen ? G.acquireGen(work, opts) : { ...currentGen(work), release() {} });
const readGen = (g) => { const w = JSON.parse(readFileSync(join(g.dir, '.graph', 'wiring.json'), 'utf8')); return { gen: w.meta.gen, nodes: w.nodes.length, asset: readFileSync(join(g.dir, 'asset'), 'utf8') }; };
const gens = (work) => readdirSync(join(work, 'gen')).sort();

describe('generation leases', () => {
  test('a paused g1 reader survives two refreshes and reads one consistent g1 snapshot; a new reader selects g3', async () => {
    const work = tmp('lease');
    await refreshTo(work, 1);
    const reader = select(work);
    assert.equal(reader.name, 'g1');
    await refreshTo(work, 2); await refreshTo(work, 3); await refreshTo(work, 4);
    assert.deepEqual(readGen(reader), { gen: 1, nodes: 400, asset: 'asset-1' });
    const fresh = select(work);
    assert.equal(fresh.name, 'g4');
    assert.deepEqual(readGen(fresh), { gen: 4, nodes: 400, asset: 'asset-4' });
    assert.ok(gens(work).includes('g1') && gens(work).includes('g4'));
    fresh.release();
    reader.release();
    await refreshTo(work, 5);
    assert.deepEqual(gens(work), ['g5'], 'once the lease is released the unused generations are reclaimed');
  });

  test('selection versus prune under deterministic interleavings never loses a pinned generation', async () => {
    assert.equal(typeof G.acquireGen, 'function', 'lease API is exported');
    // (a) refreshes run between reading CURRENT and taking the lease: the reader must land on a live generation.
    let work = tmp('lease-a'); await refreshTo(work, 1);
    let fired = false;
    let r = G.acquireGen(work, { hooks: { afterSelect() { if (!fired) { fired = true; publishGen(work, 2); publishGen(work, 3); } } } });
    assert.equal(r.name, 'g3', 'a stale selection is detected and retried');
    assert.deepEqual(readGen(r), { gen: 3, nodes: 400, asset: 'asset-3' });
    r.release();
    // (b) refreshes run after the lease exists but before the reader's existence check: prune must honour the lease.
    work = tmp('lease-b'); await refreshTo(work, 1);
    fired = false;
    r = G.acquireGen(work, { hooks: { afterLease() { if (!fired) { fired = true; publishGen(work, 2); publishGen(work, 3); } } } });
    assert.equal(r.name, 'g1');
    assert.deepEqual(readGen(r), { gen: 1, nodes: 400, asset: 'asset-1' });
    r.release();
    publishGen(work, 4);
    assert.deepEqual(gens(work), ['g4']);
  });

  test('withGeneration releases on success, failure and abort; nothing stays pinned', async () => {
    const work = tmp('lease-rel'); await refreshTo(work, 1);
    const pinned = () => (existsSync(join(work, 'leases')) ? readdirSync(join(work, 'leases')).length : 0);
    assert.equal(typeof G.withGeneration, 'function');
    assert.equal(await G.withGeneration(work, async (g) => { assert.equal(pinned(), 1); return g.name; }), 'g1');
    await assert.rejects(G.withGeneration(work, async () => { throw new Error('query failed'); }), /query failed/);
    const ac = new AbortController();
    const held = G.withGeneration(work, (g) => new Promise((_, rej) => { ac.signal.addEventListener('abort', () => rej(new Error('cancelled'))); }));
    ac.abort();
    await assert.rejects(held, /cancelled/);
    assert.equal(pinned(), 0);
  });

  test('crashed, expired and live leases: only live ones retain a generation', async () => {
    const work = tmp('lease-crash'); await refreshTo(work, 1);
    mkdirSync(join(work, 'leases'), { recursive: true });
    const lease = (name, rec) => writeFileSync(join(work, 'leases', name), typeof rec === 'string' ? rec : JSON.stringify(rec));
    lease('g1.dead.1', { gen: 'g1', pid: 2 ** 22 + 4242, at: Date.now() }); // crashed reader
    await refreshTo(work, 2);
    assert.deepEqual(gens(work), ['g2'], 'a dead reader does not retain g1');
    const live = select(work); // g2, held by this live process
    lease('g2.expired.1', { gen: 'g2', pid: process.pid, at: Date.now() - 24 * HOUR }); // forgotten by a hung reader
    lease('g2.torn.1', '{'); ago(join(work, 'leases', 'g2.torn.1'), HOUR);
    await refreshTo(work, 3); await refreshTo(work, 4);
    assert.deepEqual(gens(work), ['g2', 'g4'], 'the live lease keeps g2; g3 and abandoned records retain nothing');
    live.release();
    await refreshTo(work, 5);
    assert.deepEqual(gens(work), ['g5']);
    assert.deepEqual(readdirSync(join(work, 'leases')), [], 'stale lease records are swept');
  });
});

describe('adapter query held across refreshes (shim upstream)', () => {
  const waitFor = async (f, what) => { for (let i = 0; i < 400; i++) { if (f()) return; await new Promise((r) => setTimeout(r, 25)); } assert.fail(`timed out: ${what}`); };

  test('a query paused on g1 completes against g1 while two rebuilds publish g2 and g3; afterwards unused generations are reclaimed', async () => {
    const shim = makeShim({ mode: { mapGate: true } });
    const root = makeProject('held'); const cache = tmp('cache-held');
    const a = new Adapter({ root, cache, upstream: shim.bin });
    const b = new Adapter({ root, cache, upstream: shim.bin });
    const held = a.call('orient', {});
    await waitFor(() => existsSync(join(shim.dir, 'gate.held')), 'query A reached the upstream');
    const g1 = readFileSync(join(shim.dir, 'gate.held'), 'utf8').trim();
    assert.match(g1, /gen\/g1$/);
    for (let i = 0; i < 2; i++) assert.equal((await b.call('orient', { refresh: 'rebuild' })).status, 'complete');
    const work = join(cache, 'graft-context', readdirSync(join(cache, 'graft-context'))[0]);
    assert.equal(currentGen(work).name, 'g3');
    assert.ok(existsSync(join(g1, '.graph', 'wiring.json')), 'the generation under an active query survived two refreshes');
    writeFileSync(join(shim.dir, 'gate.open'), '');
    const r = await held;
    assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    assert.match(r.payload.metadata.refresh.outcome, /rebuilt/);
    assert.equal((await b.call('orient', { refresh: 'rebuild' })).status, 'complete');
    assert.deepEqual(gens(work), ['g4'], 'no lease left: unused generations reclaimed');
    assert.deepEqual(existsSync(join(work, 'leases')) ? readdirSync(join(work, 'leases')) : [], []);
    await a.close(); await b.close();
  });

  test('a query that times out releases its generation', async () => {
    const shim = makeShim({ mode: { mapGate: true } });
    const root = makeProject('held-to'); const cache = tmp('cache-held-to');
    const a = new Adapter({ root, cache, upstream: shim.bin });
    const r = await a.call('orient', {}, { deadline_ms: 1500 });
    assert.notEqual(r.status, 'complete');
    const work = join(cache, 'graft-context', readdirSync(join(cache, 'graft-context'))[0]);
    for (let i = 0; i < 100 && existsSync(join(work, 'leases')) && readdirSync(join(work, 'leases')).length; i++) await new Promise((x) => setTimeout(x, 50));
    assert.deepEqual(existsSync(join(work, 'leases')) ? readdirSync(join(work, 'leases')) : [], []);
    await a.close();
  });
});
