// Project/cache configuration, upstream identity, index lifecycle and coverage.
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { basename, dirname, isAbsolute, join, posix, sep } from 'node:path';
import { execFileSync } from 'node:child_process';
import { findGit, prepareWork, runGraft, RunError, sanitizedEnv } from './runner.mjs';
import { QUALIFICATION_PATH, loadProfiles, profileFor, testedVersions } from './compat.mjs';
import { acquireGen, beginGen, discardGen, pinGen, publish, withRefreshLock } from './generation.mjs';
import { adoptCopiedSnapshot, adoptionConfig, treeDigest, verifyUserIndex } from './adopt.mjs';

export const PROVIDER_ID = 'org.nanonets/graft-context';
export const UPSTREAM_PACKAGE = '@nanonets/graft';
const MAX_WIRING_BYTES = 64 * 1024 * 1024;
export const MAX_INDEX_FILES = 20000;
const MAX_WALK_FILES = 50000;
const MAX_SKIPPED_LISTED = 200;

// Copied from graft 0.18.0 dist/context/build.js (CODE_EXTENSIONS) and dist/ingest/fs.js (SKIP_DIRS).
const GRAFT_CODE_EXTENSIONS = new Set([
  '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.py', '.go', '.rs', '.java', '.kt', '.scala', '.rb', '.php',
  '.c', '.h', '.cpp', '.hpp', '.cc', '.cs', '.swift', '.sql', '.sh', '.proto',
]);
const SEMAPRAX_EXTENSIONS = new Set(['.spx', '.spatch']);
const SKIP_DIRS = new Set(['node_modules', 'dist', 'build', '_build', 'out', 'target', 'vendor', 'coverage', '__pycache__', 'venv']);
const LANGUAGES = {
  '.ts': 'typescript', '.tsx': 'typescript', '.js': 'javascript', '.jsx': 'javascript', '.mjs': 'javascript',
  '.cjs': 'javascript', '.py': 'python', '.go': 'go', '.rs': 'rust', '.java': 'java', '.kt': 'kotlin',
  '.scala': 'scala', '.rb': 'ruby', '.php': 'php', '.c': 'c', '.h': 'c', '.cpp': 'cpp', '.hpp': 'cpp', '.cc': 'cpp',
  '.cs': 'csharp', '.swift': 'swift', '.sql': 'sql', '.sh': 'shell', '.proto': 'protobuf', '.spx': 'semaprax',
};

export const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
export const extOf = (p) => { const i = p.lastIndexOf('.'); return i < 0 ? '' : p.slice(i).toLowerCase(); };
export const languageOf = (p) => LANGUAGES[extOf(p)] ?? 'unknown';
export const isSemaprax = (p) => SEMAPRAX_EXTENSIONS.has(extOf(p));

export class Refusal extends Error {
  constructor(status, code, message) { super(message); this.status = status; this.code = code; }
}

// Relative POSIX path inside the project, or null.
export function safeRel(p) {
  if (typeof p !== 'string' || !p || p.length > 1024 || p.includes('\0') || p.includes('\\') || isAbsolute(p)) return null;
  const n = posix.normalize(p);
  if (n === '.' || n.startsWith('../') || n === '..' || n.startsWith('/')) return null;
  return n;
}

export function loadConfig(env) {
  const upstream = env.SEMAPRAX_HARNESS_UPSTREAM;
  const root = env.SEMAPRAX_HARNESS_PROJECT_ROOT;
  const cache = env.SEMAPRAX_HARNESS_CACHE_DIR;
  const missing = [];
  if (!upstream || !isAbsolute(upstream)) missing.push('SEMAPRAX_HARNESS_UPSTREAM (absolute path)');
  if (!root || !isAbsolute(root)) missing.push('SEMAPRAX_HARNESS_PROJECT_ROOT (absolute path)');
  if (!cache || !isAbsolute(cache)) missing.push('SEMAPRAX_HARNESS_CACHE_DIR (absolute path)');
  if (missing.length) throw new Refusal('unavailable', 'graft.config-missing', `host did not provide: ${missing.join(', ')}`);
  let realRoot;
  try { realRoot = realpathSync(root); } catch { throw new Refusal('unavailable', 'graft.project-missing', 'project root does not exist'); }
  const key = sha256(Buffer.from(realRoot)).slice(0, 32);
  const work = join(cache, 'graft-context', key);
  const cacheReal = (() => { try { mkdirSync(cache, { recursive: true }); return realpathSync(cache); } catch { return cache; } })();
  if (realRoot === cacheReal || realRoot.startsWith(cacheReal + sep) || cacheReal.startsWith(realRoot + sep)) {
    // A cache inside the project would be indexed as project content and pollute the user's tree.
    throw new Refusal('refused', 'graft.cache-inside-project', 'cache root must not overlap the project root');
  }
  return { upstream, root: realRoot, work, git: findGit(env.SEMAPRAX_HARNESS_GIT), nodePath: process.execPath, adopt: adoptionConfig(env) };
}

