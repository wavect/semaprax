// context.repository v1 operations over graft's JSON output.
import { runGraft, RunError } from './runner.mjs';
import { treeDigest } from './adopt.mjs';
import {
  Refusal, coverageFor, ensureFresh, firstLine, isSemaprax, languageOf, makeFileReader, probeIdentity, safeRel, sourceText, spanDigest,
} from './project.mjs';

const KIND = 'context.repository';
const DEFAULT_BUDGET = 65536;
const FRAME_CAP = 900 * 1024;
const SYMBOL_RE = /^[A-Za-z_$][\w$.:-]{0,199}$/;

function parseJson(r, what) {
  try { return JSON.parse(r.stdout); } catch { throw new RunError('bad-output', `graft ${what} produced non-JSON output`); }
}

// Parses graft pointers "path:L1-L3", "path:L5" or bare "path".
function parsePointer(p) {
  const m = /^(.*):L(\d+)(?:-L(\d+))?$/.exec(p);
  if (!m) return { path: p, start: null, end: null };
  const s = Number(m[2]);
  return { path: m[1], start: s, end: m[3] ? Number(m[3]) : s };
}
function parseSpan(span) {
  const m = /^L(\d+)(?:-L(\d+))?$/.exec(span ?? '');
  if (!m) return { start: null, end: null };
  return { start: Number(m[1]), end: m[2] ? Number(m[2]) : Number(m[1]) };
}

export class Context {
  constructor(cfg, req, signal, state) {
    this.cfg = cfg; this.req = req; this.signal = signal; this.state = state; this.profile = state.profile; this.cli = state.profile.cli;
    this.deadline = Date.now() + Math.min(Math.max(req.deadline_ms ?? 30000, 1000), 300000);
    this.budget = Math.min(req.budget?.max_result_bytes ?? DEFAULT_BUDGET, FRAME_CAP);
    this.reader = makeFileReader(cfg);
    this.stale = new Set();
    this.diags = [];
  }
  // Runs a read-only graft query against the private index.
  async query(args) {
    const left = Math.max(500, this.deadline - Date.now());
    // args = [command, ...options, '--', ...positionals]; everything after `--` is positional, so the
    // repo root is appended there and caller-supplied text can never be parsed as an option.
    const i = args.indexOf('--');
    const head = i < 0 ? args : args.slice(0, i);
    const pos = i < 0 ? [] : args.slice(i + 1);
    const c = this.profile.cli;
    return runGraft(this.cfg.upstream, [...head, c.noRefresh, c.dir, this.state.index.dir, '--', ...pos, this.cfg.root], {
      work: this.cfg.work, signal: this.signal, timeoutMs: left,
    });
  }
  // Builds one payload item or null when the file is unreadable / outside the project.
  item({ path, start, end, provenance, rank, text }) {
    const rel = safeRel(path);
    if (!rel) return null;
    const file = this.reader(rel);
    if (!file) return null;
    const known = this.state.index.files.get(rel);
    if (known && known !== file.sha) this.stale.add(rel); // file changed since the index was built
    const d = spanDigest(file, start ?? 1, end ?? file.lineStarts.length);
    const it = { path: rel, span: { start_line: d.start, end_line: d.end }, digest: d.digest, provenance, language: languageOf(rel), rank };
    if (text) it.text = text.slice(0, 2000);
    return it;
  }
}

function pathPrefix(payload) {
  // Contract member `in`; `path_prefix` is the adapter-internal spelling.
  const raw = payload.in ?? payload.path_prefix;
  if (raw === undefined) return null;
  const p = safeRel(raw);
  if (!p) throw new Refusal('refused', 'graft.bad-path', '`in` must be a relative path inside the project');
  return p;
}
function limitOf(payload, dflt, max = 100) {
  // Contract member `max_items`; `limit` is the adapter-internal spelling.
  const n = payload.max_items ?? payload.limit ?? dflt;
  if (!Number.isInteger(n) || n < 1) throw new Refusal('refused', 'graft.bad-payload', 'max_items must be a positive integer');
  return Math.min(n, max);
}
function stringArg(payload, name, max = 500) {
  const v = payload[name];
  if (typeof v !== 'string' || !v.trim() || v.length > max || v.includes('\0')) throw new Refusal('refused', 'graft.bad-payload', `${name} must be a non-empty string up to ${max} chars`);
  return v;
}

