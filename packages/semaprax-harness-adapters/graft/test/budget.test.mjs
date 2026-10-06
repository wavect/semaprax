// DV-13: the complete result envelope, not just `payload.items`, fits the requested byte budget (shim upstream).
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { afterEach, describe, test } from 'node:test';
import { Adapter, makeProject, makeShim, tmp } from './helpers.mjs';

const FULL = (c) => 'sha256:' + c.repeat(64);

function crowdedProject(extra, { suffix = 'x'.repeat(35), multibyte = false } = {}) {
  const root = makeProject('budget');
  for (let i = 0; i < extra; i++) {
    const name = `extra/${multibyte ? '漢字é'.repeat(6) : 'file'}-${String(i).padStart(3, '0')}-${suffix}.spx`;
    mkdirSync(dirname(join(root, name)), { recursive: true });
    writeFileSync(join(root, name), 'fn main() -> i32 { 0 }\n');
  }
  return root;
}

async function ask(a, operation, payload, bytes) {
  const req = a.envelope(operation, payload, {
    budget: { max_result_bytes: bytes, remaining_calls: 8 },
    project: { id: FULL('1'), worktree: FULL('2'), revision: FULL('3') },
  });
  const result = (await a.rpc('harness/invoke', req)).result;
  return { result, bytes: Buffer.byteLength(JSON.stringify(result)) };
}

describe('graft result envelope budget (shim upstream)', () => {
  const open = [];
  afterEach(async () => { for (const a of open.splice(0)) await a.close(); });
  const spawnAdapter = (root) => { const a = new Adapter({ root, cache: tmp('c'), upstream: makeShim().bin }); open.push(a); return a; };
  for (const [label, opts] of [['ascii', {}], ['multibyte', { multibyte: true }]]) {
    test(`${label}: zero items and many skipped files fit 4096 bytes with honest coverage`, async () => {
      const root = crowdedProject(80, opts);
      const a = spawnAdapter(root);
      await a.init();
      const { result, bytes } = await ask(a, 'orient', { max_items: 10 }, 4096);
      assert.ok(bytes <= 4096, `envelope is ${bytes} bytes`);
      assert.equal(result.status, 'partial');
      const p = result.payload;
      assert.equal(p.items.length, 0);
      assert.equal(p.coverage.complete, false);
      assert.equal(p.coverage.exhaustive, false);
      assert.equal(p.metadata.absence_proven, false);
      assert.ok(p.metadata.skipped_omitted > 0, 'omitted skipped files are counted');
      assert.ok(p.coverage.skipped.length + p.metadata.skipped_omitted >= 80);
      assert.ok(result.diagnostics.some((d) => d.code === 'graft.coverage-incomplete'));
    });
  }

  test('a small request over a small project is not truncated and keeps metadata', async () => {
    const a = spawnAdapter(makeProject('small'));
    await a.init();
    const { result, bytes } = await ask(a, 'orient', {}, 65536);
    assert.ok(bytes < 65536);
    assert.equal(result.status, 'complete', JSON.stringify(result.diagnostics));
    assert.ok(result.payload.metadata.refresh, 'ordinary results keep refresh metadata');
    assert.ok(!result.diagnostics.some((d) => d.code === 'graft.truncated'));
  });

  test('an unsatisfiable budget is a bounded refusal, never an oversized complete result', async () => {
    const a = spawnAdapter(crowdedProject(5));
    await a.init();
    const { result } = await ask(a, 'orient', {}, 300);
    assert.notEqual(result.status, 'complete');
    assert.equal(result.status, 'refused');
    assert.equal(result.payload, null);
    assert.equal(result.diagnostics[0].code, 'graft.result-too-large');
  });

  test('the semaprax-source early return is bounded too', async () => {
    const a = spawnAdapter(makeProject('early'));
    await a.init();
    const { result, bytes } = await ask(a, 'skeleton', { path: `${'d'.repeat(120)}/${'e'.repeat(120)}/x.spx` }, 700);
    assert.ok(bytes <= 700, `envelope is ${bytes} bytes`);
    assert.notEqual(result.status, 'complete');
  });
});
