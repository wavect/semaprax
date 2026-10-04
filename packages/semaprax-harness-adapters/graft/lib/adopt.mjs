// Opt-in adoption of a user-owned graft index (<project>/graft by default). Never trusted blindly:
// compatibility (wiring schema, extractor identity, code-only content) and source binding (file set and
// content digests against the working tree) are verified on every invocation before it answers a query.
// The user's index is never written: reuse is read-only, or an immutable copied snapshot.
import { createHash } from 'node:crypto';
import { chmodSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync } from 'node:fs';
import { join, sep } from 'node:path';
import { extOf, sha256, walkCandidates } from './project.mjs';

export const MODES = ['read-only', 'copied-snapshot'];
const MAX_WIRING_BYTES = 64 * 1024 * 1024;
const MAX_FILE_BYTES = 1_000_000; // graft indexes nothing larger
const MAX_REASONS = 8;

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
export function verifyUserIndex(cfg, profile, version, ac) {
  const t0 = Date.now();
  const reasons = [];
  const fail = (r) => { if (reasons.length < MAX_REASONS) reasons.push(r); };
  const out = () => ({ ok: false, reasons, descriptor: null, files: null, dir: null, work: { verification_ms: Date.now() - t0, files_verified: 0, bytes_hashed: 0 } });
  const dir = join(cfg.root, ac.rel);
  let st;
  try { st = lstatSync(dir); } catch { fail('no user index directory'); return out(); }
  if (!st.isDirectory() || st.isSymbolicLink()) { fail('user index is not a plain directory (symlinks are refused)'); return out(); }
  const real = realpathSync(dir);
  if (!real.startsWith(cfg.root + sep)) { fail('user index resolves outside the project root'); return out(); }
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

// Copies the verified index into an immutable snapshot keyed by its content digest; returns {dir, copied_bytes, reused}.
export function snapshotIndex(work, userDir) {
  const { digest } = treeDigest(userDir);
  const dest = join(work, 'adopted', digest.slice(0, 32));
  if (existsSync(join(dest, '.graph', 'wiring.json')) && treeDigest(dest).digest === digest) return { dir: dest, copied_bytes: 0, reused: true, digest };
  const tmp = `${dest}.partial-${process.pid}`;
  rmSync(tmp, { recursive: true, force: true });
  mkdirSync(join(work, 'adopted'), { recursive: true });
  cpSync(userDir, tmp, { recursive: true });
  rmSync(dest, { recursive: true, force: true });
  renameSync(tmp, dest);
  const sealed = (d) => { for (const e of readdirSync(d, { withFileTypes: true })) { const p = join(d, e.name); if (e.isDirectory()) sealed(p); else chmodSync(p, 0o444); } };
  sealed(dest);
  return { dir: dest, copied_bytes: treeDigest(dest).bytes, reused: false, digest };
}
