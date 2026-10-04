import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { decide } from './adapter.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const drive = join(here, '..', 'tools', 'drive.py');

function wire(payload) {
  const r = spawnSync('python3', [drive, '--kind', 'decision.evaluate', '--op', 'evaluate', '--payload', JSON.stringify(payload),
    '--', 'node', join(here, 'adapter.mjs')], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  // drive.py pretty-prints three JSON objects; split on top-level "}\n{"
  const docs = r.stdout.split(/\n(?=\{)/).map((s) => JSON.parse(s));
  assert.equal(docs.length, 3);
  return docs[1].result;
}

test('threshold picks strongest above, cheapest below', () => {
  assert.equal(decide({ task: 'model-route/v1', features: { complexity: 0.9 }, options: ['small', 'large'] }).choice, 'large');
  assert.equal(decide({ task: 'model-route/v1', features: { complexity: 0.1 }, options: ['small', 'large'] }).choice, 'small');
});

test('abstains when features are missing or invalid', () => {
  for (const features of [{}, { complexity: 'high' }, { complexity: null }]) {
    const d = decide({ task: 'model-route/v1', features, options: ['a', 'b'] });
    assert.deepEqual(d, { choice: null, scores: {}, abstain: true });
  }
});

test('choice is always one of options; deterministic', () => {
  const p = { task: 'model-route/v1', features: { complexity: 0.5 }, options: ['x', 'y', 'z'] };
  assert.ok(p.options.includes(decide(p).choice));
  assert.deepEqual(decide(p), decide(p));
});

test('unknown task abstains', () => {
  assert.equal(decide({ task: 'other/v1', features: { complexity: 1 }, options: ['a'] }).abstain, true);
});

test('over real stdio frames', () => {
  const r = wire({ task: 'model-route/v1', features: { complexity: 0.8 }, options: ['m1', 'm2'] });
  assert.equal(r.status, 'complete');
  assert.equal(r.payload.choice, 'm2');
  assert.equal(r.capability.kind, 'decision.evaluate');
});
