// Graft context.repository adapter speaking semaprax.harness-rpc.v1 over stdio.
// Host-provided env: SEMAPRAX_HARNESS_UPSTREAM, SEMAPRAX_HARNESS_PROJECT_ROOT, SEMAPRAX_HARNESS_CACHE_DIR
// (optional SEMAPRAX_HARNESS_GIT). Node standard library only.
import { createInterface } from 'node:readline';
import { PROTOCOL, result } from '../sdk/node/semaprax-harness-adapter.mjs';
import { OPERATIONS, invoke } from './lib/ops.mjs';
import { Refusal, loadConfig } from './lib/project.mjs';
import { RunError } from './lib/runner.mjs';

const ADAPTER_VERSION = '0.1.0';
const ACCEPTED = [{ kind: 'context.repository', version: 1, operations: OPERATIONS }];

function sortKeys(v) {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (v && typeof v === 'object') return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortKeys(v[k])]));
  return v;
}

export function main(env = process.env, input = process.stdin, output = process.stdout) {
  const send = (o) => output.write(JSON.stringify(sortKeys(o)) + '\n');
  const session = { identity: null };
  const running = new Map(); // invocation_id -> AbortController
  const cancelled = new Set();
  let queue = Promise.resolve(); // max_concurrency 1: invocations are serialized
  let closing = false;
  const provenance = () => ({ provider_id: 'org.nanonets/graft-context', adapter_version: ADAPTER_VERSION, upstream_version: session.identity?.version ?? 'unknown' });

  async function handleInvoke(id, req) {
    let envelope;
    const ac = new AbortController();
    running.set(req.invocation_id, ac);
    if (cancelled.has(req.invocation_id)) ac.abort();
    try {
      if (ac.signal.aborted) throw new RunError('cancelled', 'cancelled before start');
      const cfg = loadConfig(env);
      const [status, payload, diags] = await invoke(cfg, req, ac.signal, session);
      envelope = result(req, status, payload, provenance(), diags ?? []);
    } catch (e) {
      const [status, code] = e instanceof Refusal ? [e.status, e.code]
        : e instanceof RunError ? [e.code === 'cancelled' ? 'refused' : 'failed', `graft.${e.code}`]
          : ['failed', 'graft.internal'];
      envelope = result(req, status, null, provenance(), [{ code, message: String(e.message).slice(0, 500) }]);
    } finally {
      running.delete(req.invocation_id);
    }
    send({ jsonrpc: '2.0', id, result: envelope });
  }

  const rl = createInterface({ input, crlfDelay: Infinity });
  rl.on('line', (line) => {
    if (!line || closing) return;
    let msg;
    try { msg = JSON.parse(line); } catch { send({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'parse error' } }); return; }
    const { method, id } = msg;
    if (method === 'harness/initialize') {
      const params = msg.params ?? {};
      if (params.protocol !== PROTOCOL) { send({ jsonrpc: '2.0', id, error: { code: -32600, message: 'unsupported protocol' } }); return; }
      const offered = new Set((params.offered ?? []).map((c) => `${c.kind}@${c.version}`));
      send({ jsonrpc: '2.0', id, result: { protocol: PROTOCOL, accepted: ACCEPTED.filter((c) => offered.has(`${c.kind}@${c.version}`)) } });
    } else if (method === 'harness/cancel') {
      const inv = msg.params?.invocation_id;
      cancelled.add(inv);
      running.get(inv)?.abort(); // kills graft's process group
    } else if (method === 'harness/shutdown') {
      closing = true;
      for (const ac of running.values()) ac.abort();
      queue.finally(() => { send({ jsonrpc: '2.0', id, result: {} }); rl.close(); });
    } else if (method === 'harness/invoke') {
      queue = queue.then(() => handleInvoke(id, msg.params)).catch(() => {});
    } else {
      send({ jsonrpc: '2.0', id, error: { code: -32601, message: 'method not found' } });
    }
  });
  rl.on('close', () => { for (const ac of running.values()) ac.abort(); });
  return rl;
}

main();
