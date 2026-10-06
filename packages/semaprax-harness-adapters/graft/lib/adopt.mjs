// Opt-in adoption of a user-owned graft index (<project>/graft by default). Never trusted blindly:
// compatibility (wiring schema, extractor identity, code-only content) and source binding (file set and
// content digests against the working tree) are verified on every invocation before it answers a query.
// The user's index is never written: reuse is read-only, or an immutable copied snapshot.
import { createHash, randomBytes } from 'node:crypto';
import { chmodSync, closeSync, constants, existsSync, fstatSync, lstatSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join, sep } from 'node:path';
import { extOf, sha256, walkCandidates } from './project.mjs';

export const MODES = ['read-only', 'copied-snapshot'];
const MAX_WIRING_BYTES = 64 * 1024 * 1024;
const MAX_FILE_BYTES = 1_000_000; // graft indexes nothing larger
const MAX_REASONS = 8;
export const MAX_SNAPSHOT_ATTEMPTS = 3; // staging retries while the user index is still being written
const MAX_SNAPSHOT_ENTRIES = 100000;
const MAX_SNAPSHOT_BYTES = 2 * 1024 * 1024 * 1024;

// Env contract (names are not SEMAPRAX_HARNESS_*, which the host reserves).
// Primary source: the host-validated descriptor config (`[capability."context.repository".config]`,
// forwarded as SEMAPRAX_HARNESS_CFG_<FIELD>); the older SEMAPRAX_GRAFT_* names remain aliases.
export function adoptionConfig(env) {
  const mode = env.SEMAPRAX_HARNESS_CFG_ADOPT_INDEX || env.SEMAPRAX_GRAFT_ADOPT_INDEX;
  if (!mode) return null;
  const rel = env.SEMAPRAX_HARNESS_CFG_USER_INDEX || env.SEMAPRAX_GRAFT_USER_INDEX || 'graft';
  if (!MODES.includes(mode) || rel.startsWith('/') || rel.split('/').includes('..') || rel.includes('\\')) return { invalid: true, mode, rel };
  return { mode, rel };
}