// -- operations: each returns {items, truncated, exhaustive, diags, extra} ---------------

async function orient(ctx, payload) {
  const maxDirs = limitOf({ max_items: payload.max_items ?? payload.max_dirs }, 16, 64);
  const r = await ctx.query(['map', ctx.cli.json, ctx.cli.maxDirs, String(maxDirs)]);
  if (r.code !== 0) throw new RunError('query-failed', `graft map exited ${r.code}: ${firstLine(r.stderr)}`);
  const m = parseJson(r, 'map');
  const items = [];
  const seen = new Set();
  const add = (h, label) => {
    const { start, end } = parseSpan(h.span);
    const key = `${h.path}:${start}`;
    if (seen.has(key)) return;
    seen.add(key);
    const it = ctx.item({ path: h.path, start, end, provenance: 'structural', rank: items.length + 1, text: `${label}: ${h.name} (${h.kind}, in-degree ${h.inDegree})` });
    if (it) items.push(it);
  };
  for (const h of m.hotspots ?? []) add(h, 'hotspot');
  for (const d of m.dirs ?? []) for (const h of d.hubs ?? []) add(h, `hub of ${d.path}`);
  return { items, truncated: (m.dropped ?? 0) > 0, exhaustive: false, extra: { orientation: m.totals ?? null, dirs_dropped: m.dropped ?? 0 } };
}

async function search(ctx, payload) {
  const query = stringArg(payload, 'query');
  const mode = payload.mode ?? 'ranked';
  if (!['ranked', 'exact'].includes(mode)) throw new Refusal('refused', 'graft.bad-payload', 'mode must be "ranked" or "exact"');
  const prefix = pathPrefix(payload);
  const limit = limitOf(payload, 8, mode === 'exact' ? 200 : 50);
  const withSource = payload.include_source === true;
  if (mode === 'ranked') {
    const r = await ctx.query(['ask', ctx.cli.json, ctx.cli.limit, String(limit), ...(prefix ? [ctx.cli.in, prefix] : []), '--', query]);
    if (r.code !== 0) throw new RunError('query-failed', `graft ask exited ${r.code}: ${firstLine(r.stderr)}`);
    const a = parseJson(r, 'ask');
    const items = [];
    for (const h of a.hits ?? []) {
      if (typeof h.pointer !== 'string') continue;
      const p = parsePointer(h.pointer);
      const it = ctx.item({ path: p.path, start: p.start, end: p.end, provenance: 'structural', rank: items.length + 1, text: [h.title, h.snippet].filter(Boolean).join('\n') });
      if (!it) continue;
      if (withSource) it.text = sourceText(ctx.reader(it.path), it.span.start_line, it.span.end_line);
      items.push(it);
    }
    // Ranked retrieval is a top-N: never exhaustive.
    return { items, truncated: false, exhaustive: false, extra: { mode: 'ranked', retrieval: a.mode ?? null } };
  }
  return grepItems(ctx, query, prefix, limit, 'exact');
}

