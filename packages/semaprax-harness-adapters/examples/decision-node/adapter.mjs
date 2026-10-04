// decision.evaluate/v1 adapter for task model-route/v1.
// Deterministic rule: score = clamp(complexity, 0, 1); score >= threshold picks the
// strongest option, otherwise the cheapest. Abstains when the feature is missing.
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const { serve } = await import(join(here, '..', '..', 'sdk', 'node', 'semaprax-harness-adapter.mjs'));

const THRESHOLD = 0.5;

export function decide(payload) {
  const options = Array.isArray(payload?.options) ? payload.options : [];
  const features = payload?.features ?? {};
  const abstain = { choice: null, scores: {}, abstain: true };
  if (payload?.task !== 'model-route/v1' || options.length === 0) return abstain;
  const c = features.complexity;
  if (typeof c !== 'number' || !Number.isFinite(c)) return abstain;
  const score = Math.min(1, Math.max(0, c));
  // Options are ordered cheapest -> strongest by the host's task contract.
  const choice = score >= THRESHOLD ? options[options.length - 1] : options[0];
  const scores = Object.fromEntries(options.map((o) => [o, o === choice ? score : 0]));
  return { choice, scores, abstain: false };
}

const handlers = new Map([['decision.evaluate evaluate', async (req) => ['complete', decide(req.payload), []]]]);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await serve(
    [{ kind: 'decision.evaluate', version: 1, operations: ['evaluate'] }],
    handlers,
    { provider_id: 'org.example/threshold-route', adapter_version: '0.1.0', upstream_version: 'builtin-0.1.0' },
  );
}