// Digest of every regular file (path + content) under dir, sorted; also total bytes.
export function treeDigest(dir) {
  const h = createHash('sha256');
  let bytes = 0;
  const walk = (rel) => {
    for (const e of readdirSync(join(dir, rel), { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const p = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) walk(p);
      else if (e.isFile()) { const b = readFileSync(join(dir, p)); bytes += b.length; h.update(`${p}\0${sha256(b)}\n`); }
    }
  };
  walk('');
  return { digest: h.digest('hex'), bytes };
}

// Verifies the user index; returns {ok, reasons[], descriptor, files, work}. Pure reads.
// `staged` verifies an already-copied snapshot directory instead of the user's index (same checks, same binding).
export function verifyUserIndex(cfg, profile, version, ac, staged = null) {
  const t0 = Date.now();
  const reasons = [];
  const fail = (r) => { if (reasons.length < MAX_REASONS) reasons.push(r); };
  const out = () => ({ ok: false, reasons, descriptor: null, files: null, dir: null, work: { verification_ms: Date.now() - t0, files_verified: 0, bytes_hashed: 0 } });
  const dir = staged ?? join(cfg.root, ac.rel);
  let st;
  try { st = lstatSync(dir); } catch { fail('no user index directory'); return out(); }
  if (!st.isDirectory() || st.isSymbolicLink()) { fail('user index is not a plain directory (symlinks are refused)'); return out(); }
  if (!staged) {
    const real = realpathSync(dir);
    if (!real.startsWith(cfg.root + sep)) { fail('user index resolves outside the project root'); return out(); }
  }
  const wf = join(dir, '.graph', 'wiring.json');
  if (!existsSync(wf) || statSync(wf).size > MAX_WIRING_BYTES) { fail('no readable wiring.json'); return out(); }
  let wiring;
  try { wiring = JSON.parse(readFileSync(wf, 'utf8')); } catch { fail('wiring.json is not valid JSON'); return out(); }
  if (wiring?.meta?.version !== profile.wiring_meta_version || !Array.isArray(wiring.nodes)) { fail(`wiring schema ${wiring?.meta?.version} is not the qualified ${profile.wiring_meta_version}`); return out(); }
  // Parser/config identity: the extractor id names the fingerprint file.
  const ids = existsSync(join(dir, '.cache')) ? readdirSync(join(dir, '.cache')).map((f) => /^fingerprint\.(.+)\.json$/.exec(f)?.[1]).filter(Boolean) : [];
  if (ids.length !== 1) fail(`expected one extractor fingerprint, found ${ids.length}`);
  else if (ids[0] !== profile.extractor) fail(`index extractor ${ids[0]} differs from graft ${version} extractor ${profile.extractor} (changed parser or configuration)`);
  // Code-only policy: LLM-produced summaries or concept nodes belong to another mode.
  if (wiring.nodes.some((n) => n.summary || (n.summary_state && n.summary_state !== 'pending'))) fail('index contains summaries from a richer (--deep) mode; local code-only policy refuses it');
  if (existsSync(join(dir, 'concepts')) || wiring.nodes.some((n) => n.kind === 'concept')) fail('index contains concept nodes from a richer (--deep) mode');
  // Source binding.
  const parsed = new Set(profile.parsed_extensions);
  const walk = walkCandidates(cfg.root, cfg);
  const candidates = new Set(walk.found.filter((p) => parsed.has(extOf(p))));
  const indexed = new Map();
  for (const n of wiring.nodes) if (n.kind === 'file' && typeof n.path === 'string') indexed.set(n.path, n.body_hash);
  let files = 0; let bytes = 0;
  for (const [p, want] of indexed) {
    if (!candidates.has(p)) { fail(`indexed path ${p} is excluded, ignored or absent in this working tree`); continue; }
    let buf;
    try { buf = readFileSync(join(cfg.root, p)); } catch { fail(`indexed path ${p} is unreadable`); continue; }
    files++; bytes += buf.length;
    if (sha256(buf) !== want) fail(`indexed path ${p} differs from its indexed content`);
  }
  for (const p of candidates) {
    if (indexed.has(p)) continue;
    let size = 0; try { size = statSync(join(cfg.root, p)).size; } catch { /* gone */ }
    if (size <= MAX_FILE_BYTES) fail(`source file ${p} is not in the index`);
  }
  const descriptor = {
    schema: 'semaprax.harness-index-adoption.v1', provider: 'org.nanonets/graft-context', upstream_version: version, index_schema: `wiring.v${wiring.meta.version}`,
    source_root: cfg.root, index_dir: ac.rel, config: { extractor: ids[0] ?? null, code_only: true }, coverage: { languages: wiring.meta.languages ?? [], files: indexed.size },
    inputs_digest: createHash('sha256').update([...indexed].sort().map(([p, h]) => `${p}\0${h}`).join('\n')).digest('hex'), ownership: ac.mode,
  };
  return { ok: reasons.length === 0, reasons, descriptor, files: new Map([...indexed].filter(([, h]) => /^[0-9a-f]{64}$/.test(h ?? ''))), dir, work: { verification_ms: Date.now() - t0, files_verified: files, bytes_hashed: bytes } };
}


export class SnapshotRefusal extends Error {}

// Explicit recursive inventory with lstat at every depth: only directories and regular files are admitted.
// Symlinks (file, directory, dangling) and special files are refused with a bounded message; nothing is followed.
// Returns {dirs[], files[{rel,size}], digest, bytes}; the digest has the treeDigest format (path + content hash).
export function inventoryTree(dir) {
  const dirs = []; const files = [];
  const walk = (rel) => {
    for (const e of readdirSync(join(dir, rel)).sort((a, b) => (a < b ? -1 : 1))) {
      const p = rel ? `${rel}/${e}` : e;
      if (dirs.length + files.length >= MAX_SNAPSHOT_ENTRIES) throw new SnapshotRefusal(`index has more than ${MAX_SNAPSHOT_ENTRIES} entries`);
      const st = lstatSync(join(dir, p));
      if (st.isSymbolicLink()) throw new SnapshotRefusal(`index contains a symlink at ${p.slice(0, 200)} (symlinks are refused)`);
      if (st.isDirectory()) { dirs.push(p); walk(p); } else if (st.isFile()) files.push({ rel: p, size: st.size });
      else throw new SnapshotRefusal(`index contains a special file at ${p.slice(0, 200)} (only regular files and directories are admitted)`);
    }
  };
  walk('');
  files.sort((a, b) => (a.rel < b.rel ? -1 : 1));
  const h = createHash('sha256'); let bytes = 0;
  for (const f of files) { const b = readFileSync(join(dir, f.rel)); bytes += b.length; h.update(`${f.rel}\0${sha256(b)}\n`); }
  return { dirs, files, digest: h.digest('hex'), bytes };
}

// Copies exactly the inventoried files into a fresh private directory. Each source is opened without following
// links and must be a regular file on the open descriptor; the user's tree is never written or chmod-ed.
function stageCopy(userDir, inv, tmp, hooks) {
  mkdirSync(tmp, { recursive: true });
  for (const d of inv.dirs) mkdirSync(join(tmp, d));
  let total = 0;
  for (const f of inv.files) {
    const src = join(userDir, f.rel);
    hooks?.beforeFile?.(f.rel, src);
    let fd;
    try { fd = openSync(src, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0)); } catch (e) { throw new SnapshotRefusal(`index file ${f.rel.slice(0, 200)} is no longer a regular file (${e.code ?? 'unreadable'})`); }
    let buf;
    try {
      const st = fstatSync(fd);
      if (!st.isFile()) throw new SnapshotRefusal(`index file ${f.rel.slice(0, 200)} is no longer a regular file`);
      total += st.size;
      if (total > MAX_SNAPSHOT_BYTES) throw new SnapshotRefusal('index exceeds the snapshot size bound');
      buf = readFileSync(fd);
    } finally { closeSync(fd); }
    writeFileSync(join(tmp, f.rel), buf, { flag: 'wx', mode: 0o644 });
    hooks?.afterFile?.(f.rel);
  }
}