// Literal text occurrences over the indexed files (graft grep --fixed); lexical, so provenance is "inferred".
async function grepItems(ctx, literal, prefix, limit, mode) {
  const r = await ctx.query(['grep', ctx.cli.json, ctx.cli.fixed, ...(prefix ? [ctx.cli.in, prefix] : []), '--', literal]);
  if (r.code !== 0) throw new RunError('query-failed', `graft grep exited ${r.code}: ${firstLine(r.stderr)}`);
  const g = parseJson(r, 'grep');
  const items = [];
  let dropped = 0;
  for (const grp of g.groups ?? []) {
    for (const h of grp.hits ?? []) {
      if (items.length >= limit) { dropped++; continue; }
      const it = ctx.item({ path: grp.path, start: h.line, end: h.line, provenance: 'inferred', rank: items.length + 1, text: h.text });
      if (it) items.push(it);
    }
  }
  const upstreamTruncated = (g.truncated?.files ?? 0) + (g.truncated?.hits ?? 0) > 0;
  const truncated = upstreamTruncated || dropped > 0;
  return { items, truncated, exhaustive: !truncated, extra: { mode, files_searched: g.filesSearched ?? null, total_hits: g.totalHits ?? null } };
}

async function skeleton(ctx, payload) {
  const rel = safeRel(payload.path);
  if (!rel) throw new Refusal('refused', 'graft.bad-path', 'path must be a relative path inside the project');
  const r = await ctx.query(['skeleton', ctx.cli.json, '--', rel]);
  if (r.code !== 0) {
    return { items: [], truncated: false, exhaustive: false, diags: [{ code: 'graft.file-not-indexed', message: firstLine(r.stderr) || 'file not in graft index' }], extra: {} };
  }
  const s = parseJson(r, 'skeleton');
  const items = [];
  for (const e of s.entries ?? []) {
    const { start, end } = parseSpan(e.span);
    const it = ctx.item({ path: s.file ?? rel, start, end, provenance: 'structural', rank: items.length + 1, text: e.signature ?? e.name });
    if (it) items.push(it);
  }
  const diags = items.length ? [] : [{ code: 'graft.file-not-indexed', message: s.note ?? 'no definitions indexed for this file' }];
  return { items, truncated: false, exhaustive: false, diags, extra: { file: s.file ?? rel } };
}

async function references(ctx, payload) {
  const symbol = stringArg(payload, 'symbol', 200);
  if (!SYMBOL_RE.test(symbol)) throw new Refusal('refused', 'graft.bad-payload', 'symbol must be an identifier (optionally qualified)');
  const direction = payload.direction ?? 'in';
  if (!['in', 'out'].includes(direction)) throw new Refusal('refused', 'graft.bad-payload', 'direction must be "in" or "out"');
  const depth = payload.depth ?? 1;
  if (!Number.isInteger(depth) || depth < 1 || depth > 5) throw new Refusal('refused', 'graft.bad-payload', 'depth must be 1..5');
  const prefix = pathPrefix(payload);
  const limit = limitOf(payload, 50, 200);
  const r = await ctx.query(['callers', ctx.cli.json, ctx.cli.direction, direction, ctx.cli.depth, String(depth), ...(prefix ? [ctx.cli.in, prefix] : []), '--', symbol]);
  const diags = [];
  const items = [];
  let known = true;
  if (r.code !== 0) {
    if (!/no symbol/i.test(r.stderr)) throw new RunError('query-failed', `graft callers exited ${r.code}: ${firstLine(r.stderr)}`);
    known = false;
    diags.push({ code: 'graft.symbol-not-found', message: `no symbol "${symbol}" in the graft index; this does not prove it has no references` });
  } else {
    const c = parseJson(r, 'callers');
    for (const m of c.matches ?? []) {
      for (const h of m.hits ?? []) {
        if (items.length >= limit) { diags.length || diags.push({ code: 'graft.truncated', message: `reference list capped at ${limit}` }); continue; }
        const { start, end } = parseSpan(h.span);
        const it = ctx.item({ path: h.path, start, end, provenance: 'structural', rank: items.length + 1, text: `${h.relation} ${direction === 'in' ? 'by' : 'of'} ${h.name} (${h.kind}, depth ${h.depth})` });
        if (it) items.push(it);
      }
    }
  }
  let truncated = diags.some((d) => d.code === 'graft.truncated');
  // Graft call edges are static name-resolution wiring and miss dynamic dispatch, so edges alone are never exhaustive.
  let exhaustive = false;
  if (payload.exhaustive === true && known) {
    // Opt-in exhaustive textual scan of the indexed files for the bare name.
    const bare = symbol.split(/[.:]/).filter(Boolean).pop();
    const g = await grepItems(ctx, bare, prefix, 2000, 'exhaustive-text');
    const have = new Set(items.map((i) => `${i.path}:${i.span.start_line}`));
    for (const it of g.items) {
      if (have.has(`${it.path}:${it.span.start_line}`)) continue;
      it.rank = items.length + 1;
      items.push(it);
    }
    truncated = truncated || g.truncated;
    exhaustive = g.exhaustive && !truncated;
  }
  return { items, truncated, exhaustive, diags, extra: { symbol, direction, depth, symbol_known: known } };
}