// Verifies the executable is really @nanonets/graft at a tested version (similarly named
// packages must not pass) and returns {version, license}.
export async function probeIdentity(cfg, signal) {
  let real;
  try { real = realpathSync(cfg.upstream); } catch { throw new Refusal('unavailable', 'graft.upstream-missing', 'upstream executable not found'); }
  let dir = dirname(real);
  let pkg = null;
  for (let i = 0; i < 4 && !pkg; i++, dir = dirname(dir)) {
    const f = join(dir, 'package.json');
    if (existsSync(f)) { try { pkg = JSON.parse(readFileSync(f, 'utf8')); } catch { /* keep looking */ } }
  }
  if (!pkg || pkg.name !== UPSTREAM_PACKAGE) {
    throw new Refusal('refused', 'graft.identity-mismatch', `executable is not package ${UPSTREAM_PACKAGE}`);
  }
  const repo = typeof pkg.repository === 'string' ? pkg.repository : pkg.repository?.url ?? '';
  if (!loadProfiles().repositories.some((r) => repo.toLowerCase().replace(/\.git$/, '').endsWith(`github.com/${r.toLowerCase()}`))) {
    throw new Refusal('refused', 'graft.identity-mismatch', `package repository "${repo}" is not a known upstream of ${UPSTREAM_PACKAGE}`);
  }
  prepareWork(cfg.work, cfg.nodePath, cfg.git);
  const r = await runGraft(cfg.upstream, ['--version'], { work: cfg.work, signal, timeoutMs: 10000 });
  const reported = r.stdout.trim();
  if (r.code !== 0 || reported !== pkg.version) {
    throw new Refusal('refused', 'graft.identity-mismatch', `--version reported "${reported}" but package is ${pkg.version}`);
  }
  const profile = profileFor(pkg.version);
  if (!profile) {
    throw new Refusal('unsupported', 'graft.version-unqualified', `graft ${pkg.version} has no compatibility profile (qualified: ${testedVersions().join(', ')}); to qualify it: ${QUALIFICATION_PATH}`);
  }
  return { version: pkg.version, license: pkg.license ?? null, profile };
}

function readWiring(idx) {
  const f = join(idx, '.graph', 'wiring.json');
  if (!existsSync(f) || statSync(f).size > MAX_WIRING_BYTES) return null;
  try {
    const d = JSON.parse(readFileSync(f, 'utf8'));
    if (d?.meta?.version !== 1 || !Array.isArray(d.nodes)) return null;
    const files = new Map();
    for (const n of d.nodes) if (n.kind === 'file' && typeof n.path === 'string' && /^[0-9a-f]{64}$/.test(n.body_hash ?? '')) files.set(n.path, n.body_hash);
    return files;
  } catch { return null; }
}

function indexDigest(files) {
  const h = createHash('sha256');
  for (const p of [...files.keys()].sort()) h.update(`${p}\0${files.get(p)}\n`);
  return h.digest('hex');
}

const driftPaths = (g) => new Set([...(g.added ?? []), ...(g.removed ?? []), ...(g.changed ?? []), ...(g.stale ?? [])].map((s) => s.split('#')[0]));

