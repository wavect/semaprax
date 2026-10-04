// HN-10: single-flight refresh and atomic generation swap for the adapter-owned index (no real graft needed).
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, test } from 'node:test';
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
      import { currentGen } from ${JSON.stringify(new URL('../lib/generation.mjs', import.meta.url).href)};
      import { readFileSync } from 'node:fs'; import { join } from 'node:path';
      const work = ${JSON.stringify(work)}; let reads = 0; const seen = new Set(); let bad = 0;
      const stop = Date.now() + 4000;
      while (Date.now() < stop) {
        const g = currentGen(work); if (!g) { bad++; continue; }
        try { const w = JSON.parse(readFileSync(join(g.dir, '.graph', 'wiring.json'), 'utf8')); if (w.nodes.length !== 400 || w.end !== w.meta.gen) bad++; seen.add(w.meta.gen); reads++; } catch { bad++; }
      }
      console.log(JSON.stringify({ reads, bad, gens: seen.size }));`], { stdio: ['ignore', 'pipe', 'inherit'] });
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
    assert.ok(readdirSync(join(work, 'gen')).filter((e) => !e.endsWith('.partial')).length <= 2, 'old generations are pruned, one kept for in-flight readers');
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
