// Shared by server and browser: checked i64 arithmetic, string helpers, codec, validation.
export class Trap extends Error {
  constructor(code) { super(code); this.name = "Trap"; this.code = code; }
}
const MIN = -(2n ** 63n), MAX = 2n ** 63n - 1n;
const chk = (v) => { if (v < MIN || v > MAX) throw new Trap("overflow"); return v; };
const nz = (b) => { if (b === 0n) throw new Trap("division_by_zero"); return b; };
export const add = (a, b) => chk(a + b);
export const sub = (a, b) => chk(a - b);
export const mul = (a, b) => chk(a * b);
export const div = (a, b) => chk(a / nz(b));
export const rem = (a, b) => { nz(b); if (a === MIN && b === -1n) throw new Trap("overflow"); return a % b; };
export const neg = (a) => chk(-a);
const enc = new TextEncoder();
export const len = (s) => BigInt(enc.encode(s).length);
export const lenChars = (s) => { let n = 0n; for (const _ of s) n++; return n; };
export const isEmpty = (s) => s.length === 0;
export const contains = (s, t) => s.includes(t);
export const startsWith = (s, t) => s.startsWith(t);
export const concat = (a, b) => a + b;
export const fromI64 = (n) => n.toString();
export const unreachable = () => { throw new Trap("unreachable"); };
export const precondition = (ok) => { if (!ok) throw new Trap("precondition"); return true; };

export const postcondition = (ok) => { if (!ok) throw new Trap("postcondition"); return true; };

// ---- codec ----
class Raw { constructor(source) { this.source = source; } }
// JSON.parse that keeps number source text (exact big ints) where the engine supports it.
export const parseJSON = (text) =>
  JSON.parse(text, (k, v, ctx) => (typeof v === "number" && ctx && typeof ctx.source === "string" ? new Raw(ctx.source) : v));

