// Isolation tests: what the adapter passes to graft (shim), identity checks, cancellation, and a
// network-denied run of the real graft under macOS sandbox-exec.
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { describe, test } from 'node:test';
import { Adapter, GRAFT, makeProject, makeShim, tmp } from './helpers.mjs';

const PLANTED = {
  GRAFT_API_KEY: 'sk-planted-remote-ingestion', GRAFT_PROVIDER: 'openai', GRAFT_MODEL: 'gpt-x', GRAFT_BASE_URL: 'https://example.invalid/v1',
  OPENAI_API_KEY: 'sk-openai', ANTHROPIC_API_KEY: 'sk-anthropic', GITHUB_TOKEN: 'ghp_x', NPM_TOKEN: 'npm_x',
  HTTPS_PROXY: 'http://proxy.invalid:3128', GRAFT_POSTHOG_KEY: 'phc_planted', GRAFT_DIR: '/etc', DO_NOT_TRACK: '0', CI: 'true',
};

describe('what graft is given (shim upstream)', () => {
  test('planted remote-ingestion settings are not inherited; telemetry off; no --deep; private PATH/HOME/cwd', async () => {
    const shim = makeShim();
    const root = makeProject('shim'); const cache = tmp('cache-shim');
    const a = new Adapter({ root, cache, upstream: shim.bin, env: PLANTED });
    const r = await a.call('orient', {});
    assert.equal(r.status, 'complete', JSON.stringify(r.diagnostics));
    const calls = shim.calls();
    assert.deepEqual(calls.map((c) => c.argv[0]), ['--version', 'build', 'map']);
    for (const c of calls) {
      for (const k of Object.keys(PLANTED).filter((k) => k !== 'DO_NOT_TRACK' && k !== 'CI')) assert.ok(!(k in c.env), `${k} leaked`);
      assert.ok(!('CI' in c.env));
      assert.equal(c.env.DO_NOT_TRACK, '1', 'telemetry opt-out is forced, not inherited');
      assert.ok(!c.argv.includes('--deep') && !c.argv.includes('--api-key') && !c.argv.includes('--provider') && !c.argv.includes('--base-url'));
      assert.ok(c.env.HOME.startsWith(cache), 'HOME is private to the adapter cache');
      assert.equal(c.env.PATH, join(c.env.HOME, '..', 'bin').replace(/\/home\/\.\.\//, '/'), 'PATH is only the private bin dir (node [+ git])');
      assert.ok(!existsSync(join(c.env.PATH, 'npm')), 'no npm on PATH: graft cannot run its registry update check');
      assert.ok(c.cwd.startsWith(cache), 'cwd is an empty cache dir (graft loads dotenv from cwd)');
      assert.equal(Object.keys(c.env).filter((k) => /KEY|TOKEN|PROXY|GRAFT_(PROVIDER|MODEL|BASE_URL|DIR)/.test(k)).length, 0);
    }
    const build = calls.find((c) => c.argv[0] === 'build');
    assert.ok(build.argv.includes('--dir'));
    assert.ok(build.argv[build.argv.indexOf('--dir') + 1].startsWith(cache), 'index lives in the cache root, not the repo');
    await a.close();
  });

  test('a package that is not @nanonets/graft is refused before any query', async () => {
    const shim = makeShim({ name: 'graft-lookalike' });
    const a = new Adapter({ root: makeProject('imp'), cache: tmp('c'), upstream: shim.bin });
    const r = await a.call('orient', {});
    assert.equal(r.status, 'refused');
    assert.equal(r.diagnostics[0].code, 'graft.identity-mismatch');
    assert.ok(shim.calls().every((c) => c.argv[0] === '--version'), 'only the identity probe ran');
    await a.close();
  });

  test('an untested graft version is unsupported', async () => {
    const shim = makeShim({ version: '0.21.1' });
    const a = new Adapter({ root: makeProject('ver'), cache: tmp('c'), upstream: shim.bin });
    const r = await a.call('orient', {});
    assert.equal(r.status, 'unsupported');
    assert.equal(r.diagnostics[0].code, 'graft.version-untested');
    await a.close();
  });

  test('cancel kills the running graft process group and answers promptly', async () => {
    const shim = makeShim({ mode: { hang: true } });
    const a = new Adapter({ root: makeProject('cancel'), cache: tmp('c'), upstream: shim.bin });
    const env = a.envelope('orient', {});
    const p = a.rpc('harness/invoke', env);
    let pid;
    for (let i = 0; i < 100 && !pid; i++) { await new Promise((r) => setTimeout(r, 100)); pid = shim.calls().find((c) => c.argv[0] === 'map')?.pid; }
    assert.ok(pid, 'graft query started');
    a.notify('harness/cancel', { invocation_id: env.invocation_id });
    const res = (await p).result;
    assert.equal(res.status, 'refused');
    assert.equal(res.diagnostics[0].code, 'graft.cancelled');
    await new Promise((r) => setTimeout(r, 200));
    assert.throws(() => process.kill(pid, 0), /ESRCH/, 'graft is gone');
    await a.close();
  });

  test('the invocation deadline bounds a hung graft', async () => {
    const shim = makeShim({ mode: { hang: true } });
    const a = new Adapter({ root: makeProject('dl'), cache: tmp('c'), upstream: shim.bin });
    const t = Date.now();
    const r = await a.call('orient', {}, { deadline_ms: 1500 });
    assert.equal(r.status, 'failed');
    assert.equal(r.diagnostics[0].code, 'graft.timeout');
    assert.ok(Date.now() - t < 10000);
    await a.close();
  });
});

const sandbox = existsSync('/usr/bin/sandbox-exec') && process.platform === 'darwin';
describe('network blocked (macOS sandbox-exec deny network*)', { skip: !GRAFT ? 'graft not installed' : !sandbox ? 'sandbox-exec unavailable' : false }, () => {
  test('cold build and warm reuse both work with all network denied', async () => {
    const wrap = ['/usr/bin/sandbox-exec', '-p', '(version 1)(allow default)(deny network*)'];
    // Positive control: the sandbox really denies network for a child.
    const probe = new Adapter({ root: makeProject('probe'), cache: tmp('c'), wrap });
    await probe.close();
    const net = await import('node:child_process').then(({ spawnSync }) => spawnSync(wrap[0], [wrap[1], process.execPath, '-e',
      "require('net').connect(443,'1.1.1.1').on('error',e=>{console.log(e.code);process.exit(0)}).on('connect',()=>{console.log('CONNECTED');process.exit(0)})"], { encoding: 'utf8', timeout: 15000 }));
    assert.doesNotMatch(net.stdout, /CONNECTED/);
    const root = makeProject('net'); const cache = tmp('cache-net');
    const a = new Adapter({ root, cache, wrap });
    const cold = await a.call('search', { query: 'greetUser' });
    assert.equal(cold.status, 'complete', JSON.stringify(cold.diagnostics));
    assert.equal(cold.payload.metadata.refresh.action, 'build');
    assert.equal(cold.payload.items[0].path, 'src/greet.ts');
    const warm = await a.call('references', { symbol: 'greetUser' });
    assert.equal(warm.status, 'complete');
    assert.equal(warm.payload.metadata.refresh.action, 'reuse');
    await a.close();
    // A second adapter process over the same cache reuses the warm index, still offline.
    const b = new Adapter({ root, cache, wrap });
    const again = await b.call('orient', {});
    assert.equal(again.payload.metadata.refresh.action, 'reuse');
    await b.close();
  });
});