// Seals only independent regular files inside the staged (owned) directory, after its final inventory check.
function sealStaged(tmp, inv) { for (const f of inv.files) chmodSync(join(tmp, f.rel), 0o444); }

// An existing destination is trusted only after its full inventory and digest equal the admitted identity.
function destIsIntact(dest, digest) {
  try {
    if (!lstatSync(dest).isDirectory()) return false;
    const inv = inventoryTree(dest);
    return inv.digest === digest && inv.files.some((f) => f.rel === '.graph/wiring.json');
  } catch { return false; }
}

// Publishes the staged directory as adopted/<digest>. Never deletes a last-good destination before the new
// one is validated; a corrupt destination is moved aside, replaced, then removed.
function publishStaged(adoptedRoot, tmp, dest, digest) {
  if (destIsIntact(dest, digest)) { rmSync(tmp, { recursive: true, force: true }); return { reused: true }; }
  const aside = `${dest}.old-${process.pid}-${randomBytes(4).toString('hex')}`;
  let moved = false;
  try { renameSync(dest, aside); moved = true; } catch (e) { if (e.code !== 'ENOENT') throw e; }
  try { renameSync(tmp, dest); } catch (e) {
    if (moved) { try { renameSync(aside, dest); } catch { /* keep the aside copy */ } }
    // Another publisher won the race: accept only if its destination verifies against the same identity.
    if (destIsIntact(dest, digest)) { rmSync(tmp, { recursive: true, force: true }); return { reused: true }; }
    throw e;
  }
  if (moved) rmSync(aside, { recursive: true, force: true });
  return { reused: false };
}

// Adopts the user index as an immutable, owned, validated snapshot. One staged copy is validated as staged
// (inventory, bytes, schema, extractor, source binding); its identity is computed from those bytes; only then it
// is published. A moving source is retried a bounded number of times. Returns the verifyUserIndex shape with
// {dir: published snapshot, snapshotDigest, copied_bytes, reused}, or {ok:false, reasons}.
export function adoptCopiedSnapshot(cfg, profile, version, ac) {
  const hooks = cfg.adoptHooks ?? null;
  const t0 = Date.now();
  const adoptedRoot = join(cfg.work, 'adopted');
  for (let attempt = 1; attempt <= MAX_SNAPSHOT_ATTEMPTS; attempt++) {
    const v0 = verifyUserIndex(cfg, profile, version, ac);
    if (!v0.ok) return v0;
    const refuse = (e) => ({ ...v0, ok: false, reasons: [e.message.slice(0, 400)], descriptor: null, files: null, dir: null });
    let pre;
    try { pre = inventoryTree(v0.dir); } catch (e) { if (e instanceof SnapshotRefusal) return refuse(e); throw e; }
    hooks?.afterDigest?.(pre.digest);
    mkdirSync(adoptedRoot, { recursive: true });
    const tmp = join(adoptedRoot, `${pre.digest.slice(0, 32)}.partial-${process.pid}-${randomBytes(4).toString('hex')}`);
    try {
      let staged; let post;
      try {
        stageCopy(v0.dir, pre, tmp, hooks);
        hooks?.afterCopy?.(tmp);
        staged = inventoryTree(tmp); // final validation of the owned copy: every entry is a regular file or directory
        post = inventoryTree(v0.dir); // and of the source: a file that became a link while staging fails here
      } catch (e) { if (e instanceof SnapshotRefusal) return refuse(e); throw e; }
      if (staged.digest !== pre.digest || post.digest !== staged.digest) continue; // the source moved while copying
      const v = verifyUserIndex(cfg, profile, version, ac, tmp);
      if (!v.ok) return { ...v, dir: null };
      sealStaged(tmp, staged);
      const dest = join(adoptedRoot, staged.digest.slice(0, 32));
      const pub = publishStaged(adoptedRoot, tmp, dest, staged.digest);
      return { ...v, dir: dest, snapshotDigest: staged.digest, copied_bytes: pub.reused ? 0 : staged.bytes, reused: pub.reused, work: { ...v.work, verification_ms: Date.now() - t0 } };
    } finally { rmSync(tmp, { recursive: true, force: true }); }
  }
  return { ok: false, reasons: [`the user index kept changing while it was copied (${MAX_SNAPSHOT_ATTEMPTS} attempts); falling back to an owned build`], descriptor: null, files: null, dir: null, work: { verification_ms: Date.now() - t0, files_verified: 0, bytes_hashed: 0 } };
}