const BAD_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;
const INT_ERR = "must be an integer within the signed 64-bit range";
function toInt(v) {
  let s;
  if (v instanceof Raw) s = v.source;
  else if (typeof v === "string") s = v;
  else if (typeof v === "number") { if (!Number.isSafeInteger(v)) return null; return BigInt(v); }
  else return null;
  if (!/^-?\d+$/.test(s)) {
    const n = v instanceof Raw ? Number(s) : NaN;
    if (!Number.isSafeInteger(n)) return null;
    s = String(n);
  }
  const n = BigInt(s);
  return n >= MIN && n <= MAX ? n : null;
}
function toFloat(v) {
  let n = null;
  if (v instanceof Raw) n = Number(v.source);
  else if (typeof v === "number") n = v;
  else if (typeof v === "string" && /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(v)) n = Number(v);
  return Number.isFinite(n) ? n : null;
}
// -> [value] on success, [undefined, message] on failure.
export function decodeValue(f, enums, v) {
  const bad = (m) => [undefined, m];
  switch (f.type) {
    case "int": case "ref": { const n = toInt(v); return n === null ? bad(INT_ERR) : [n]; }
    case "float": { const n = toFloat(v); return n === null ? bad("must be a finite number") : [n]; }
    case "bool": return typeof v === "boolean" ? [v] : bad("must be true or false");
    case "string": return typeof v === "string" && !BAD_SURROGATE.test(v) ? [v] : bad("must be a string");
    case "char": return typeof v === "string" && !BAD_SURROGATE.test(v) && [...v].length === 1 ? [v] : bad("must be exactly one Unicode character");
    case "enum": { const cases = enums[f.enum] || []; return cases.includes(v) ? [v] : bad("must be one of " + cases.join(", ")); }
  }
  return bad("unsupported field type " + f.type);
}
// Decode a parsed JSON object into the in-memory row. Returns {row, errors, bad} where bad is the set of failed fields.
export function decodeRow(ent, enums, input, withId = false) {
  const row = Object.create(null), errors = [], bad = new Set();
  for (const f of ent.fields) {
    if (!Object.hasOwn(input, f.name) || input[f.name] === null) { errors.push({ field: f.name, message: "is required" }); bad.add(f.name); continue; }
    const [v, m] = decodeValue(f, enums, input[f.name]);
    if (m) { errors.push({ field: f.name, message: m }); bad.add(f.name); } else row[f.name] = v;
  }
  if (withId && Object.hasOwn(input, "id")) { const id = toInt(input.id); if (id !== null) row.id = id; }
  return { row, errors, bad };
}
export const encValue = (f, v, strInts) => {
  switch (f.type) {
    case "int": case "ref": return strInts ? JSON.stringify(v.toString()) : v.toString();
    case "float": return String(v);
    case "bool": return v ? "true" : "false";
    default: return JSON.stringify(v);
  }
};
// Run computed fields: {name: value | {error: code}}. A throwing field never crashes.
export function evalComputed(ent, row) {
  const out = Object.create(null);
  for (const c of ent.computed || []) {
    try {
      const v = c.value(row);
      out[c.name] = c.type === "float" && !Number.isFinite(v) ? { error: "non_finite" } : v;
    } catch (e) { out[c.name] = { error: e instanceof Trap ? e.code : "error" }; }
  }
  return out;
}
// Encode a row as JSON text. ints are written from BigInt digits (exact). opts: {computed, strInts}.
export function toJSON(ent, row, opts = {}) {
  const parts = [];
  if (row.id !== undefined) parts.push('"id":' + encValue({ type: "int" }, row.id, opts.strInts));
  for (const f of ent.fields) parts.push(JSON.stringify(f.name) + ":" + encValue(f, row[f.name], opts.strInts));
  if (opts.computed) {
    const cv = evalComputed(ent, row);
    for (const c of ent.computed || []) {
      const v = cv[c.name];
      parts.push(JSON.stringify(c.name) + ":" + (v !== null && typeof v === "object" ? '{"error":' + JSON.stringify(v.error) + "}" : encValue(c, v)));
    }
  }
  return "{" + parts.join(",") + "}";
}
// Failed rules as [{field, message: rule text}]; a rule fails when its test returns false or throws.
// Rules touching a field in `bad` (undecodable) are skipped.
export function evalRules(ent, row, bad = new Set()) {
  const out = [];
  for (const r of ent.rules || []) {
    if ((r.fields || []).some((f) => bad.has(f))) continue;
    let ok;
    try { ok = r.test(row) === true; } catch { ok = false; }
    if (!ok) out.push({ field: (r.fields && r.fields[0]) || "", message: r.text });
  }
  return out;
}
// Synthesize one row satisfying every rule of `ent` (refs: {targetPath: id}). -> {row} | {fail: rule text}.
// Depth-first over per-field candidates, rules checked as soon as their fields are chosen; `budget` caps rule evaluations.
// opt: {fixed: {field: value}, extra: [{text, test}] (checked once the row is complete), vary: more string candidates}.
// Workflow fields only take the first case of their enum.
export function synthesizeRow(ent, enums, refs, budget = 20000, opt = {}) {
  const fs = ent.fields, idx = new Map(fs.map((f, i) => [f.name, i]));
  const steps = new Set((ent.steps || []).map((s) => s.field)), fixed = opt.fixed || {};
  const lits = [];
  for (const r of ent.rules || []) for (const m of r.text.matchAll(/"((?:[^"\\]|\\.)*)"/g)) {
    let s; try { s = JSON.parse(m[0]); } catch { s = m[1]; }
    if (s !== "" && !lits.includes(s)) lits.push(s);
  }
  const more = opt.vary ? [1, 2, 3, 4, 5, 6, 7, 8, 9].flatMap((i) => [...lits.flatMap((l) => [l + "abcd" + i, "abcd" + i + l + "x.io"]), "abcd" + i]) : [];
  const cand = (f) => {
    if (Object.hasOwn(fixed, f.name)) return [fixed[f.name]];
    switch (f.type) {
      case "string": return [...new Set(["", ...lits.flatMap((l) => [l + "abcd", "abcd" + l + "x.io"]), "ab", "abc", "abcd", "abcdefgh", ...more])];
      case "int": return [0, 1, 2, 3, 5, 10, 24, 50, 100, 1000].map(BigInt);
      case "float": return [0, 1.5, 100.5];
      case "bool": return [false, true];
      case "char": return ["a"];
      case "enum": return (enums[f.enum] || []).slice(0, steps.has(f.name) ? 1 : undefined);
      case "ref": return refs[f.ref] === undefined ? [] : [refs[f.ref]];
    }
    return [];
  };
  const last = fs.length - 1, at = fs.map(() => []);
  for (const r of ent.rules || []) at[Math.max(0, ...(r.fields || []).map((n) => (idx.has(n) ? idx.get(n) : last)))]?.push(r);
  at[Math.max(0, last)].push(...(opt.extra || []));
  const row = Object.create(null); let evals = 0, worst = -1, why = "";
  const go = (d) => {
    if (d > last) return true;
    const f = fs[d];
    for (const v of cand(f)) {
      row[f.name] = v;
      let ok = true;
      for (const r of at[d]) {
        if (++evals > budget) return false;
        let p; try { p = r.test(row) === true; } catch { p = false; }
        if (!p) { ok = false; if (d >= worst) { worst = d; why = r.text; } break; }
      }
      if (ok && go(d + 1)) return true;
      if (evals > budget) return false;
    }
    delete row[f.name];
    return false;
  };
  if (go(0)) return { row };
  return { fail: why || (evals > budget ? "search budget exhausted" : `no candidate for a field of ${ent.name}`) };
}

