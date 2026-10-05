// Single-flight refresh lock, generation leases and atomic generation swap for the adapter-owned index.
// Layout under <work>: gen/<name>/ (immutable once published), CURRENT (name of the live generation,
// replaced by rename), leases/ (one file per reader pinning a generation), refresh.lock[.N]/ (epoch locks).
// Readers follow CURRENT through acquireGen(), which pins the generation until release(); prune reclaims every
// generation that is neither current nor pinned by a live lease.
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { join } from 'node:path';

const STALE_LOCK_MS = 10 * 60 * 1000;
export const INIT_GRACE_MS = 15 * 1000; // an owner record must appear within this long after the lock directory
export const LEASE_TTL_MS = 15 * 60 * 1000; // upper bound on one query; a longer lease is treated as abandoned
const NAME_RE = /^g\d{1,9}$/;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export const genRoot = (work) => join(work, 'gen');
const leaseRoot = (work) => join(work, 'leases');

export function currentGen(work) {
  try {
    const name = readFileSync(join(work, 'CURRENT'), 'utf8').trim();
    if (!NAME_RE.test(name)) return null;
    const dir = join(genRoot(work), name);
    return existsSync(join(dir, '.graph', 'wiring.json')) ? { name, dir } : null;
  } catch { return null; }
}

function alive(pid) { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } }
const token = () => randomBytes(6).toString('hex');

// ---- reader leases -------------------------------------------------------------------------------------
// A lease is a small file naming the generation, the reader's pid and a start time, written atomically.
// It is live while the pid is alive and younger than LEASE_TTL_MS; crashed readers' leases are therefore
// ignored (and swept) rather than retaining generations forever.
function takeLease(work, name) {
  mkdirSync(leaseRoot(work), { recursive: true });
  const id = `${name}.${process.pid}.${token()}`;
  const file = join(leaseRoot(work), id);
  const tmp = `${file}.tmp`;
  writeFileSync(tmp, JSON.stringify({ gen: name, pid: process.pid, at: Date.now() }));
  renameSync(tmp, file);
  let released = false;
  return { release() { if (released) return; released = true; try { unlinkSync(file); } catch { /* already swept */ } } };
}

function liveLeasedGens(work) {
  const held = new Set();
  let names = [];
  try { names = readdirSync(leaseRoot(work)); } catch { return held; }
  for (const n of names) {
    if (n.endsWith('.tmp')) { try { if (Date.now() - statSync(join(leaseRoot(work), n)).mtimeMs > INIT_GRACE_MS) unlinkSync(join(leaseRoot(work), n)); } catch { /* raced */ } continue; }
    const f = join(leaseRoot(work), n);
    let l = null;
    try { l = JSON.parse(readFileSync(f, 'utf8')); } catch { /* vanished or torn */ }
    if (l && NAME_RE.test(l.gen) && Number.isInteger(l.pid) && alive(l.pid) && Date.now() - l.at < LEASE_TTL_MS) held.add(l.gen);
    else if (l || existsSync(f)) { try { unlinkSync(f); } catch { /* raced */ } }
  }
  return held;
}

// Pins `gen` ({name, dir}); null when it is already gone. The lease is written before the existence check and
// prune re-lists leases after marking a candidate, so one of the two always observes the other.
export function pinGen(work, gen, hooks = null) {
  const lease = takeLease(work, gen.name);
  hooks?.afterLease?.(gen);
  if (existsSync(join(gen.dir, '.graph', 'wiring.json')) && !existsSync(`${gen.dir}.reclaiming`)) return { ...gen, release: lease.release };
  lease.release();
  return null;
}

