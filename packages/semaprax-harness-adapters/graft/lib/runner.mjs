// Sanitized, bounded, killable spawning of the upstream graft executable.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readlinkSync, renameSync, rmSync, symlinkSync } from 'node:fs';
import { join } from 'node:path';

export const MAX_STDOUT_BYTES = 4 * 1024 * 1024;
const MAX_STDERR_BYTES = 64 * 1024;

// Allowlist, never subtraction: nothing from the parent environment is inherited,
// so GRAFT_API_KEY / GRAFT_PROVIDER / GRAFT_MODEL / GRAFT_BASE_URL, any *_API_KEY or
// *_TOKEN, proxies and user config cannot reach graft.
export function sanitizedEnv(work) {
  return {
    PATH: join(work, 'bin'),
    HOME: join(work, 'home'),
    TMPDIR: join(work, 'tmp'),
    DO_NOT_TRACK: '1', // graft telemetry gate (src/telemetry/gate.ts honours it unconditionally)
    GRAFT_NO_REFRESH: '1', // queries never rebuild behind our back; the adapter owns refresh
    NODE_OPTIONS: '--max-old-space-size=2048', // bound indexing memory
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_OPTIONAL_LOCKS: '0', // git must not rewrite the user's .git/index
    LC_ALL: 'C',
  };
}

// PATH contains only `node` (graft's shebang) and optionally `git` (so graft honours
// .gitignore). No `npm`, so graft's background update check cannot reach the network.
export function prepareWork(work, nodePath, gitPath) {
  for (const d of ['bin', 'home', 'tmp', 'cwd']) mkdirSync(join(work, d), { recursive: true });
  for (const [name, target] of [['node', nodePath], ['git', gitPath]]) {
    const link = join(work, 'bin', name);
    // Idempotent and race-free: concurrent adapter processes share this directory, so never remove a live link.
    let cur = null;
    try { cur = readlinkSync(link); } catch { /* absent */ }
    if (!target) { if (cur !== null) rmSync(link, { force: true }); continue; }
    if (cur === target) continue;
    const tmp = `${link}.${process.pid}.tmp`;
    rmSync(tmp, { force: true });
    symlinkSync(target, tmp);
    renameSync(tmp, link);
  }
}

export function findGit(envGit) {
  if (envGit) return existsSync(envGit) ? envGit : null;
  return existsSync('/usr/bin/git') ? '/usr/bin/git' : null;
}

export class RunError extends Error {
  constructor(code, message, extra = {}) {
    super(message);
    this.code = code;
    Object.assign(this, extra);
  }
}

// Runs upstream with argv; resolves {code, stdout, stderr}; rejects RunError on
// timeout / abort / output overflow / spawn failure. Kills the whole process group.
export function runGraft(upstream, args, { work, signal, timeoutMs, maxBytes = MAX_STDOUT_BYTES }) {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(new RunError('cancelled', 'cancelled before start'));
    const child = spawn(upstream, args, {
      cwd: join(work, 'cwd'), // empty: graft loads dotenv from cwd
      env: sanitizedEnv(work),
      stdio: ['ignore', 'pipe', 'pipe'],
      detached: true,
    });
    let out = [];
    let err = [];
    let outLen = 0;
    let errLen = 0;
    let settled = false;
    const kill = () => {
      try { process.kill(-child.pid, 'SIGKILL'); } catch { try { child.kill('SIGKILL'); } catch { /* gone */ } }
    };
    const finish = (fn, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener('abort', onAbort);
      fn(value);
    };
    const onAbort = () => { kill(); finish(reject, new RunError('cancelled', 'cancelled')); };
    const timer = setTimeout(() => { kill(); finish(reject, new RunError('timeout', `graft exceeded ${timeoutMs} ms`)); }, timeoutMs);
    signal?.addEventListener('abort', onAbort, { once: true });
    child.stdout.on('data', (b) => {
      outLen += b.length;
      if (outLen > maxBytes) { kill(); finish(reject, new RunError('output-too-large', `graft output exceeded ${maxBytes} bytes`)); return; }
      out.push(b);
    });
    child.stderr.on('data', (b) => { if (errLen < MAX_STDERR_BYTES) { err.push(b); errLen += b.length; } });
    child.on('error', (e) => finish(reject, new RunError('spawn', `cannot start graft: ${e.code ?? e.message}`)));
    child.on('close', (code) => {
      kill(); // settle stragglers in the group
      finish(resolve, { code, stdout: Buffer.concat(out).toString('utf8'), stderr: Buffer.concat(err).toString('utf8') });
    });
  });
}
