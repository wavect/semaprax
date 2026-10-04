#!/usr/bin/env node
// Qualification probe: node scripts/qualify.mjs <graft-executable> [--node <node>] [--json]
// Runs the installed graft against a throwaway project (private HOME, DO_NOT_TRACK, no npm on PATH) and
// prints the compatibility-profile entry the adapter needs (compat/profiles.json). It measures what the
// installed parser really indexes; it never infers language support from documentation.
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

const SAMPLES = {
  '.ts': 'export function f(){}\n', '.tsx': 'export function f(){ return null }\n', '.js': 'function f(){}\n', '.jsx': 'function f(){}\n',
  '.mjs': 'function f(){}\n', '.cjs': 'function f(){}\n', '.py': 'def f(): pass\n', '.go': 'package a\nfunc F() {}\n', '.rs': 'pub fn f() {}\n',
  '.java': 'class A { void f(){} }\n', '.kt': 'fun f() {}\n', '.scala': 'object A { def f() = 1 }\n', '.rb': 'def f; end\n',
  '.php': '<?php function f(){}\n', '.c': 'int f(){return 0;}\n', '.h': 'int f(void);\n', '.cpp': 'int f(){return 0;}\n', '.hpp': 'int f();\n',
  '.cc': 'int f(){return 0;}\n', '.cs': 'class A { void F(){} }\n', '.swift': 'func f() {}\n', '.sql': 'create table t(a int);\n',
  '.sh': 'f(){ :; }\n', '.proto': 'message M {}\n', '.spx': 'fn main() -> i32 { 0 }\n', '.spatch': 'patch\n',
};
// --dir is a hidden option (absent from --help); the build below proves it behaviourally.
const COMMANDS = {
  build: ['--no-reuse'], ask: ['--json', '--limit', '--in', '--no-refresh'], check: ['--json'],
  map: ['--json', '--max-dirs', '--no-refresh'], callers: ['--json', '--direction', '--depth', '--in', '--no-refresh'],
  skeleton: ['--json', '--no-refresh'], grep: ['--json', '--fixed', '--in', '--no-refresh'],
};

const args = process.argv.slice(2);
const nodeAt = args.indexOf('--node');
const upstream = args.find((a, i) => !a.startsWith('--') && (nodeAt < 0 || i !== nodeAt + 1));
if (!upstream) { console.error('usage: qualify.mjs <graft-executable> [--node <node>]'); process.exit(2); }
const nodeBin = nodeAt >= 0 ? args[nodeAt + 1] : process.execPath;
const work = realpathSync(mkdtempSync(join(tmpdir(), 'graft-qualify-')));
for (const d of ['bin', 'home', 'cwd']) mkdirSync(join(work, d));
execFileSync('/bin/ln', ['-s', nodeBin, join(work, 'bin', 'node')]);
const env = { PATH: join(work, 'bin'), HOME: join(work, 'home'), DO_NOT_TRACK: '1', CI: '1', GRAFT_NO_REFRESH: '1', LC_ALL: 'C' };
const run = (a) => spawnSync(upstream, a, { env, cwd: join(work, 'cwd'), encoding: 'utf8', timeout: 120000 });

const version = run(['--version']).stdout.trim();
let pkg = null;
for (let d = dirname(realpathSync(upstream)), i = 0; i < 4 && !pkg; i++, d = dirname(d)) { try { pkg = JSON.parse(readFileSync(join(d, 'package.json'), 'utf8')); } catch { /* up */ } }
const proj = join(work, 'proj'); mkdirSync(proj);
for (const [e, c] of Object.entries(SAMPLES)) writeFileSync(join(proj, `probe${e}`), c);
const idx = join(work, 'idx');
const b = run(['build', '--dir', idx, proj]);
const wiring = JSON.parse(readFileSync(join(idx, '.graph', 'wiring.json'), 'utf8'));
const symbolFiles = new Set(wiring.nodes.filter((n) => n.kind !== 'file').map((n) => n.path));
const fileNodes = new Set(wiring.nodes.filter((n) => n.kind === 'file').map((n) => n.path));
const parsed = Object.keys(SAMPLES).filter((e) => fileNodes.has(`probe${e}`));
const extractor = readdirSync(join(idx, '.cache')).map((f) => /^fingerprint\.(.+)\.json$/.exec(f)?.[1]).find(Boolean) ?? null;
const flags = {};
for (const [c, fl] of Object.entries(COMMANDS)) {
  const h = run([c, '--help']).stdout;
  flags[c] = fl.filter((f) => !new RegExp(`(^|[\\s,])${f}\\b`).test(h));
}
const profile = {
  extractor, wiring_meta_version: wiring.meta?.version ?? null, languages_seen: wiring.meta?.languages ?? [],
  parsed_extensions: parsed, symbol_extensions: Object.keys(SAMPLES).filter((e) => symbolFiles.has(`probe${e}`)),
  unparsed_extensions: Object.keys(SAMPLES).filter((e) => !fileNodes.has(`probe${e}`)),
};
const report = {
  executable_version: version, package: pkg ? { name: pkg.name, version: pkg.version, repository: pkg.repository?.url ?? null } : null,
  build_exit: b.status, missing_flags: flags, profile,
  verdict: b.status === 0 && Object.values(flags).every((m) => !m.length) && profile.wiring_meta_version === 1 ? 'candidate' : 'incompatible',
};
console.log(JSON.stringify({ [version]: report }, null, 2));
process.exit(report.verdict === 'candidate' ? 0 : 1);