// Selects CURRENT and pins it for the caller's whole use; null when there is no usable generation.
// Each lost race means a newer generation was published meanwhile, so retries make progress; they are
// bounded by time (a stalled reader can lose several rounds against a fast writer), not by a small count.
export function acquireGen(work, { hooks = null, maxMs = 5000 } = {}) {
  const stop = Date.now() + maxMs;
  for (let i = 0; i < 8 || Date.now() < stop; i++) {
    const cur = currentGen(work);
    if (!cur) {
      // CURRENT named a generation that a newer publish already reclaimed: re-read. Only a missing CURRENT means none.
      if (existsSync(join(work, 'CURRENT'))) { Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5); continue; }
      return null;
    }
    hooks?.afterSelect?.(cur);
    const pinned = pinGen(work, cur, hooks);
    if (pinned) return pinned;
  }
  return null;
}

// Runs fn(gen) with the generation pinned; the lease is released on success, failure, timeout or abort.
export async function withGeneration(work, fn, opts) {
  const gen = acquireGen(work, opts);
  if (!gen) return fn(null);
  try { return await fn(gen); } finally { gen.release(); }
}

// ---- refresh lock --------------------------------------------------------------------------------------
// Epoch locks: the lock is the highest-numbered directory refresh.lock[.N] (the unnumbered name is epoch 0, the
// historical layout). A lock is free when its directory holds a `done` marker, written exclusively (wx) by the
// holder on release or by a recoverer that judged the lock abandoned; exactly one writer wins per epoch, and
// the next epoch is taken with an exclusive mkdir. Epoch numbers never repeat, so a recoverer can only ever
// retire the very lock it inspected, never a replacement that another waiter acquired meanwhile.
const lockPath = (work, e) => join(work, e === 0 ? 'refresh.lock' : `refresh.lock.${e}`);
function epochs(work) {
  const out = [];
  for (const n of readdirSync(work)) {
    if (n === 'refresh.lock') out.push(0);
    else { const m = /^refresh\.lock\.(\d{1,9})$/.exec(n); if (m) out.push(Number(m[1])); }
  }
  return out.sort((a, b) => a - b);
}
function readOwner(dir) {
  try { const o = JSON.parse(readFileSync(join(dir, 'owner.json'), 'utf8')); return Number.isInteger(o?.pid) && o.pid > 0 && Number.isFinite(o?.at) ? o : null; } catch { return null; }
}
// Age of a lock whose owner record is missing or malformed: the older of the directory and record mtimes.
function lockAge(dir) {
  let t = Infinity;
  for (const p of [dir, join(dir, 'owner.json')]) { try { t = Math.min(t, statSync(p).mtimeMs); } catch { /* absent */ } }
  return t === Infinity ? 0 : Date.now() - t;
}

// Runs fn while holding the cross-process lock. `coalesced` tells fn it waited, i.e. another process
// may have published a newer generation meanwhile and the work may already be done.
// An ownerless or malformed lock is recovered only after `initGraceMs`, so a live acquirer that is merely
// slow to write its owner record is never displaced.
export async function withRefreshLock(work, fn, { signal, timeoutMs = 120000, initGraceMs = INIT_GRACE_MS, hooks = null } = {}) {
  const start = Date.now();
  let waited = false;
  let mine = -1;
  for (;;) {
    const all = epochs(work);
    const cur = all.length ? all[all.length - 1] : -1;
    let free = cur < 0;
    let why = '';
    if (!free) {
      const dir = lockPath(work, cur);
      if (existsSync(join(dir, 'done'))) free = true;
      else {
        const holder = readOwner(dir);
        const stale = holder ? !alive(holder.pid) || Date.now() - holder.at > STALE_LOCK_MS : lockAge(dir) > initGraceMs;
        if (stale) {
          try { writeFileSync(join(dir, 'done'), JSON.stringify({ by: process.pid, recovered: true }), { flag: 'wx' }); free = true; } catch (e) {
            if (e.code === 'EEXIST' || e.code === 'ENOENT') continue; // someone else retired it first
            throw e;
          }
        } else why = holder ? `held by pid ${holder.pid}` : `owner record not yet written (${Math.round(lockAge(dir) / 1000)}s old, grace ${Math.round(initGraceMs / 1000)}s)`;
      }
    }
    if (free) {
      try { mkdirSync(lockPath(work, cur + 1)); mine = cur + 1; break; } catch (e) { if (e.code !== 'EEXIST') throw e; waited = true; continue; }
    }
    if (signal?.aborted) throw new Error('cancelled while waiting for the refresh lock');
    if (Date.now() - start > timeoutMs) throw new Error(`timed out waiting for the refresh lock (${why})`);
    waited = true;
    await sleep(40);
  }
  const dir = lockPath(work, mine);
  try {
    await hooks?.afterCreate?.(dir);
    const tmp = join(dir, `owner.json.${token()}.tmp`);
    writeFileSync(tmp, JSON.stringify({ pid: process.pid, at: Date.now() }));
    renameSync(tmp, join(dir, 'owner.json'));
    for (const e of epochs(work)) if (e < mine) rmSync(lockPath(work, e), { recursive: true, force: true }); // all retired
    return await fn({ coalesced: waited });
  } finally {
    // Retire (never delete) our epoch: the number must stay visible so it cannot be reused.
    try { writeFileSync(join(dir, 'done'), JSON.stringify({ by: process.pid }), { flag: 'wx' }); } catch { /* already retired by a recoverer */ }
  }
}