// Computed values as the API sent them (the server is the truth for rollups): {name: value | {error}}.
export function decodeComputed(ent, enums, o) {
  const out = Object.create(null);
  for (const c of ent.computed || []) {
    const v = o[c.name];
    out[c.name] = v !== null && typeof v === "object" && !(v instanceof Raw) ? { error: String(v.error) } : decodeValue(c, enums, v)[0] ?? { error: "decode" };
  }
  return out;
}

// ---- v2: unique keys, workflow steps, rollups ----
// The entity's keys; the account entity's login field is implicitly unique.
export function keysOf(ent, account) {
  const ks = ent.keys || [];
  if (!account || account.entity !== ent.path || ks.some((k) => k.fields.length === 1 && k.fields[0] === account.login)) return ks;
  return [...ks, { name: "login", fields: [account.login], value: (r) => r[account.login] }];
}
const keyVal = (k, r) => {
  try { return { value: k.value(r) }; }
  catch (e) { return { error: e instanceof Trap ? e.code : "error" }; }
};
// A failed constraint evaluation is never evidence of uniqueness.
export function keyErrors(keys, row, others, bad = new Set()) {
  const out = [];
  for (const k of keys) {
    if (k.fields.some((f) => bad.has(f))) continue;
    const v = keyVal(k, row), field = k.fields[0] ?? "";
    if (v.error) { out.push({ field, message: `key ${k.name} failed: ${v.error}` }); continue; }
    for (const o of others) {
      if (o.id === row.id) continue;
      const w = keyVal(k, o);
      if (w.error) { out.push({ field, message: `stored key ${k.name} failed: ${w.error}` }); break; }
      if (w.value === v.value) { out.push({ field, message: k.fields.join(", ") + " must be unique" }); break; }
    }
  }
  return out;
}
export const stepCases = (ent, enums, s) => enums[(ent.fields.find((f) => f.name === s.field) || {}).enum] || [];
export const stepOk = (s, a, b) => { try { return s.test(a, b) === true; } catch { return false; } };
// Workflow violations: a new row starts in the first case; a changed value must be an allowed transition from `old`.
export function stepErrors(ent, enums, row, old, bad = new Set()) {
  const out = [];
  for (const s of ent.steps || []) {
    if (bad.has(s.field)) continue;
    const to = row[s.field];
    if (!old) { const first = stepCases(ent, enums, s)[0]; if (to !== first) out.push({ field: s.field, message: `${s.field} must start as ${first}` }); }
    else if (old[s.field] !== to && !stepOk(s, old[s.field], to)) out.push({ field: s.field, message: `${s.field} cannot change from ${old[s.field]} to ${to}` });
  }
  return out;
}
// Copy of `row` with every rollup of `ent` set. kids(path) -> {ent, rows: iterable}. A failed aggregate
// (overflow, child computed error) is a property that throws its Trap, so only computed fields reading it fail.
export function withRollups(ent, row, kids) {
  const r = Object.assign(Object.create(null), row);
  for (const u of ent.rollups || []) {
    try { r[u.name] = rollup(u, row.id, kids(u.child)); }
    catch (e) { const code = e instanceof Trap ? e.code : "error"; Object.defineProperty(r, u.name, { enumerable: true, get() { throw new Trap(code); } }); }
  }
  return r;
}
function rollup(u, id, k) {
  const stored = !u.field || k.ent.fields.some((f) => f.name === u.field);
  let acc = u.kind === "sum" && u.type === "float" ? 0 : 0n;
  for (const c of k.rows) {
    if (c[u.via] !== id) continue;
    let v = true;
    if (u.field) { v = stored ? c[u.field] : evalComputed(k.ent, c)[u.field]; if (v !== null && typeof v === "object") throw new Trap(v.error); }
    if (u.kind === "count") { if (v === true) acc += 1n; }
    else acc = u.type === "float" ? acc + v : add(acc, v);
  }
  return acc;
}