// Host metadata allows only scalars or one level of scalar objects.
function flatAdoption(a) {
  const reasons = (a.reasons ?? []).join('; ').slice(0, 900);
  if (a.outcome === 'incompatible') return { outcome: 'incompatible', reasons, reasons_count: (a.reasons ?? []).length, fallback: a.fallback ?? null };
  return {
    schema: a.schema, provider: a.provider, upstream_version: a.upstream_version, index_schema: a.index_schema, ownership: a.ownership, mode: a.mode,
    extractor: a.config?.extractor ?? null, languages: (a.coverage?.languages ?? []).join(','), files: a.coverage?.files ?? 0, inputs_digest: a.inputs_digest, source_root: a.source_root,
  };
}

const OPS = { orient, search, skeleton, references };
export const OPERATIONS = Object.keys(OPS);

// Shrinks items until the serialized payload fits the byte budget.
function fit(payload, budget) {
  let truncated = false;
  while (Buffer.byteLength(JSON.stringify(payload)) > budget && payload.items.length) {
    payload.items.length = Math.max(0, Math.floor(payload.items.length / 2));
    truncated = true;
  }
  return truncated;
}

export async function invoke(cfg, req, signal, session) {
  if (req.capability?.kind !== KIND || req.capability?.version !== 1 || !OPS[req.operation]) {
    return ['unsupported', null, [{ code: 'graft.unsupported-operation', message: `operation ${req.operation} not implemented` }]];
  }
  const payload = req.payload ?? {};
  if (typeof payload !== 'object' || Array.isArray(payload)) throw new Refusal('refused', 'graft.bad-payload', 'payload must be an object');
  const deadline = Date.now() + Math.min(Math.max(req.deadline_ms ?? 30000, 1000), 300000);
  const asked = req.operation === 'skeleton' ? safeRel(payload.path) : null;
  if (asked && isSemaprax(asked)) {
    // Semantic .spx queries belong to the Semaprax compiler: never sent to graft.
    const reason = 'unsupported: Semaprax source; semantic .spx/.spatch queries belong to the Semaprax compiler, not graft';
    return ['unsupported', {
      items: [],
      coverage: { complete: false, exhaustive: false, indexed_files: 0, skipped: [{ path: asked, reason }] },
      metadata: { absence_proven: false },
    }, [{ code: 'graft.semaprax-source', message: reason }]];
  }
  session.identity ??= await probeIdentity(cfg, signal);
  const identity = session.identity;
  const refresh = payload.refresh ?? 'auto';
  if (!['auto', 'rebuild', 'never'].includes(refresh)) throw new Refusal('refused', 'graft.bad-payload', 'refresh must be auto, rebuild or never');
  let index = await ensureFresh(cfg, identity, { signal, deadline, force: refresh === 'rebuild', readOnly: refresh === 'never' });
  // The index generation stays pinned from selection through the last read of this query.
  try {
    if (index.action === 'drift') return ['stale', null, [{ code: 'graft.index-stale', message: 'index is behind the working tree and refresh=never' }]];
    let result;
    for (let attempt = 0; attempt < 2; attempt++) {
      const ctx = new Context(cfg, req, signal, { index, profile: identity.profile });
      result = { ctx, ...(await OPS[req.operation](ctx, payload)) };
      if (!ctx.stale.size) break;
      if (attempt === 0 && refresh !== 'never') {
        // Content changed under an index graft's own check considered fresh (e.g. mtime preserved), or an
        // adopted index went stale mid-call: force an owned rebuild.
        const again = await ensureFresh(cfg, identity, { signal, deadline, force: true });
        again.ms += index.ms; again.action = 'refresh'; again.files_changed = Math.max(again.files_changed, ctx.stale.size);
        again.adoption = index.adoption ? { outcome: 'incompatible', reasons: ['index changed under the query'], fallback: again.outcome } : again.adoption;
        index.release?.();
        index = again;
      }
    }
    const { ctx } = result;
    if (ctx.stale.size) {
      return ['stale', null, [{ code: 'graft.index-stale', message: `index disagrees with ${ctx.stale.size} file(s) on disk after refresh: ${[...ctx.stale].slice(0, 3).join(', ')}` }]];
    }
    const diags0 = [];
    if (index.userDir && index.userIndexDigest && treeDigest(index.userDir).digest !== index.userIndexDigest) {
      diags0.push({ code: 'graft.user-index-modified', message: 'the user index changed while it was being queried; its answers are not used as evidence' });
      return ['failed', null, diags0];
    }
    const cov = coverageFor(cfg, index.files, identity.profile);
    const diags = [...(result.diags ?? [])];
    if (index.adoption?.outcome === 'incompatible') diags.push({ code: 'graft.index-incompatible', message: `user index not adopted (${index.adoption.reasons.slice(0, 3).join('; ')}); served from ${index.adoption.fallback}` });
    if (!cfg.git) diags.push({ code: 'graft.gitignore-unavailable', message: 'git not available to graft; .gitignore is not honoured and ignored files may be indexed' });
    if (cov.skipped.length) diags.push({ code: 'graft.coverage-incomplete', message: `${cov.skipped.length + cov.skipped_omitted} source file(s) are not covered by graft` });
    const payloadOut = {
      items: result.items,
      coverage: {
        complete: false, exhaustive: false, indexed_files: cov.indexed_files, skipped: cov.skipped,
      },
      metadata: {
        ...result.extra,
        refresh: {
          action: index.action, outcome: index.adoption?.outcome === 'incompatible' ? 'incompatible' : index.outcome, served_by: index.outcome, ms: index.ms,
          files_changed: index.files_changed, files_indexed: index.files.size, coalesced: Boolean(index.coalesced),
          verification_ms: index.work.verification_ms ?? 0, index_ms: index.work.index_ms ?? 0, files_verified: index.work.files_verified ?? 0,
          bytes_hashed: index.work.bytes_hashed ?? 0, copied_bytes: index.work.copied_bytes ?? 0,
        },
        ...(index.adoption ? { index_adoption: flatAdoption(index.adoption) } : {}),
        index_digest: index.index_digest,
        upstream_version: session.identity.version,
        skipped_omitted: cov.skipped_omitted,
      },
    };
    let truncated = result.truncated || cov.walk_truncated;
    if (fit(payloadOut, ctx.budget - 1024)) truncated = true;
    payloadOut.coverage.complete = !truncated && !cov.skipped.length && !cov.skipped_omitted;
    payloadOut.coverage.exhaustive = Boolean(result.exhaustive) && !truncated;
    // Absence may be asserted only for an exhaustive, complete, fresh answer.
    payloadOut.metadata.absence_proven = payloadOut.coverage.exhaustive && payloadOut.coverage.complete;
    if (truncated) diags.push({ code: 'graft.truncated', message: 'result truncated; coverage.complete=false and absence is not proven' });
    const status = truncated || (!cfg.git) ? 'partial' : 'complete';
    return [status, payloadOut, diags];
  } finally { index.release?.(); }
}
