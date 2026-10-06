// Fits the COMPLETE serialized result envelope (what the host parses against the requested cap), not just
// `payload.items`. Items are kept first, then the skipped-file list; metadata, diagnostics and messages are
// bounded, every omission is counted, and coverage can only move toward incomplete. When even the smallest
// permitted envelope cannot fit, a bounded refusal replaces the oversized partial result.
import { result } from '../../sdk/node/semaprax-harness-adapter.mjs';

export const DEFAULT_BUDGET = 65536;
export const FRAME_CAP = 900 * 1024;
const MAX_DIAG_MESSAGE = 300;
const MAX_DIAGS = 12;

export function budgetOf(req) {
  const v = req?.budget?.max_result_bytes;
  return Math.min(Number.isInteger(v) && v > 0 ? v : DEFAULT_BUDGET, FRAME_CAP);
}

export const envelopeBytes = (req, prov, status, payload, diags) => Buffer.byteLength(JSON.stringify(result(req, status, payload, prov, diags)));

function boundDiags(diags) {
  const kept = diags.slice(0, MAX_DIAGS).map((d) => ({ ...d, message: String(d.message ?? '').slice(0, MAX_DIAG_MESSAGE) }));
  if (diags.length > MAX_DIAGS) kept.push({ code: 'graft.diagnostics-truncated', message: `${diags.length - MAX_DIAGS} further diagnostic(s) omitted` });
  return kept;
}

// Largest n in [lo, hi] with ok(n), assuming ok(lo) and monotone sizes.
function largest(lo, hi, ok) {
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (ok(mid)) lo = mid; else hi = mid - 1;
  }
  return lo;
}

export function fitResult(req, prov, status, payload, diags) {
  const limit = budgetOf(req);
  const fits = (s, p, d) => envelopeBytes(req, prov, s, p, d) <= limit;
  const base = boundDiags(diags ?? []);
  if (!payload || !Array.isArray(payload.items)) {
    if (fits(status, payload ?? null, base)) return [status, payload ?? null, base];
    return refuse();
  }
  const items = payload.items;
  const skipped = payload.coverage?.skipped ?? [];
  const meta = payload.metadata ?? {};
  const build = (ni, ns, slim) => {
    const dropItems = items.length - ni;
    const dropSkipped = skipped.length - ns;
    const coverage = { ...payload.coverage, skipped: skipped.slice(0, ns) };
    const metadata = { ...meta };
    if (slim) {
      if (metadata.refresh) metadata.refresh = { action: metadata.refresh.action, outcome: metadata.refresh.outcome };
      if (metadata.index_adoption) { delete metadata.index_adoption; metadata.index_adoption_omitted = true; }
    }
    const d = [...base];
    let st = status;
    if (dropSkipped) {
      coverage.complete = false;
      metadata.skipped_omitted = (meta.skipped_omitted ?? 0) + dropSkipped;
      d.push({ code: 'graft.skipped-truncated', message: `skipped list truncated to ${ns} of ${skipped.length + (meta.skipped_omitted ?? 0)} to fit the byte budget` });
    }
    if (dropItems) {
      coverage.complete = false;
      coverage.exhaustive = false;
      metadata.absence_proven = false;
      metadata.omitted_items = (meta.omitted_items ?? 0) + dropItems;
      if (!d.some((x) => x.code === 'graft.truncated')) d.push({ code: 'graft.truncated', message: 'result truncated; coverage.complete=false and absence is not proven' });
    }
    if ((dropItems || dropSkipped) && st === 'complete') st = 'partial';
    return [st, { ...payload, items: items.slice(0, ni), coverage, metadata }, d];
  };
  for (const slim of [false, true]) {
    const size = (ni, ns) => envelopeBytes(req, prov, ...build(ni, ns, slim)) <= limit;
    if (!size(0, 0)) continue;
    const ni = size(items.length, 0) ? items.length : largest(0, items.length - 1, (n) => size(n, 0));
    const ns = largest(0, skipped.length, (n) => size(ni, n));
    const out = build(ni, ns, slim);
    if (fits(...out)) return out;
  }
  return refuse();
}

function refuse() {
  const d = [{ code: 'graft.result-too-large', message: 'result exceeds the requested byte budget' }];
  return ['refused', null, d];
}