// Allocates gen/<next>.partial (seeded from `from` so a build can replay unchanged files) and returns
// {name, dir}; the caller builds into dir then calls publish(). Must hold the lock.
export function beginGen(work, from) {
  mkdirSync(genRoot(work), { recursive: true });
  const used = readdirSync(genRoot(work)).map((n) => /^g(\d+)/.exec(n)?.[1]).filter(Boolean).map(Number);
  const name = `g${Math.max(0, ...used) + 1}`;
  const dir = join(genRoot(work), `${name}.partial`);
  rmSync(dir, { recursive: true, force: true });
  if (from) cpSync(from.dir, dir, { recursive: true }); else mkdirSync(dir, { recursive: true });
  return { name, dir };
}

// Atomically makes the built generation current: rename partial -> final, then rename CURRENT.tmp -> CURRENT.
export function publish(work, gen) {
  const final = join(genRoot(work), gen.name);
  renameSync(gen.dir, final);
  const tmp = join(work, `CURRENT.${process.pid}.tmp`);
  writeFileSync(tmp, gen.name + '\n');
  renameSync(tmp, join(work, 'CURRENT'));
  prune(work, gen.name);
  return { name: gen.name, dir: final };
}

// Reclamation contract: after a publish, gen/ holds the live generation plus exactly the generations that a
// live reader lease pins. Everything else (older generations, leftover partial directories) is removed.
// A candidate is first marked (<name>.reclaiming, exclusive to the pruner, which holds the refresh lock), then
// the leases are re-read: a lease taken before the mark is seen and the candidate is kept; a lease taken after
// the mark makes the reader's own check (pinGen) fail and it selects again. Nothing in use is ever moved.
function prune(work, live) {
  const root = genRoot(work);
  const marker = (n) => join(root, `${n}.reclaiming`);
  for (const e of readdirSync(root)) if (e.endsWith('.reclaiming')) rmSync(join(root, e), { force: true }); // a crashed pruner's
  for (const e of readdirSync(root)) {
    const m = /^(g\d+)(\.partial)?$/.exec(e);
    if (!m || (m[1] === live && !m[2])) continue;
    const dir = join(root, e);
    if (m[2]) { rmSync(dir, { recursive: true, force: true }); continue; }
    if (liveLeasedGens(work).has(m[1])) continue;
    writeFileSync(marker(m[1]), '');
    if (!liveLeasedGens(work).has(m[1])) rmSync(dir, { recursive: true, force: true });
    rmSync(marker(m[1]), { force: true });
  }
}

export function discardGen(gen) { rmSync(gen.dir, { recursive: true, force: true }); }
