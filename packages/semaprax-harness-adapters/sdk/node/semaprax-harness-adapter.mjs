// Minimal adapter-side helper for semaprax.harness-rpc.v1 (docs/HARNESS-PROVIDER-V1.md).
// Node standard library only; same contract as sdk/python/semaprax_harness_adapter.py.
import { createInterface } from 'node:readline';

export const PROTOCOL = 'semaprax.harness-rpc.v1';
export const RESULT_SCHEMA = 'semaprax.harness-result.v1';
export const STATUSES = ['complete', 'partial', 'stale', 'unavailable', 'unsupported', 'refused', 'failed'];

export class AdapterError extends Error {
  constructor(status, code, message) {
    super(message);
    if (!STATUSES.includes(status)) throw new TypeError(status);
    this.status = status;
    this.code = code;
  }
}

function sortKeys(value) {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((k) => [k, sortKeys(value[k])]));
  }
  return value;
}

export function result(request, status, payload, provenance, diagnostics = []) {
  return {
    schema: RESULT_SCHEMA,
    invocation_id: request.invocation_id,
    project: request.project,
    capability: request.capability,
    status,
    payload,
    diagnostics,
    provenance,
  };
}

// handlers: Map key `${kind} ${operation}` -> async (request) => [status, payload, diagnostics]
export async function serve(accepted, handlers, provenance, input = process.stdin, output = process.stdout) {
  const send = (obj) => output.write(JSON.stringify(sortKeys(obj)) + '\n');
  const cancelled = new Set();
  const rl = createInterface({ input, crlfDelay: Infinity });
  for await (const line of rl) {
    if (!line) continue;
    const msg = JSON.parse(line);
    const { method, id } = msg;
    if (method === 'harness/initialize') {
      const params = msg.params ?? {};
      if (params.protocol !== PROTOCOL) {
        send({ jsonrpc: '2.0', id, error: { code: -32600, message: 'unsupported protocol' } });
        continue;
      }
      const offered = new Set((params.offered ?? []).map((c) => `${c.kind}@${c.version}`));
      send({ jsonrpc: '2.0', id, result: { protocol: PROTOCOL, accepted: accepted.filter((c) => offered.has(`${c.kind}@${c.version}`)) } });
    } else if (method === 'harness/cancel') {
      cancelled.add(msg.params?.invocation_id);
    } else if (method === 'harness/shutdown') {
      send({ jsonrpc: '2.0', id, result: {} });
      rl.close();
      return;
    } else if (method === 'harness/invoke') {
      const req = msg.params;
      let envelope;
      if (cancelled.has(req.invocation_id)) {
        envelope = result(req, 'refused', null, provenance, [{ code: 'cancelled', message: 'cancelled before start' }]);
      } else {
        const fn = handlers.get(`${req.capability.kind} ${req.operation}`);
        if (!fn) {
          envelope = result(req, 'unsupported', null, provenance, [{ code: 'unsupported', message: `operation ${req.operation} not implemented` }]);
        } else {
          try {
            const [status, payload, diags] = await fn(req);
            envelope = result(req, status, payload, provenance, diags ?? []);
          } catch (err) {
            if (!(err instanceof AdapterError)) throw err;
            envelope = result(req, err.status, null, provenance, [{ code: err.code, message: err.message }]);
          }
        }
      }
      send({ jsonrpc: '2.0', id, result: envelope });
    } else {
      send({ jsonrpc: '2.0', id, error: { code: -32601, message: 'method not found' } });
    }
  }
}