// Resolves the index to query: an adopted user index (opt-in, verified every call), else the adapter-owned
// generation, building or refreshing it under a single-flight lock and publishing atomically.
// Returns {dir, files, action, outcome, work, ms, files_changed, index_digest, adoption}.
//   outcome: reused-user-index | copied-validated-index | incremental-refresh | rebuilt | incompatible
//            (| reused-owned-index when the owned index was already fresh).
export async function ensureFresh(cfg, identity, { signal, deadline, force = false, readOnly = false }) {
  const { version, profile } = identity;
  prepareWork(cfg.work, cfg.nodePath, cfg.git);
  const t0 = Date.now();
  const left = () => Math.max(1000, deadline - Date.now());
  let adoption = null;
  if (cfg.adopt && !force) {
    if (cfg.adopt.invalid) adoption = { outcome: 'incompatible', reasons: [`invalid adoption setting ${cfg.adopt.mode} / ${cfg.adopt.rel}`] };
    else {
      const copied = cfg.adopt.mode === 'copied-snapshot';
      const v = copied ? adoptCopiedSnapshot(cfg, profile, version, cfg.adopt) : verifyUserIndex(cfg, profile, version, cfg.adopt);
      if (v.ok) {
        // For a copied snapshot v describes the staged, validated bytes that were then published at v.dir:
        // directory, digest, source-binding map and descriptor are all one admitted snapshot.
        const work = { ...v.work, index_ms: 0, copied_bytes: copied ? v.copied_bytes : 0 };
        const outcome = copied ? 'copied-validated-index' : 'reused-user-index';
        const before = copied ? null : treeDigest(v.dir).digest;
        return {
          dir: v.dir, files: v.files, action: 'reuse', outcome, work, ms: Date.now() - t0, files_changed: 0, index_digest: v.descriptor.inputs_digest,
          adoption: { ...v.descriptor, mode: cfg.adopt.mode }, userIndexDigest: before, userDir: copied ? null : v.dir, snapshotDigest: copied ? v.snapshotDigest : null,
        };
      }
      adoption = { outcome: 'incompatible', reasons: v.reasons, work: v.work };
    }
  }
  const owned = await ensureOwned(cfg, identity, { signal, left, force, readOnly });
  const work = { ...owned.work, ...(adoption?.work ?? {}) };
  return { ...owned, work, ms: Date.now() - t0, adoption: adoption ? { outcome: 'incompatible', reasons: adoption.reasons, fallback: owned.outcome } : null };
}

const driftFiles = (g) => driftPaths(g).size;

// The returned index carries `release()`: the generation it names stays pinned (never pruned) until the caller
// has finished its last read and calls it. Every other generation pinned along the way is released here.
async function ensureOwned(cfg, identity, opts) {
  const pins = [];
  let r;
  try { r = await ensureOwnedPinned(cfg, identity, opts, pins); } catch (e) { for (const p of pins) p.release(); throw e; }
  const keep = basename(r.dir);
  let kept = null;
  for (const p of pins) { if (!kept && p.name === keep) kept = p; else p.release(); }
  return { ...r, release: () => kept?.release() };
}

