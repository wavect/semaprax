// Test harness: fixture projects, a JSON-RPC client for the adapter, and real/shim graft locators.
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, writeFileSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ADAPTER = join(dirname(fileURLToPath(import.meta.url)), '..', 'adapter.mjs');
export const sha = (b) => createHash('sha256').update(b).digest('hex');

export function findGraft() {
  const explicit = process.env.SEMAPRAX_TEST_GRAFT;
  if (explicit && existsSync(explicit)) return explicit;
  try { return execFileSync('/bin/sh', ['-c', 'command -v graft'], { encoding: 'utf8' }).trim() || null; } catch { return null; }
}
export const GRAFT = findGraft();
export const GIT = existsSync('/usr/bin/git') ? '/usr/bin/git' : undefined;

export function tmp(label) { return realpathSync(mkdtempSync(join(tmpdir(), `hp06a-${label}-`))); }

const FILES = {
  'src/greet.ts': 'export function greetUser(name: string): string {\n  return "hello " + name;\n}\nexport class Greeter {\n  hi(n: string) { return greetUser(n); }\n}\n',
  'src/main.ts': 'import { greetUser } from "./greet";\nexport function run() { return greetUser("x"); }\n',
  'util.py': 'def parse_amount(s):\n    return int(s)\n\ndef total(xs):\n    return sum(parse_amount(x) for x in xs)\n',
  'app.spx': 'fn main() -> i32 { 0 }\n',
  'gen/ignored.py': 'def greetUser_ignored():\n    pass\n',
  '.gitignore': 'gen/\n',
};

// A mixed-language project (TypeScript + Python + .spx), git-initialised so graft honours .gitignore.
export function makeProject(label = 'proj') {
  const root = join(tmp(label), 'project');
  for (const [p, c] of Object.entries(FILES)) {
    mkdirSync(dirname(join(root, p)), { recursive: true });
    writeFileSync(join(root, p), c);
  }
  if (GIT) {
    const g = (...a) => execFileSync(GIT, ['-C', root, '-c', 'user.email=t@t', '-c', 'user.name=t', ...a], { stdio: 'ignore' });
    g('init', '-q'); g('add', '-A'); g('commit', '-qm', 'init');
  }
  return root;
}

export function snapshot(root) {
  const out = {};
  const walk = (rel) => {
    for (const e of readdirSync(join(root, rel), { withFileTypes: true })) {
      const p = rel ? `${rel}/${e.name}` : e.name;
      if (p === '.git') continue;
      if (e.isDirectory()) walk(p); else out[p] = sha(readFileSync(join(root, p)));
    }
  };
  walk('');
  return out;
}

const LIVE = new Set();
process.on('exit', () => { for (const c of LIVE) try { c.kill('SIGKILL'); } catch { /* gone */ } });

export class Adapter {
  constructor({ root, cache, upstream = GRAFT, env = {}, wrap = null }) {
    const adapterEnv = {
      PATH: dirname(process.execPath) + ':/usr/bin:/bin',
      SEMAPRAX_HARNESS_UPSTREAM: upstream,
      SEMAPRAX_HARNESS_PROJECT_ROOT: root,
      SEMAPRAX_HARNESS_CACHE_DIR: cache,
      ...(GIT ? { SEMAPRAX_HARNESS_GIT: GIT } : {}),
      ...env,
    };
    const [cmd, args] = wrap ? [wrap[0], [...wrap.slice(1), process.execPath, ADAPTER]] : [process.execPath, [ADAPTER]];
    this.child = spawn(cmd, args, { env: adapterEnv, stdio: ['pipe', 'pipe', 'pipe'] });
    LIVE.add(this.child);
    this.child.on('close', () => LIVE.delete(this.child));
    this.pending = new Map();
    this.nextId = 1;
    this.stderr = '';
    this.child.stderr.on('data', (b) => { this.stderr += b; });
    let buf = '';
    this.child.stdout.on('data', (b) => {
      buf += b;
      let i;
      while ((i = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, i); buf = buf.slice(i + 1);
        const m = JSON.parse(line);
        this.pending.get(m.id)?.(m); this.pending.delete(m.id);
      }
    });
    this.exited = new Promise((r) => this.child.on('close', r));
  }
  rpc(method, params) {
    const id = this.nextId++;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  }
  notify(method, params) { this.child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method, params }) + '\n'); }
  async init(offered = [{ kind: 'context.repository', version: 1 }]) {
    return (await this.rpc('harness/initialize', { protocol: 'semaprax.harness-rpc.v1', host_version: 't', descriptor_digest: 'x', offered, project: {} })).result;
  }
  envelope(operation, payload, extra = {}) {
    const n = this.nextId;
    return {
      schema: 'semaprax.harness-request.v1', invocation_id: `inv-${String(n).padStart(6, '0')}`,
      project: { id: 'p', worktree: 'w', revision: 'r' }, lock_digest: 'l',
      capability: { kind: 'context.repository', version: 1 }, operation, deadline_ms: 60000,
      budget: { max_result_bytes: 65536, remaining_calls: 8 }, lineage: [], payload, ...extra,
    };
  }
  async call(operation, payload, extra) { return (await this.rpc('harness/invoke', this.envelope(operation, payload, extra))).result; }
  async close() {
    if (this.child.exitCode === null) {
      this.rpc('harness/shutdown', {});
      const t = setTimeout(() => this.child.kill('SIGKILL'), 5000);
      await this.exited; clearTimeout(t);
    }
  }
}

// A stand-in "graft" package for tests that must observe what the adapter passes to the upstream.
// Behaviour is read from <dir>/mode.json (never from env); every call appends to <dir>/calls.jsonl.
export function makeShim({ name = '@nanonets/graft', version = '0.18.0', mode = {} } = {}) {
  const dir = join(tmp('shim'), 'pkg');
  mkdirSync(join(dir, 'dist'), { recursive: true });
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name, version, license: 'MIT', type: 'module' }));
  writeFileSync(join(dir, 'mode.json'), JSON.stringify(mode));
  const cli = join(dir, 'dist', 'cli.js');
  writeFileSync(cli, `#!/usr/bin/env node
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const pkg = join(dirname(fileURLToPath(import.meta.url)), '..');
const mode = JSON.parse(readFileSync(join(pkg, 'mode.json'), 'utf8'));
const argv = process.argv.slice(2);
appendFileSync(join(pkg, 'calls.jsonl'), JSON.stringify({ argv, env: process.env, cwd: process.cwd(), pid: process.pid }) + '\\n');
const dirIdx = argv.indexOf('--dir');
const idx = dirIdx >= 0 ? argv[dirIdx + 1] : null;
if (argv[0] === '--version') { console.log(${JSON.stringify(version)}); }
else if (argv[0] === 'build') {
  mkdirSync(join(idx, '.graph'), { recursive: true });
  writeFileSync(join(idx, '.graph', 'wiring.json'), JSON.stringify({ meta: { version: 1 }, nodes: [] }));
} else if (argv[0] === 'check') { console.log(JSON.stringify({ graph: { ok: true, missing: false } })); }
else if (mode.hang) { setInterval(() => {}, 1000); }
else if (argv[0] === 'map') { console.log(JSON.stringify({ totals: { files: 0, symbols: 0, edges: 0, languages: [] }, dirs: [], hotspots: [], dropped: 0 })); }
else { process.exit(2); }
`);
  chmodSync(cli, 0o755);
  return { dir, bin: cli, calls: () => (existsSync(join(dir, 'calls.jsonl')) ? readFileSync(join(dir, 'calls.jsonl'), 'utf8').trim().split('\n').map((l) => JSON.parse(l)) : []) };
}
