// Single-flight refresh lock and atomic generation swap for the adapter-owned index.
// Layout under <work>: gen/<name>/ (immutable once published), CURRENT (name of the live generation,
// replaced by rename), refresh.lock/ (mkdir lock). Readers follow CURRENT and never see a partial build.
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const STALE_LOCK_MS = 10 * 60 * 1000;
const NAME_RE = /^g\d{1,9}$/;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export const genRoot = (work) => join(work, 'gen');

export function currentGen(work) {
  try {
    const name = readFileSync(join(work, 'CURRENT'), 'utf8').trim();
    if (!NAME_RE.test(name)) return null;
    const dir = join(genRoot(work), name);
    return existsSync(join(dir, '.graph', 'wiring.json')) ? { name, dir } : null;
  } catch { return null; }
}

function alive(pid) { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } }

// Runs fn while holding the cross-process lock. `coalesced` tells fn it waited, i.e. another process
// may have published a newer generation meanwhile and the work may already be done.
export async function withRefreshLock(work, fn, { signal, timeoutMs = 120000 } = {}) {
  const lock = join(work, 'refresh.lock');
  const start = Date.now();
  let waited = false;
  for (;;) {
    try { mkdirSync(lock); break; } catch (e) { if (e.code !== 'EEXIST') throw e; }
    let holder = null;
    try { holder = JSON.parse(readFileSync(join(lock, 'owner.json'), 'utf8')); } catch { /* being created */ }
    const age = holder ? Date.now() - holder.at : 0;
    if (holder && (!alive(holder.pid) || age > STALE_LOCK_MS)) { rmSync(lock, { recursive: true, force: true }); continue; }
    if (signal?.aborted) throw new Error('cancelled while waiting for the refresh lock');
    if (Date.now() - start > timeoutMs) throw new Error('timed out waiting for the refresh lock');
    waited = true;
    await sleep(40);
  }
  try {
    writeFileSync(join(lock, 'owner.json'), JSON.stringify({ pid: process.pid, at: Date.now() }));
    return await fn({ coalesced: waited });
  } finally { rmSync(lock, { recursive: true, force: true }); }
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

// Keeps the live generation and the one before it (a reader may still be mid-query on it).
function prune(work, live) {
  const n = Number(live.slice(1));
  for (const e of readdirSync(genRoot(work))) {
    const m = /^g(\d+)(\.partial)?$/.exec(e);
    if (m && Number(m[1]) < n - 1) rmSync(join(genRoot(work), e), { recursive: true, force: true });
  }
}

export function discardGen(gen) { rmSync(gen.dir, { recursive: true, force: true }); }