async function ensureOwnedPinned(cfg, identity, { signal, left, force, readOnly }, pins) {
  const pin = (g) => { const p = g ? pinGen(cfg.work, g) : null; if (p) pins.push(p); return p; };
  const select = () => { const p = acquireGen(cfg.work); if (p) pins.push(p); return p; };
  const { version, profile } = identity;
  const cli = profile.cli;
  const owner = join(cfg.work, 'owner.json');
  const buildEnv = { provider: PROVIDER_ID, root: cfg.root, graft: version, git: Boolean(cfg.git) };
  const genDir = join(cfg.work, 'gen');
  const hasState = existsSync(genDir) || existsSync(join(cfg.work, 'CURRENT'));
  if (hasState) {
    let mine = false;
    try { mine = JSON.stringify(JSON.parse(readFileSync(owner, 'utf8'))) === JSON.stringify(buildEnv); } catch { /* unowned or env changed */ }
    if (!mine) {
      if (!existsSync(owner)) throw new Refusal('refused', 'graft.index-not-owned', 'cache index exists without an adapter ownership marker; refusing to overwrite');
      rmSync(genDir, { recursive: true, force: true }); // ours, but built under a different environment
      rmSync(join(cfg.work, 'CURRENT'), { force: true });
    }
  }
  const t0 = Date.now();
  const work = { verification_ms: 0, index_ms: 0, files_verified: 0, bytes_hashed: 0, copied_bytes: 0 };
  const probe = async (dir) => {
    const v0 = Date.now();
    const c = await runGraft(cfg.upstream, ['check', cli.json, cli.dir, dir, '--', cfg.root], { work: cfg.work, signal, timeoutMs: left() });
    work.verification_ms += Date.now() - v0;
    try { return JSON.parse(c.stdout).graph; } catch { return null; }
  };
  const staleOf = async (cur) => {
    const g = await probe(cur.dir);
    if (!g) return { needs: true, changed: 0 };
    return g.ok && !g.missing ? { needs: false, changed: 0 } : { needs: true, changed: driftFiles(g) };
  };
  let cur = select();
  let files = cur ? readWiring(cur.dir) : null;
  let changed = 0;
  let needs = force || !files;
  if (!needs) { const s = await staleOf(cur); needs = s.needs; changed = s.changed; }
  const done = (dir, fl, action, outcome, extra = {}) => ({
    dir, files: fl, action, outcome, work, ms: Date.now() - t0, files_changed: extra.changed ?? changed, index_digest: indexDigest(fl), adoption: null, coalesced: Boolean(extra.coalesced),
  });
  if (!needs) return done(cur.dir, files, 'reuse', 'reused-owned-index');
  if (readOnly) {
    // refresh=never: report staleness instead of building.
    if (!files) throw new Refusal('unavailable', 'graft.index-missing', 'no index and refresh=never');
    return done(cur.dir, files, 'drift', 'reused-owned-index');
  }
  const pre = walkCandidates(cfg.root, cfg);
  if (pre.count > MAX_INDEX_FILES) throw new Refusal('refused', 'graft.project-too-large', `${pre.count} candidate files exceeds the indexing bound ${MAX_INDEX_FILES}`);
  mkdirSync(cfg.work, { recursive: true });
  return withRefreshLock(cfg.work, async ({ coalesced }) => {
    // Single flight: a process that waited re-reads CURRENT; if another process already published a fresh
    // generation, adopt it instead of rebuilding.
    if (coalesced && !force) {
      const now = select();
      const f = now ? readWiring(now.dir) : null;
      if (f) { const s = await staleOf(now); if (!s.needs) return done(now.dir, f, 'reuse', 'reused-owned-index', { coalesced: true, changed: 0 }); }
    }
    cur = select();
    files = cur ? readWiring(cur.dir) : null;
    const reuseCache = Boolean(files) && !force;
    writeFileSync(owner, JSON.stringify(buildEnv));
    const gen = beginGen(cfg.work, reuseCache ? cur : null);
    const b0 = Date.now();
    try {
      const b = await runGraft(cfg.upstream, ['build', ...(force ? [cli.noReuse] : []), cli.dir, gen.dir, '--', cfg.root], { work: cfg.work, signal, timeoutMs: left() });
      if (b.code !== 0) throw new RunError('build-failed', `graft build exited ${b.code}: ${firstLine(b.stderr)}`);
      const nf = readWiring(gen.dir);
      if (!nf) throw new RunError('build-failed', 'graft build produced an unreadable wiring index');
      work.index_ms = Date.now() - b0;
      for (const p of pins.splice(0)) p.release(); // the superseded generation was only a build seed; do not pin it against our own prune
      const pub = publish(cfg.work, gen);
      pin(pub); // still under the refresh lock, so no prune can have removed it
      return done(pub.dir, nf, reuseCache ? 'refresh' : 'build', reuseCache ? 'incremental-refresh' : 'rebuilt', { changed: reuseCache ? changed : nf.size, coalesced });
    } catch (e) { discardGen(gen); throw e; }
  }, { signal });
}

export const firstLine = (s) => (String(s).split('\n').find((l) => l.trim()) ?? '').slice(0, 300);

// Bounded walk mirroring graft's skip rules; reports candidate source files that the index lacks.
export function walkCandidates(root, cfg = null) {
  const tracked = cfg?.git ? gitCandidates(cfg) : null;
  if (tracked) return tracked;
  const found = [];
  let count = 0;
  let truncated = false;
  const stack = [''];
  while (stack.length && !truncated) {
    const rel = stack.pop();
    let entries;
    try { entries = readdirSync(join(root, rel), { withFileTypes: true }); } catch { continue; }
    entries.sort((a, b) => (a.name < b.name ? -1 : 1));
    for (const e of entries) {
      const p = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) { if (!e.name.startsWith('.') && !SKIP_DIRS.has(e.name)) stack.push(p); continue; }
      if (!e.isFile()) continue;
      const ext = extOf(e.name);
      if (!GRAFT_CODE_EXTENSIONS.has(ext) && !SEMAPRAX_EXTENSIONS.has(ext)) continue;
      if (++count > MAX_WALK_FILES) { truncated = true; break; }
      found.push(p);
    }
  }
  return { found: found.sort(), count, truncated };
}

// Tracked + untracked-but-not-ignored files (same view graft uses when git is on its PATH); null on failure.
function gitCandidates(cfg) {
  try {
    const out = execFileSync(cfg.git, ['-C', cfg.root, 'ls-files', '-co', '--exclude-standard', '-z'], {
      env: sanitizedEnv(cfg.work), timeout: 10000, maxBuffer: 32 * 1024 * 1024, stdio: ['ignore', 'pipe', 'ignore'],
    }).toString('utf8');
    const found = [];
    let skipped = 0;
    for (const p of out.split('\0')) {
      if (!p) continue;
      const parts = p.split('/');
      if (parts.slice(0, -1).some((d) => d.startsWith('.') || SKIP_DIRS.has(d))) continue;
      const ext = extOf(p);
      if (!GRAFT_CODE_EXTENSIONS.has(ext) && !SEMAPRAX_EXTENSIONS.has(ext)) continue;
      if (found.length >= MAX_WALK_FILES) { skipped++; continue; }
      found.push(p);
    }
    return { found: found.sort(), count: found.length + skipped, truncated: skipped > 0 };
  } catch { return null; }
}

export function coverageFor(cfg, indexed, profile = null) {
  const w = walkCandidates(cfg.root, cfg);
  const skipped = [];
  const parsed = profile ? new Set(profile.parsed_extensions) : null;
  for (const p of w.found) {
    if (indexed.has(p)) continue;
    skipped.push({
      path: p,
      reason: isSemaprax(p)
        ? 'unsupported: Semaprax source; semantic .spx/.spatch queries belong to the Semaprax compiler, not graft'
        : parsed && !parsed.has(extOf(p))
          ? `unsupported: the installed graft parser indexes no ${extOf(p)} files`
          : 'not indexed by graft (git-ignored, unparsable, or larger than 1 MB)',
    });
  }
  const listed = skipped.slice(0, MAX_SKIPPED_LISTED);
  const omitted = skipped.length - listed.length;
  return { skipped: listed, skipped_omitted: omitted, walk_truncated: w.truncated, indexed_files: indexed.size };
}

// Whole-file reads with a per-invocation cache; returns null when unreadable or outside the project.
export function makeFileReader(cfg) {
  const cache = new Map();
  return (rel) => {
    if (cache.has(rel)) return cache.get(rel);
    let v = null;
    try {
      const abs = join(cfg.root, rel);
      const real = realpathSync(abs);
      if (real === cfg.root || real.startsWith(cfg.root + sep)) {
        const st = statSync(real);
        if (st.isFile() && st.size <= 8 * 1024 * 1024) v = readFileSync(real);
      }
    } catch { v = null; }
    const out = v ? { buf: v, sha: sha256(v), lineStarts: lineStarts(v) } : null;
    cache.set(rel, out);
    return out;
  };
}

function lineStarts(buf) {
  const s = [0];
  for (let i = buf.indexOf(10); i >= 0; i = buf.indexOf(10, i + 1)) if (i + 1 < buf.length) s.push(i + 1);
  return s;
}

// sha256 of the exact bytes of lines [start,end] (1-based, inclusive), joined by LF with no trailing
// terminator: the convention the context broker re-hashes with.
export function spanDigest(file, start, end) {
  const n = file.lineStarts.length;
  const s = Math.min(Math.max(start, 1), n);
  const e = Math.min(Math.max(end, s), n);
  const from = file.lineStarts[s - 1];
  let to = e < n ? file.lineStarts[e] : file.buf.length;
  if (to > from && file.buf[to - 1] === 10) to -= 1;
  return { digest: `sha256:${sha256(file.buf.subarray(from, to))}`, start: s, end: e, lines: n };
}

export function sourceText(file, start, end, maxLines = 40, maxBytes = 2000) {
  const from = file.lineStarts[start - 1];
  const to = Math.min(end, start + maxLines - 1);
  const stop = to < file.lineStarts.length ? file.lineStarts[to] : file.buf.length;
  return file.buf.subarray(from, Math.min(stop, from + maxBytes)).toString('utf8');
}
