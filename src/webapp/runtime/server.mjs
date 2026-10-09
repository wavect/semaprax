// node server.mjs [--port N] [--host 127.0.0.1] [--data DIR] [--setup] [--migrate]
// node server.mjs --self-test [--data DIR]   (verify the whole app against its schema; exit 0/1)
// node server.mjs --self-test-offline       (verify pure schema/runtime behavior without IO)
// With accounts, --setup admits unauthenticated requests as an unrestricted setup user while no account
// has a password: create the first account with a password, then sign in. Setup ends with the first password.
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import * as S from "./schema.js";
import * as rt from "./runtime.js";
import { security } from "./security.mjs";
import { loadState, stateSchema, constraintErrors } from "./state.mjs";

const { entities, enums, app } = S, ACCOUNT = S.account ?? null;
const here = path.dirname(fileURLToPath(import.meta.url));
const opt = { port: "8080", host: "127.0.0.1", data: "./data" };
const argv = process.argv.slice(2), flag = (n) => { const i = argv.indexOf(n); if (i >= 0) argv.splice(i, 1); return i >= 0; };
const SELF = flag("--self-test"), SELF_OFFLINE = flag("--self-test-offline"), SETUP = flag("--setup"), MIGRATE = flag("--migrate");
for (let i = 0; i < argv.length; i += 2) {
  const k = argv[i].replace(/^--/, "");
  if (!(k in opt) || argv[i + 1] === undefined || ((SELF || SELF_OFFLINE) && k !== "data")) { console.error("usage: node server.mjs [--port N] [--host H] [--data DIR] [--setup] [--migrate] | --self-test [--data DIR] | --self-test-offline"); process.exit(2); }
  opt[k] = argv[i + 1];
}
if (SELF_OFFLINE && (SELF || SETUP || MIGRATE || argv.length)) { console.error("usage: node server.mjs --self-test-offline"); process.exit(2); }
const SCRYPT = { N: 16384, r: 8, p: 1 };
function hashPw(pw, salt = crypto.randomBytes(16)) { return salt.toString("hex") + ":" + crypto.scryptSync(pw, salt, 32, SCRYPT).toString("hex"); }
if (SELF) process.exit(await selfTest());
// Keep this dispatch before data-directory initialization, lock creation, and listener setup.
if (SELF_OFFLINE) process.exit(offlineSelfTest());
const LIMIT = 1 << 20;
const protection = security();
const sha = (s) => crypto.createHash("sha256").update(s).digest("hex");
fs.mkdirSync(path.resolve(opt.data), { recursive: true });
const dir = fs.realpathSync(path.resolve(opt.data)), dbFile = path.join(dir, "db.json"), authFile = path.join(dir, "auth.json"), auditFile = path.join(dir, "audit.jsonl");
// One exclusive claim over the canonical directory. A crash leaves a conservative
// stale claim: the operator may remove it only after confirming its PID is dead.
const lockDir = path.join(dir, ".writer-lock"), lockOwner = path.join(lockDir, "owner.json");
const lockToken = crypto.randomBytes(16).toString("hex");
try { fs.mkdirSync(lockDir); }
catch (e) { throw new Error(`data directory already has a writer claim (${lockOwner}); after a crash confirm that the recorded PID is dead before removing .writer-lock: ${e.message}`); }
fs.writeFileSync(lockOwner, JSON.stringify({ pid: process.pid, token: lockToken }), { flag: "wx", mode: 0o600 });
process.on("exit", () => {
  try { if (JSON.parse(fs.readFileSync(lockOwner, "utf8")).token === lockToken) { fs.unlinkSync(lockOwner); fs.rmdirSync(lockDir); } } catch { /* retain an uncertain claim */ }
});
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => process.exit(0));
// Only a spawning parent with an explicit private IPC channel can request this.
// Windows kill(SIGTERM) is forced termination and cannot run exit cleanup.
if (process.connected) process.on("message", (message) => {
  if (message === "semaprax.webapp.stop.v1") process.exit(0);
});
// Node cannot open directory descriptors on Windows. Flush the published
// file there; staged bytes are already flushed before the atomic rename.
// POSIX also flushes the containing directory. Every flush error propagates.
const syncPublication = (file) => {
  const windows = process.platform === "win32";
  const fd = fs.openSync(windows ? file : dir, windows ? "r+" : "r");
  try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
};
const stageFile = (file, text, mode) => {
  const fd = fs.openSync(file + ".tmp", "w", mode);
  try { fs.writeFileSync(fd, text); fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
};
const writeAtomic = (file, text, mode) => { stageFile(file, text, mode); fs.renameSync(file + ".tmp", file); syncPublication(file); };
// state.json is the publication boundary, including all required audit facts.
// Legacy files remain readable mirrors; a restart repairs them from this snapshot.
const stateFile = path.join(dir, "state.json");
if (fs.existsSync(stateFile)) {
  const state = JSON.parse(fs.readFileSync(stateFile, "utf8"));
  if (state.version !== 1 || ![state.db, state.auth, state.audit].every((v) => typeof v === "string")) throw new Error("corrupt state snapshot");
  writeAtomic(dbFile, state.db); writeAtomic(authFile, state.auth, 0o600); writeAtomic(auditFile, state.audit);
}

// ---- state: path -> {ent, rows: Map<BigInt id,row>, next: BigInt} ----
const tables = new Map(entities.map((ent) => [ent.path, { ent, rows: new Map(), next: 1n }]));
const accT = ACCOUNT && tables.get(ACCOUNT.entity);
if (ACCOUNT && !accT) throw new Error(`schema: account entity ${ACCOUNT.entity} not found`);
let migrationPending = false;
function load() {
  if (!fs.existsSync(dbFile)) return;
  const bytes = fs.readFileSync(dbFile, "utf8");
  const changed = loadState(bytes, tables, enums, ACCOUNT, MIGRATE);
  if (changed) {
    const backup = path.join(dir, "db.before-" + sha(bytes) + ".json");
    if (!fs.existsSync(backup)) fs.writeFileSync(backup, bytes, { flag: "wx", mode: 0o600 });
    migrationPending = true;
  }
}
function databaseText() {
  const rows = [], next = [];
  for (const [p, t] of tables) {
    rows.push(JSON.stringify(p) + ":[" + [...t.rows.values()].map((r) => rt.toJSON(t.ent, r, { strInts: true })).join(",") + "]");
    next.push(JSON.stringify(p) + ":" + JSON.stringify(t.next.toString()));
  }
  return `{"version":2,"schema":${JSON.stringify(stateSchema(entities, enums))},"next":{${next.join(",")}},"rows":{${rows.join(",")}}}\n`;
}
// auth.json: passwords (account id -> "salt:scrypt" hex) and sessions (sha256(token) -> account id). Never served.
const auth = { pw: new Map(), sess: new Map() };
function loadAuth() {
  if (!fs.existsSync(authFile)) return;
  const o = JSON.parse(fs.readFileSync(authFile, "utf8"));
  for (const [k, v] of Object.entries(o.passwords || {})) auth.pw.set(k, v);
  for (const [k, v] of Object.entries(o.sessions || {})) auth.sess.set(k, v);
}
const sorted = (m) => Object.fromEntries([...m].sort((a, b) => (a[0] < b[0] ? -1 : 1)));
const authText = () => JSON.stringify({ version: 1, passwords: sorted(auth.pw), sessions: sorted(auth.sess) }) + "\n";
const saveAuth = () => persistState();
// audit.jsonl: ordered audit mirror; state.json binds it to the row/auth publication.
const log = []; let seq = 1;
function loadAudit() {
  if (!fs.existsSync(auditFile)) return;
  const text = fs.readFileSync(auditFile, "utf8");
  if (text && !text.endsWith("\n")) fs.appendFileSync(auditFile, "\n"); // a torn last line stays unparsed
  for (const line of text.split("\n")) {
    let o; try { o = rt.parseJSON(line); } catch { continue; }
    const num = (v) => String(v && v.source !== undefined ? v.source : v);
    log.push({ p: o.entity, id: BigInt(num(o.id)), line });
    seq = Math.max(seq, Number(num(o.seq)) + 1);
  }
}
load(); loadAuth(); loadAudit();
if (migrationPending) persistState();

// ---- http helpers ----
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".mjs": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8" };
const STATIC = new Set(["index.html", "app.js", "style.css", "schema.js", "runtime.js"]);
const send = (res, code, body = "", type = "application/json; charset=utf-8", extra = {}) => {
  res.writeHead(code, { "content-type": type, "cache-control": "no-store", "x-content-type-options": "nosniff", ...extra });
  res.end(body);
};
const fail = (res, code, msg) => send(res, code, JSON.stringify({ error: msg }));
const verrs = (res, errors) => send(res, 400, JSON.stringify({ errors }));
const roll = (t, row) => rt.withRollups(t.ent, row, (p) => ({ ent: tables.get(p).ent, rows: tables.get(p).rows.values() }));
const out = (t, row) => rt.toJSON(t.ent, roll(t, row), { computed: true });
function readBody(req, res) {
  return new Promise((resolve) => {
    const chunks = []; let size = 0, done = false;
    const tooBig = () => { if (done) return; done = true; send(res, 413, JSON.stringify({ error: "request body exceeds 1 MiB" }), undefined, { connection: "close" }); res.once("finish", () => req.destroy()); resolve(null); };
    if (Number(req.headers["content-length"]) > LIMIT) return tooBig();
    req.on("data", (c) => { size += c.length; if (size > LIMIT) return tooBig(); chunks.push(c); });
    req.on("end", () => { if (!done) { done = true; resolve(Buffer.concat(chunks).toString("utf8")); } });
    req.on("error", () => { if (!done) { done = true; resolve(null); } });
  });
}

// ---- accounts, sessions, permissions ----

const allowed = (r) => { try { return ACCOUNT.allowed(r) === true; } catch { return false; } };
const pass = (p, r, u) => { try { return p.test(r, u) === true; } catch { return false; } };
const sid = (req) => (/(?:^|;\s*)sid=([0-9a-f]{64})(?:;|$)/.exec(req.headers.cookie || "") || [])[1];
const COOKIE = "; HttpOnly; SameSite=Strict; Path=/";
// The caller: {u: account row} when signed in, {free: true} without accounts or in setup mode, null -> 401.
function who(req) {
  if (!ACCOUNT) return { u: null, free: true };
  const tok = sid(req), id = tok && auth.sess.get(sha(tok)), u = id && accT.rows.get(BigInt(id));
  if (u && allowed(u)) return { u, free: false };
  if (SETUP && auth.pw.size === 0) return { u: null, free: true, setup: true };
  return null;
}
const canR = (c, t, r) => c.free || !t.ent.canRead || pass(t.ent.canRead, r, c.u);
const canW = (c, t, r) => c.free || !t.ent.canWrite || pass(t.ent.canWrite, r, c.u);
const canNew = (c, t) => rt.canNew(c, t);
// GET /api/audit: allowed without accounts, in setup mode, or when canWrite of the account entity passes for (u, u).
const canAudit = (c) => c.free || !accT.ent.canWrite || pass(accT.ent.canWrite, c.u, c.u);
function checkPw(pw, stored) {
  const [s, h] = (stored || "00".repeat(16) + ":" + "00".repeat(32)).split(":");
  const got = crypto.scryptSync(typeof pw === "string" ? pw : "", Buffer.from(s, "hex"), 32, SCRYPT);
  return crypto.timingSafeEqual(got, Buffer.from(h, "hex")) && !!stored;
}
async function session(req, res) {
  const m = req.method;
  if (m === "GET") {
    const c = who(req);
    if (!c) return fail(res, 401, "sign in required");
    return send(res, 200, c.setup ? '{"setup":true}' : out(accT, c.u));
  }
  if (m === "DELETE") {
    if (!who(req)) return fail(res, 401, "sign in required");
    if (!protection.valid(req)) return fail(res, 403, "invalid CSRF token");
    const tok = sid(req), key = tok && sha(tok), previous = key && auth.sess.get(key);
    if (previous !== undefined && key) {
      auth.sess.delete(key);
      try { saveAuth(); } catch (e) {
        if (!e.committed) auth.sess.set(key, previous);
        return fail(res, e.committed ? 503 : 500, e.committed ? "session change committed; restart repairs persistence mirrors" : "could not persist: " + e.message);
      }
    }
    return send(res, 204, "", undefined, { "set-cookie": "sid=; Max-Age=0" + COOKIE });
  }
  if (m !== "POST") return fail(res, 405, "method not allowed");
  if (!protection.valid(req)) return fail(res, 403, "invalid CSRF token");
  const text = await readBody(req, res);
  if (text === null) return;
  let b; try { b = JSON.parse(text); } catch { b = null; }
  // `login` is the canonical transport key. The declared account login field
  // is an equivalent spelling, so an email-backed account accepts `email`.
  const login = b && typeof b === "object" ? (b.login ?? b[ACCOUNT.login]) : undefined;
  if (!protection.attempt(req.socket.remoteAddress || "", login)) return send(res, 429, '{"error":"sign-in rate limit"}', undefined, { "retry-after": "60" });
  let row = null;
  if (typeof login === "string") for (const r of accT.rows.values()) if (r[ACCOUNT.login] === login) { row = r; break; }
  const ok = checkPw(b && b.password, row && auth.pw.get(String(row.id)));
  if (!ok || !row || !allowed(row)) return fail(res, 401, "invalid sign-in");
  const tok = crypto.randomBytes(32).toString("hex");
  auth.sess.set(sha(tok), String(row.id));
  try { saveAuth(); } catch (e) {
    if (!e.committed) auth.sess.delete(sha(tok));
    return fail(res, e.committed ? 503 : 500, e.committed ? "session change committed; restart repairs persistence mirrors" : "could not persist: " + e.message);
  }
  send(res, 200, out(accT, row), undefined, { "set-cookie": "sid=" + tok + COOKIE, "x-csrf-token": protection.issue(req, tok).token });
}

// ---- validation, audit, csv ----
function validate(t, input, old, c) {
  if (input === null || typeof input !== "object" || Array.isArray(input)) return { errors: [{ field: "", message: "body must be a JSON object" }] };
  const { row, errors, bad } = rt.decodeRow(t.ent, enums, input);
  for (const f of t.ent.fields) {
    // The shared decoder deliberately accepts decimal strings for browser
    // form controls. At the HTTP boundary, number fields require JSON number
    // tokens; parseJSON's Raw wrapper preserves exact i64 source lexemes.
    if (["int", "float", "ref"].includes(f.type) && !bad.has(f.name) && !rt.isJSONNumber(input[f.name])) {
      errors.push({ field: f.name, message: f.type === "float" ? "must be a finite JSON number" : "must be a numeric signed 64-bit integer" });
      delete row[f.name]; bad.add(f.name);
    }
    if (f.type === "ref" && !bad.has(f.name) && !(tables.get(f.ref) && tables.get(f.ref).rows.has(row[f.name])))
      errors.push({ field: f.name, message: `${f.name} must reference an existing ${f.ref} row (id ${row[f.name]} not found)` });
  }
  errors.push(...rt.evalRules(t.ent, row, bad));
  if (old) row.id = old.id;
  errors.push(...rt.keyErrors(rt.keysOf(t.ent, ACCOUNT), row, [...t.rows.values()], bad));
  errors.push(...rt.stepErrors(t.ent, enums, row, old, bad));
  let pw = null;
  if (t === accT) {
    pw = input.password ?? null;
    if (pw !== null && (typeof pw !== "string" || rt.len(pw) < 8n || /[\uD800-\uDFFF]/.test(pw))) errors.push({ field: "password", message: "must be a string of at least 8 bytes" });
    else if (pw === null && !old && !c.setup) errors.push({ field: "password", message: "is required" });
  }
  return { row, errors, pw };
}
function referrer(t, id) {
  for (const o of tables.values())
    for (const f of o.ent.fields)
      if (f.type === "ref" && f.ref === t.ent.path)
        for (const r of o.rows.values()) if (r[f.name] === id && !(o === t && r.id === id)) return `cannot delete ${t.ent.name} ${id}: referenced by ${o.ent.name} ${r.id} (field ${f.name})`;
  return null;
}
function persistState() {
  const db = databaseText(), authBytes = authText(), auditBytes = log.map((e) => e.line + "\n").join("");
  const mirrors = [[dbFile, db], [authFile, authBytes, 0o600], [auditFile, auditBytes]];
  let committed = false;
  try {
    // Check mirror destinations and stage every byte before the publication pivot.
    for (const [file, text, mode] of mirrors) {
      if (fs.existsSync(file) && !fs.statSync(file).isFile()) throw new Error(`not a regular file: ${file}`);
      stageFile(file, text, mode);
    }
    stageFile(stateFile, JSON.stringify({ version: 1, db, auth: authBytes, audit: auditBytes }) + "\n", 0o600);
    fs.renameSync(stateFile + ".tmp", stateFile); committed = true; syncPublication(stateFile);
    for (const [file] of mirrors) fs.renameSync(file + ".tmp", file);
    syncPublication(stateFile);
  } catch (e) { e.committed = committed; throw e; }
  finally { for (const [file] of [...mirrors, [stateFile]]) { try { fs.unlinkSync(file + ".tmp"); } catch { /* no stage remains */ } } }
}
// A failure before publication restores memory. A post-pivot failure is explicit
// committed uncertainty: restart replays the snapshot, with exactly one audit fact.
function commit(res, undo, entry) {
  log.push(entry); seq++;
  try { persistState(); return true; }
  catch (e) {
    if (!e.committed) { log.pop(); seq--; undo(); return fail(res, 500, "could not persist: " + e.message), false; }
    send(res, 503, JSON.stringify({ error: "mutation committed; persistence mirror recovery required", committed: true }));
    return false;
  }
}
function audit(c, t, id, action, old, now, pwSet) {
  const ch = [];
  for (const f of t.ent.fields) {
    const a = old ? rt.encValue(f, old[f.name]) : "null", b = now ? rt.encValue(f, now[f.name]) : "null";
    if (a !== b) ch.push(`${JSON.stringify(f.name)}:[${a},${b}]`);
  }
  if (pwSet) ch.push('"password":[null,"changed"]');
  const line = `{"seq":${seq},"at":${JSON.stringify(new Date().toISOString())},"by":${c.u ? c.u.id : "null"},"entity":${JSON.stringify(t.ent.path)},"id":${id},"action":"${action}","changes":{${ch.join(",")}}}`;
  return { p: t.ent.path, id, line };
}
// A leading = + - @ tab or CR would run as a spreadsheet formula; prefix it.
const csvCell = (s) => {
  if (/^[=+\-@\t\r]/.test(s) && !/^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(s)) s = "'" + s;
  return /[",\r\n]/.test(s) ? '"' + s.replaceAll('"', '""') + '"' : s;
};
const csvText = (f, v) => (v !== null && typeof v === "object" ? "error:" + v.error : f.type === "bool" ? (v ? "true" : "false") : String(v));
function csv(t, rows) {
  const cs = t.ent.computed || [], lines = [["id", ...[...t.ent.fields, ...cs].map((f) => f.name)].map(csvCell).join(",")];
  for (const r of rows) {
    const cv = rt.evalComputed(t.ent, roll(t, r));
    lines.push([String(r.id), ...t.ent.fields.map((f) => csvText(f, r[f.name])), ...cs.map((f) => csvText(f, cv[f.name]))].map(csvCell).join(","));
  }
  return lines.join("\r\n") + "\r\n";
}
// GET /api/<path>[?q=..&<enumfield>=<Case>&format=csv]: readable rows matching the search and filters.
function list(req, res, t, c) {
  const qs = new URL(req.url, "http://x").searchParams, q = (qs.get("q") || "").toLowerCase();
  const strs = t.ent.fields.filter((f) => f.type === "string"), fl = t.ent.fields.filter((f) => f.type === "enum" && qs.get(f.name));
  const rows = [...t.rows.values()].filter((r) => canR(c, t, r) && (!q || strs.some((f) => r[f.name].toLowerCase().includes(q))) && fl.every((f) => r[f.name] === qs.get(f.name)));
  if (qs.get("format") === "csv") return send(res, 200, csv(t, rows), "text/csv; charset=utf-8", { "content-disposition": `attachment; filename="${t.ent.path}.csv"` });
  send(res, 200, "[" + rows.map((r) => out(t, r)).join(",") + "]");
}

async function api(req, res, parts, c) {
  const t = tables.get(parts[0]);
  if (!t) return fail(res, 404, "unknown entity");
  const m = req.method, hasId = parts.length >= 2, hist = parts[2] === "history";
  if (parts.length > 3 || (parts.length === 3 && !hist) || (hasId && !/^[1-9]\d{0,18}$/.test(parts[1]))) return fail(res, 404, "not found");
  const id = hasId ? BigInt(parts[1]) : null;
  let cur = hasId ? t.rows.get(id) : null;
  if (cur && !canR(c, t, cur)) cur = undefined; // unreadable rows are absent
  if (!hasId) {
    if (m === "GET") return list(req, res, t, c);
    if (m !== "POST") return fail(res, 405, "method not allowed");
    if (!canNew(c, t)) return fail(res, 403, `not allowed to create ${t.ent.name}`);
  } else {
    if (m !== "GET" && (hist || (m !== "PUT" && m !== "DELETE"))) return fail(res, 405, "method not allowed");
    if (!cur) return fail(res, 404, `${t.ent.name} ${id} not found`);
    if (hist) return send(res, 200, "[" + log.filter((e) => e.p === t.ent.path && e.id === id).map((e) => e.line).join(",") + "]");
    if (m === "GET") return send(res, 200, out(t, cur));
    if (!canW(c, t, cur)) return fail(res, 403, `not allowed to ${m === "PUT" ? "update" : "delete"} this ${t.ent.name}`);
    if (m === "DELETE") {
      const constraints = constraintErrors(tables, { path: t.ent.path, id, row: null });
      if (constraints.length) return verrs(res, constraints);
      const why = referrer(t, id);
      if (why) return fail(res, 409, why);
      const key = String(id), pw = auth.pw.get(key), sess = [...auth.sess].filter(([, v]) => v === key);
      t.rows.delete(id);
      const isAcc = t === accT && (pw !== undefined || sess.length > 0);
      if (isAcc) { auth.pw.delete(key); sess.forEach(([k]) => auth.sess.delete(k)); }
      if (!commit(res, () => { t.rows.set(id, cur); if (pw !== undefined) auth.pw.set(key, pw); sess.forEach(([k, v]) => auth.sess.set(k, v)); }, audit(c, t, id, "delete", cur, null))) return;
      return send(res, 204);
    }
  }
  const text = await readBody(req, res);
  if (text === null) return;
  let input;
  try { input = rt.parseJSON(text); } catch { return verrs(res, [{ field: "", message: "body is not valid JSON" }]); }
  c = who(req);
  if (!c) return fail(res, 401, "sign in required");
  cur = hasId ? t.rows.get(id) : null;
  if (hasId && (!cur || !canR(c, t, cur))) return fail(res, 404, `${t.ent.name} ${id} not found`);
  if (hasId && !canW(c, t, cur)) return fail(res, 403, `not allowed to update this ${t.ent.name}`);
  const { row, errors, pw } = validate(t, input, cur, c);
  if (errors.length) return verrs(res, errors);
  if (!hasId && t.next > 9223372036854775807n) return fail(res, 507, "entity id space exhausted");
  row.id = hasId ? id : t.next;
  const constraints = constraintErrors(tables, { path: t.ent.path, id: row.id, row });
  if (constraints.length) return verrs(res, constraints);
  if (!canW(c, t, row)) return fail(res, 403, `not allowed to ${hasId ? "update" : "create"} this ${t.ent.name}`);
  const key = String(row.id), oldPw = auth.pw.get(key);
  if (pw !== null) auth.pw.set(key, hashPw(pw));
  const undoPw = () => { if (pw === null) return; if (oldPw === undefined) auth.pw.delete(key); else auth.pw.set(key, oldPw); };
  if (hasId) {
    t.rows.set(id, row);
    if (!commit(res, () => { t.rows.set(id, cur); undoPw(); }, audit(c, t, id, "update", cur, row, pw !== null))) return;
    return send(res, 200, out(t, row));
  }
  t.next += 1n; t.rows.set(row.id, row);
  if (!commit(res, () => { t.rows.delete(row.id); t.next = row.id; undoPw(); }, audit(c, t, row.id, "create", null, row, pw !== null))) return;
  send(res, 201, out(t, row), undefined, { location: `/api/${t.ent.path}/${row.id}` });
}
async function route(req, res, p) {
  if (p.length === 2 && p[0] === "session" && p[1] === "csrf" && req.method === "GET") {
    const csrf = protection.issue(req);
    return send(res, 200, JSON.stringify({ token: csrf.token }), undefined, { "set-cookie": csrf.cookie });
  }
  if (ACCOUNT && p.length === 1 && p[0] === "session") return session(req, res);
  const c = who(req);
  if (!c) return fail(res, 401, "sign in required");
  if (!["GET", "HEAD"].includes(req.method) && !protection.valid(req)) return fail(res, 403, "invalid CSRF token");
  if (p.length === 1 && p[0] === "audit" && !tables.has("audit")) {
    if (req.method !== "GET") return fail(res, 405, "method not allowed");
    if (!canAudit(c)) return fail(res, 403, "not allowed to read the audit log");
    return send(res, 200, "[" + log.map((e) => e.line).join(",") + "]");
  }
  return api(req, res, p, c);
}

const server = http.createServer((req, res) => {
  let parts;
  try { parts = new URL(req.url, "http://x").pathname.split("/").filter(Boolean).map(decodeURIComponent); } catch { return fail(res, 404, "not found"); }
  if (parts[0] === "api") return route(req, res, parts.slice(1)).catch((e) => { console.error(e); if (!res.headersSent) fail(res, 500, "internal error"); });
  const name = parts.length === 0 ? "index.html" : parts.length === 1 ? parts[0] : "";
  if ((req.method === "GET" || req.method === "HEAD") && STATIC.has(name)) {
    return fs.readFile(path.join(here, name), (e, buf) => (e ? fail(res, 404, "not found") : send(res, 200, req.method === "HEAD" ? "" : buf, TYPES[path.extname(name)])));
  }
  fail(res, 404, "not found");
});
const listenError = (error) => {
  const code = error?.code;
  if (code === "EPERM" || code === "EACCES") {
    console.error(`${code}: listen denied. --self-test-offline is schema-only; full server/browser acceptance still required.`);
  } else {
    console.error(`server listen failed (${code ?? "unknown"}): ${error?.message ?? error}`);
  }
  process.exitCode = 1;
};
server.once("error", listenError);
server.listen(Number(opt.port), opt.host, () => {
  server.off("error", listenError);
  console.log(`${app.title} listening on http://${opt.host}:${server.address().port}/ (data: ${dbFile})`);
  if (ACCOUNT && SETUP && auth.pw.size === 0) console.log("setup mode: requests are unrestricted until an account has a password");
});

// ---- --self-test-offline: exercise the generated schema/runtime without IO ----
function offlineSelfTest() {
  const fails = [], short = (v) => { const s = typeof v === "string" ? v : String(v); return s.length > 120 ? s.slice(0, 120) + "..." : s; };
  const check = (ent, what, want, got, ok) => { if (!ok) fails.push(`FAIL ${ent} ${what}: ${want} got ${short(got)}`); return ok; };
  const byPath = new Map(entities.map((e) => [e.path, e]));
  const order = [], seen = new Set();
  const visit = (e, stack) => {
    if (seen.has(e.path)) return;
    if (stack.includes(e.path)) { fails.push(`FAIL ${e.name} synthesize: cyclic or self reference ${e.path}`); return; }
    for (const f of e.fields) if (f.type === "ref" && byPath.has(f.ref)) visit(byPath.get(f.ref), [...stack, e.path]);
    seen.add(e.path); order.push(e);
  };
  entities.forEach((e) => visit(e, []));
  const refs = Object.create(null), made = [], rejectedTypes = new Set();
  const allowRule = ACCOUNT ? { text: "account may sign in", test: (r) => { try { return ACCOUNT.allowed(r) === true; } catch { return false; } } } : null;
  const invalidValue = (type) => ({ string: 7, int: true, ref: true, float: true, bool: 1, char: "", enum: 7 })[type];
  const sameFields = (ent, a, b) => !!b && ent.fields.every((f) => a[f.name] === b[f.name]);
  const alternatives = { string: ["", "a", "ab"], int: [-1n, 0n, 1n, 1000000n], float: [-1, 0, 1e6] };
  let ruleWitnesses = 0, workflowWitnesses = 0, typeChecks = 0, computedChecks = 0, rollupChecks = 0, keyChecks = 0;
  try {
    for (const ent of order) {
      const first = ACCOUNT && ent.path === ACCOUNT.entity
        ? rt.synthesizeRow(ent, enums, refs, 20000, { extra: [allowRule] })
        : null;
      const synthesized = first?.row ? first : rt.synthesizeRow(ent, enums, refs);
      if (!synthesized.row) { fails.push(`FAIL ${ent.name} synthesize: ${short(synthesized.fail)}`); continue; }
      const row = Object.assign(Object.create(null), synthesized.row, { id: 1n });
      const parsed = rt.parseJSON(rt.toJSON(ent, row, { strInts: true }));
      const decoded = rt.decodeRow(ent, enums, parsed);
      check(ent.name, "decode", "synthesized row round-trips", decoded.errors.map((e) => e.message).join(", "), !decoded.errors.length && sameFields(ent, row, decoded.row));
      check(ent.name, "rules", "synthesized row satisfies rules", rt.evalRules(ent, row).map((e) => e.message).join(", "), !rt.evalRules(ent, row).length);
      check(ent.name, "workflow start", "synthesized row starts in initial states", rt.stepErrors(ent, enums, row, null).map((e) => e.message).join(", "), !rt.stepErrors(ent, enums, row, null).length);
      for (const f of ent.fields) {
        const malformed = { ...row, [f.name]: invalidValue(f.type) };
        const result = rt.decodeRow(ent, enums, malformed);
        check(ent.name, `type ${f.type}`, `reject malformed ${f.name}`, result.errors.map((e) => e.message).join(", "), result.bad.has(f.name));
        if (result.bad.has(f.name)) { rejectedTypes.add(f.type); typeChecks++; }
      }
      for (const rule of ent.rules || []) {
        let witnessed = false;
        for (const name of rule.fields || []) {
          const f = ent.fields.find((x) => x.name === name);
          if (!f || !alternatives[f.type] || (ent.steps || []).some((s) => s.field === name)) continue;
          for (const value of alternatives[f.type]) {
            if (value === row[name]) continue;
            if (rt.evalRules(ent, { ...row, [name]: value }).some((e) => e.message === rule.text)) { witnessed = true; break; }
          }
          if (witnessed) break;
        }
        if (witnessed) ruleWitnesses++;
      }
      const keys = rt.keysOf(ent, ACCOUNT);
      if (keys.length) {
        const duplicate = rt.keyErrors(keys, { ...row, id: 2n }, [{ ...row, id: 1n }]);
        check(ent.name, "unique keys", "detect duplicate synthetic row", duplicate.map((e) => e.message).join(", "), duplicate.length === keys.length);
        keyChecks += duplicate.length === keys.length ? 1 : 0;
      }
      for (const step of ent.steps || []) {
        const cases = rt.stepCases(ent, enums, step), from = row[step.field];
        const allowed = cases.find((to) => to !== from && rt.stepOk(step, from, to) && !rt.evalRules(ent, { ...row, [step.field]: to }).length);
        const denied = cases.find((to) => to !== from && !rt.stepOk(step, from, to));
        if (allowed !== undefined) {
          const errors = rt.stepErrors(ent, enums, { ...row, [step.field]: allowed }, row);
          check(ent.name, "workflow allowed", `${step.field} permits ${from} -> ${allowed}`, errors.map((e) => e.message).join(", "), !errors.length);
          if (!errors.length) workflowWitnesses++;
        }
        if (denied !== undefined) {
          const errors = rt.stepErrors(ent, enums, { ...row, [step.field]: denied }, row);
          check(ent.name, "workflow denied", `${step.field} rejects ${from} -> ${denied}`, errors.map((e) => e.message).join(", "), errors.some((e) => e.field === step.field));
          if (errors.some((e) => e.field === step.field)) workflowWitnesses++;
        }
      }
      refs[ent.path] = row.id;
      made.push({ ent, row });
    }
    const kids = (p) => ({ ent: byPath.get(p), rows: made.filter((m) => m.ent.path === p).map((m) => m.row) });
    for (const { ent, row } of made) {
      const local = rt.withRollups(ent, row, kids), computed = rt.evalComputed(ent, local);
      for (const c of ent.computed || []) {
        const value = computed[c.name], ok = Object.hasOwn(computed, c.name) && !(value && typeof value === "object" && Object.hasOwn(value, "error"));
        const [, decodeError] = rt.decodeValue(c, enums, typeof value === "bigint" ? value.toString() : value);
        const typeMatches = c.type === "int" || c.type === "ref" ? typeof value === "bigint"
          : c.type === "float" ? typeof value === "number" && Number.isFinite(value)
            : c.type === "bool" ? typeof value === "boolean"
              : ["string", "char", "enum"].includes(c.type) && typeof value === "string";
        const valid = ok && !decodeError && typeMatches;
        check(ent.name, "computed", `derive ${c.name} with declared type and without runtime error`, value, valid);
        if (valid) computedChecks++;
      }
      for (const u of ent.rollups || []) {
        const child = byPath.get(u.child), childRows = kids(u.child).rows;
        let expected = u.kind === "sum" && u.type === "float" ? 0 : 0n;
        for (const candidate of childRows) {
          if (candidate[u.via] !== row.id) continue;
          let value = true;
          if (u.field) value = child.fields.some((f) => f.name === u.field) ? candidate[u.field] : rt.evalComputed(child, candidate)[u.field];
          if (value !== null && typeof value === "object") throw new Error(`computed child ${u.child}.${u.field} failed`);
          if (u.kind === "count") { if (value === true) expected += 1n; }
          else expected = u.type === "float" ? expected + value : rt.add(expected, value);
        }
        check(ent.name, "rollup", `${u.name} matches direct child reduction`, local[u.name], local[u.name] === expected);
        if (local[u.name] === expected) rollupChecks++;
      }
    }
  } catch (e) { fails.push("FAIL offline self-test: " + short(e.message)); }
  const typeKinds = new Set(entities.flatMap((e) => e.fields.map((f) => f.type)));
  check("schema", "type coverage", "reject one malformed value for every field type", [...typeKinds].join(","), [...typeKinds].every((t) => rejectedTypes.has(t)));
  if (fails.length) { console.log(fails.join("\n") + `\noffline self-test failed: ${fails.length} check(s)`); return 1; }
  const sum = (k) => entities.reduce((n, e) => n + (k === "keys" ? rt.keysOf(e, ACCOUNT).length : (e[k] || []).length), 0);
  console.log(`offline self-test ok: ${entities.length} entities, ${typeKinds.size} types, ${sum("rules")} rules, ${sum("computed")} computed, ${sum("keys")} keys, ${sum("steps")} workflows, ${sum("rollups")} rollups (decode, ${typeChecks} invalid types, ${ruleWitnesses} rule witnesses, ${keyChecks} duplicate-key checks, ${workflowWitnesses} workflow cases, ${computedChecks} computed values, ${rollupChecks} rollup checks)`);
  return 0;
}

// ---- --self-test: run the real server as a child on port 0, drive it from the schema alone ----
async function selfTest() {
  const given = argv.includes("--data");
  if (given && fs.existsSync(opt.data) && fs.readdirSync(opt.data).length) { console.error(`self-test: refusing non-empty data dir ${opt.data}`); return 1; }
  const dir = given ? path.resolve(opt.data) : fs.mkdtempSync(path.join(os.tmpdir(), "selftest-"));
  const authFile = path.join(dir, "auth.json"), stateFile = path.join(dir, "state.json");
  // Offline fixture setup updates the same authoritative snapshot as the server.
  const fixtureAuth = (text) => {
    const state = JSON.parse(fs.readFileSync(stateFile, "utf8")); state.auth = text;
    fs.writeFileSync(stateFile, JSON.stringify(state));
    fs.writeFileSync(authFile, text);
  };
  const fails = [], warns = [], short = (v) => { const s = typeof v === "string" ? v : String(v); return s.length > 120 ? s.slice(0, 120) + "..." : s; };
  const check = (ent, what, want, got, ok) => { if (!ok) fails.push(`FAIL ${ent} ${what}: ${want} got ${short(got)}`); return ok; };
  let child = null;
  const start = () => new Promise((resolve, reject) => {
    child = spawn(process.execPath, [fileURLToPath(import.meta.url), "--port", "0", "--data", dir, "--setup"], { stdio: ["ignore", "pipe", "pipe", "ipc"] });
    let buf = "";
    const timer = setTimeout(() => reject(new Error("server did not start: " + buf)), 10000);
    child.stderr.on("data", (c) => { buf += c; });
    child.stdout.on("data", (c) => { buf += c; const m = /listening on (http:\/\/\S+?)\/ /.exec(buf); if (m) { clearTimeout(timer); resolve(m[1] + "/api/"); } });
    child.on("exit", () => { clearTimeout(timer); reject(new Error("server exited: " + buf)); });
  });
  const stop = () => new Promise((resolve) => { if (!child || child.exitCode !== null) return resolve(); child.removeAllListeners("exit"); child.on("exit", resolve); child.send("semaprax.webapp.stop.v1"); });
  let base;
  const call = async (method, p, body, cookie) => {
    try {
      const headers = body === undefined ? {} : { "content-type": "application/json" };
      if (cookie) headers.cookie = cookie;
      if (!["GET", "HEAD"].includes(method)) {
        const csrf = await fetch(base + "session/csrf", { headers: cookie ? { cookie } : {} });
        const seed = (csrf.headers.get("set-cookie") || "").split(";")[0];
        headers.cookie = [cookie, seed].filter(Boolean).join("; ");
        headers["x-csrf-token"] = (await csrf.json()).token;
      }
      const r = await fetch(base + p, { method, headers, body });
      return { status: r.status, text: await r.text(), type: r.headers.get("content-type") || "", cookie: r.headers.get("set-cookie") || "" };
    } catch (e) { return { status: 0, text: String(e) }; }
  };
  // body text -> decoded row (with id), or null
  const dec = (ent, text) => { try { const d = rt.decodeRow(ent, enums, rt.parseJSON(text), true); return d.errors.length ? null : d.row; } catch { return null; } };
  const same = (ent, a, b) => !!b && ent.fields.every((f) => a[f.name] === b[f.name]);
  const arr = (text) => { try { const v = JSON.parse(text); return Array.isArray(v) ? v : []; } catch { return []; } };
  const byPath = new Map(entities.map((e) => [e.path, e]));
  const PW = "self-test-password", withPw = (body) => body.slice(0, -1) + `,"password":${JSON.stringify(PW)}}`;
  const pass = (p, r, u) => { try { return p.test(r, u) === true; } catch { return false; } };
  const allowRule = { text: "account may sign in", test: (r) => pass({ test: ACCOUNT.allowed }, r) };
  // dependency order: refs first
  const order = [], seen = new Set();
  const visit = (e, stack) => {
    if (seen.has(e.path)) return;
    if (stack.includes(e.path)) { fails.push(`FAIL ${e.name} synthesize: cyclic or self reference ${e.path}`); return; }
    for (const f of e.fields) if (f.type === "ref" && byPath.has(f.ref)) visit(byPath.get(f.ref), [...stack, e.path]);
    seen.add(e.path); order.push(e);
  };
  entities.forEach((e) => visit(e, []));
  const isAcc = (ent) => ACCOUNT && ent.path === ACCOUNT.entity;
  let roles = 0, persisted = 0, authNote = [], permEnts = 0;
  const perRole = new Map(); // role -> observed own, foreign, and unowned row actions
  const ownRows = [];
  const ev = new Map(), note = (k, v) => { if (!ev.has(k)) ev.set(k, v); }, cr = {}, trunc = (t, m) => (t.length > m ? t.slice(0, m - 3) + "..." : t);
  try {
    base = await start();
    const refs = {}, made = [];
    for (const ent of order) {
      const n = ent.name, s0 = isAcc(ent) ? rt.synthesizeRow(ent, enums, refs, 20000, { extra: [allowRule] }) : {};
      const s = s0.row ? s0 : rt.synthesizeRow(ent, enums, refs);
      if (!s.row) { fails.push(`FAIL ${n} synthesize: cannot synthesize a valid ${n}: ${short(s.fail)}`); break; }
      const row = s.row, body = rt.toJSON(ent, row);
      let r = await call("POST", ent.path, body), got = dec(ent, r.text);
      if (!check(n, "create", "201 + echo", `${r.status} ${r.text}`, r.status === 201 && same(ent, row, got))) continue;
      const id = got.id, mk = { ent, row, id }; refs[ent.path] = id; made.push(mk);
      const lead = !cr.path; if (lead) Object.assign(cr, { path: ent.path, id0: id, txt: `POST ${n} 201 #${id}` });
      r = await call("GET", `${ent.path}/${id}`);
      const gb = r.status === 200 ? rt.parseJSON(r.text) : {};
      check(n, "read", "200 + equal", `${r.status} ${r.text}`, r.status === 200 && same(ent, row, dec(ent, r.text)));
      for (const c of ent.computed || []) {
        if (!check(n, "computed", `field ${c.name}`, "missing", Object.hasOwn(gb, c.name))) continue;
        const v = gb[c.name];
        if (v !== null && typeof v === "object" && v.error) warns.push(`warning: ${n}.${c.name} computed error ${v.error}`);
      }
      if (lead) cr.txt += `, GET ${r.status}`;
      r = await call("GET", ent.path);
      if (lead) cr.txt += `, list ${r.status}`;
      check(n, "list", `200 containing id ${id}`, `${r.status} ${r.text}`, r.status === 200 && arr(r.text).some((o) => String(o.id) === String(id)));
      r = await call("PUT", `${ent.path}/${id}`, body);
      if (lead) cr.txt += `, PUT ${r.status}`;
      check(n, "update", "200 + equal", `${r.status} ${r.text}`, r.status === 200 && same(ent, row, dec(ent, r.text)));
      r = await call("POST", ent.path, "{}");
      let errs = []; try { errs = JSON.parse(r.text).errors || []; } catch {}
      if (errs.length) note("validate", `POST {} ${n} ${r.status} [${errs.slice(0, 2).map((e) => `${e.field}: ${e.message}`).join(", ")}${errs.length > 2 ? ", ..." : ""}]`);
      check(n, "validate", `400 with an error per field (${ent.fields.length})`, `${r.status} ${r.text}`, r.status === 400 && ent.fields.every((f) => errs.some((e) => e.field === f.name)));
      r = await call("GET", `${ent.path}/${id + 1000n}`);
      if (lead) cr.txt += `, DELETE @, GET #${id + 1000n} ${r.status}`;
      check(n, "404", "404", `${r.status} ${r.text}`, r.status === 404);
      // rule: mutate one field of the valid row so that one rule fails, and expect the server to reject it
      if ((ent.rules || []).length && !ev.has("rule")) {
        const alt = { string: ["", "a", "ab"], int: [-1n, 0n, 1n, 1000000n], float: [-1, 0, 1e6] };
        find: for (const rl of ent.rules) for (const fn of rl.fields || []) {
          const f = ent.fields.find((x) => x.name === fn);
          if (!f || !alt[f.type] || (ent.steps || []).some((st) => st.field === fn)) continue;
          for (const v of alt[f.type]) {
            const bad = { ...row, [fn]: v };
            if (v === row[fn] || !rt.evalRules(ent, bad).some((e) => e.message === rl.text)) continue;
            const rr = await call("POST", ent.path, rt.toJSON(ent, bad));
            let es = []; try { es = JSON.parse(rr.text).errors || []; } catch {}
            if (check(n, "rule", `400 naming rule ${rl.text}`, `${rr.status} ${rr.text}`, rr.status === 400 && es.some((e) => e.message === rl.text))) note("rule", `${n} ${rr.status} "${trunc(rl.text, 60)}"`);
            break find;
          }
        }
      }
      if (rt.keysOf(ent, ACCOUNT).length) {
        r = await call("POST", ent.path, body);
        check(n, "key", "400 for a duplicate key", `${r.status} ${r.text}`, r.status === 400 && /must be unique/.test(r.text));
        if (r.status === 400) note("key", `duplicate ${n}.${rt.keysOf(ent, ACCOUNT)[0].fields.join("+")} 400`);
      }
      for (const st of ent.steps || []) {
        const cases = rt.stepCases(ent, enums, st), from = mk.row[st.field];
        const other = (ok) => cases.find((to) => to !== from && rt.stepOk(st, from, to) === ok && (!ok || !rt.evalRules(ent, { ...mk.row, [st.field]: to }).length));
        const no = other(false), yes = other(true), wf = [];
        if (no !== undefined) {
          r = await call("PUT", `${ent.path}/${id}`, rt.toJSON(ent, { ...mk.row, [st.field]: no }));
          check(n, "workflow", `400 for ${st.field} ${from} -> ${no}`, `${r.status} ${r.text}`, r.status === 400);
          wf.push(`${from}->${no} ${r.status}`);
        }
        if (yes !== undefined) {
          const next = { ...mk.row, [st.field]: yes };
          r = await call("PUT", `${ent.path}/${id}`, rt.toJSON(ent, next));
          if (check(n, "workflow", `200 for ${st.field} ${from} -> ${yes}`, `${r.status} ${r.text}`, r.status === 200)) mk.row = next;
          wf.push(`${from}->${yes} ${r.status}`);
        }
        if (wf.length) note("workflow", `${n}.${st.field} ${wf.join(", ")}`);
      }
      r = await call("GET", `${ent.path}/${id}/history`);
      check(n, "audit", "200 with >= 2 history entries", `${r.status} ${r.text}`, r.status === 200 && arr(r.text).length >= 2);
      if (r.status === 200) note("audit", `${n} #${id} history ${arr(r.text).length} entries (${[...new Set(arr(r.text).map((h) => h.action))].join(", ")})`);
      r = await call("GET", `${ent.path}?format=csv`);
      const lines = r.status === 200 ? r.text.split("\r\n").filter(Boolean) : [], head = ["id", ...[...ent.fields, ...(ent.computed || [])].map((f) => f.name)].join(",");
      check(n, "csv", `200 text/csv, header ${head} + rows`, `${r.status} ${r.type} ${r.text}`, r.status === 200 && r.type.startsWith("text/csv") && lines[0] === head && lines.length >= 2);
      note("csv", `${n} ${lines.length} lines, header ${trunc(lines[0] || "", 60)}`);
    }
    if (!fails.length) {
      // rollups: computed values served for each parent equal a local evaluation over the rows created here
      const kids = (p) => ({ ent: byPath.get(p), rows: made.filter((m) => m.ent.path === p).map((m) => ({ ...m.row, id: m.id })) });
      for (const { ent, row, id } of made) if ((ent.rollups || []).length) {
        const loc = rt.withRollups(ent, { ...row, id }, kids), want = rt.toJSON(ent, loc, { computed: true }), r = await call("GET", `${ent.path}/${id}`);
        check(ent.name, "rollup", want, `${r.status} ${r.text}`, r.status === 200 && r.text === want);
        if (r.status === 200 && r.text === want) note("rollup", `${ent.name} #${id} ${ent.rollups.map((u) => `${u.name}=${loc[u.name]}`).slice(0, 3).join(", ")}`);
        for (const u of ent.rollups) if (u.kind === "count" && !u.field && made.some((m) => m.ent.path === u.child))
          check(ent.name, "rollup", `${u.name} >= 1`, String(loc[u.name]), loc[u.name] >= 1n);
      }
      const tried = new Set();
      for (const { ent, row } of made) for (const f of ent.fields) if (f.type === "ref") {
        const tg = made.find((m) => m.ent.path === f.ref);
        if (tg && !tried.has(f.ref)) {
          tried.add(f.ref);
          const r = await call("DELETE", `${f.ref}/${tg.id}`);
          check(tg.ent.name, "409", "409 while referenced", `${r.status} ${r.text}`, r.status === 409);
          note("reference", `DELETE ${tg.ent.name} #${tg.id} ${r.status} (${ent.name}.${f.name})`);
        }
        const r = await call("POST", ent.path, rt.toJSON(ent, { ...row, [f.name]: 999999n }));
        if (r.status === 400) note("refmiss", `missing ${ent.name}.${f.name} ${r.status}`);
        check(ent.name, "ref", `400 for missing ${f.name}`, `${r.status} ${r.text}`, r.status === 400);
      }
      await stop(); base = await start();
      for (const { ent, row, id } of made) {
        const r = await call("GET", `${ent.path}/${id}`);
        check(ent.name, "persist", "200 + equal after restart", `${r.status} ${r.text}`, r.status === 200 && same(ent, row, dec(ent, r.text)));
        if (r.status === 200 && same(ent, row, dec(ent, r.text))) persisted++;
      }
      const am = ACCOUNT && made.find((m) => isAcc(m.ent)), extra = [];
      if (am) {
        // one account per case of every enum field of the account entity (created in setup mode, without password)
        const ent = am.ent, n = ent.name, keys = rt.keysOf(ent, ACCOUNT), accts = [am];
        for (const f of ent.fields) if (f.type === "enum") {
          const first = (ent.steps || []).some((st) => st.field === f.name);
          for (const cs of (enums[f.enum] || []).slice(0, first ? 1 : undefined)) {
            if (accts.some((a) => a.row[f.name] === cs)) continue;
            const others = accts.map((a) => ({ ...a.row, id: a.id })), uniq = { text: "unique keys", test: (r) => !rt.keyErrors(keys, r, others).length };
            let s = rt.synthesizeRow(ent, enums, refs, 20000, { fixed: { [f.name]: cs }, extra: [uniq, allowRule], vary: true });
            if (!s.row) s = rt.synthesizeRow(ent, enums, refs, 20000, { fixed: { [f.name]: cs }, extra: [uniq], vary: true });
            if (!check(n, "auth", `an account with ${f.name}=${cs}`, s.fail, !!s.row)) continue;
            const r = await call("POST", ent.path, rt.toJSON(ent, s.row)), got = dec(ent, r.text);
            if (check(n, "auth", `201 for an account with ${f.name}=${cs}`, `${r.status} ${r.text}`, r.status === 201 && !!got)) { accts.push({ ent, row: s.row, id: got.id }); extra.push(accts.at(-1)); }
          }
        }
        // Seed policy fixtures while setup is still open; later checks always
        // go through the real sign-in, list, and PUT routes below.
        for (const acct of accts) {
          for (const rowEnt of entities) {
            const ownerFields = rowEnt.fields.filter((f) => f.type === "ref" && f.ref === ACCOUNT.entity);
            if (!ownerFields.length || !(rowEnt.canRead?.row || rowEnt.canWrite?.row)) continue;
            const fixed = Object.fromEntries(ownerFields.map((f) => [f.name, acct.id]));
            const existing = [...made, ...ownRows]
              .filter((m) => m.ent.path === rowEnt.path)
              .map((m) => ({ ...m.row, id: m.id }));
            const unique = { text: "unique keys", test: (r) => !rt.keyErrors(rt.keysOf(rowEnt, ACCOUNT), r, existing).length };
            const s = rt.synthesizeRow(rowEnt, enums, refs, 20000, { fixed, extra: [unique], vary: true });
            if (!check(rowEnt.name, "own-row synthesize", `row for ${ACCOUNT.login}=${acct.row[ACCOUNT.login]}`, s.fail, !!s.row)) continue;
            const r = await call("POST", rowEnt.path, rt.toJSON(rowEnt, s.row)), got = dec(rowEnt, r.text);
            if (check(rowEnt.name, "own-row create", "201 + echo", `${r.status} ${r.text}`, r.status === 201 && same(rowEnt, s.row, got))) {
              ownRows.push({ ent: rowEnt, row: s.row, id: got.id });
            }
          }
        }
        const login = (a, pw = PW) => JSON.stringify({ login: a.row[ACCOUNT.login], password: pw });
        let r = await call("PUT", `${ent.path}/${am.id}`, withPw(rt.toJSON(ent, am.row)));
        check(n, "auth", "200 setting a password", `${r.status} ${r.text}`, r.status === 200 && !r.text.includes(PW));
        r = await call("GET", ent.path);
        check(n, "auth", "401 without a session once a password exists", `${r.status} ${r.text}`, r.status === 401);
        if (r.status === 401) authNote.push("no session 401");
        r = await call("POST", "session", login(am, PW + "x"));
        check(n, "auth", "401 for a wrong password", `${r.status} ${r.text}`, r.status === 401);
        if (r.status === 401) authNote.push("wrong password 401");
        // the other accounts' passwords go straight into the data dir: no account need be allowed to write accounts
        await stop();
        const a = JSON.parse(fs.readFileSync(authFile, "utf8"));
        for (const x of extra) a.passwords[String(x.id)] = hashPw(PW);
        fixtureAuth(JSON.stringify(a));
        base = await start();
        for (const acct of accts) {
          const u = { ...acct.row, id: acct.id }, can = allowRule.test(u), what = `${ACCOUNT.login}=${u[ACCOUNT.login]}`;
          r = await call("POST", "session", login(acct));
          if (!check(n, "auth", `sign-in ${can ? 200 : 401} for ${what}`, `${r.status} ${r.text}`, r.status === (can ? 200 : 401)) || !can) continue;
          if (!authNote.includes("sign-in 200")) authNote.push("sign-in 200");
          check(n, "auth", "HttpOnly SameSite=Strict sid cookie", r.cookie, /HttpOnly/.test(r.cookie) && /SameSite=Strict/.test(r.cookie));
          const ck = "sid=" + ((/sid=([0-9a-f]{64})/.exec(r.cookie) || [])[1] || "");
          roles++; permEnts = made.length;
          r = await call("GET", "session", undefined, ck);
          check(n, "auth", `200 current account for ${what}`, `${r.status} ${r.text}`, r.status === 200 && (dec(ent, r.text) || {}).id === acct.id);
          for (const m of [...made, ...ownRows]) {
            const mr = { ...m.row, id: m.id }, read = !m.ent.canRead || pass(m.ent.canRead, mr, u), write = !m.ent.canWrite || pass(m.ent.canWrite, mr, u);
            r = await call("GET", m.ent.path, undefined, ck);
            const vis = arr(r.text).some((o) => String(o.id) === String(m.id));
            check(m.ent.name, "permissions", `${what}: list ${read ? "shows" : "hides"} row ${m.id}`, `${r.status} ${r.text}`, r.status === 200 && vis === read);
            const role = ent.fields.filter((x) => x.type === "enum").map((x) => u[x.name]).slice(0, 1)[0] ?? what;
            r = await call("PUT", `${m.ent.path}/${m.id}`, rt.toJSON(m.ent, m.row), ck);
            const want = !read ? 404 : write ? 200 : 403;
            check(m.ent.name, "permissions", `${what}: PUT ${want}`, `${r.status} ${r.text}`, r.status === want);
            const tally = perRole.get(role) ?? perRole.set(role, {
              own: { hidden: [], denied: [], writes: [] },
              foreign: { hidden: [], denied: [], writes: [] },
              unowned: { hidden: [], denied: [], writes: [] },
            }).get(role);
            const ownerFields = m.ent.fields.filter((f) => f.type === "ref" && f.ref === ACCOUNT.entity);
            const isOwn = ownerFields.length > 0 && ownerFields.every((f) => String(m.row[f.name]) === String(acct.id));
            const actions = ownerFields.length ? (isOwn ? tally.own : tally.foreign) : tally.unowned;
            if (r.status === want) {
              if (!read) actions.hidden.push(m.ent.name);
              else if (write) actions.writes.push(m.ent.name);
              else actions.denied.push(m.ent.name);
            }
            if (!read) note("permRead", `${role} list ${m.ent.name} hides #${m.id}`);
            else if (!write && r.status === 403) note("permWrite", `${role} PUT ${m.ent.name} #${m.id} 403`);
          }
          r = await call("DELETE", "session", undefined, ck);
          check(n, "auth", "204 sign-out", `${r.status} ${r.text}`, r.status === 204);
          r = await call("GET", "session", undefined, ck);
          check(n, "auth", "401 after sign-out", `${r.status} ${r.text}`, r.status === 401);
          if (r.status === 401 && !authNote.includes("sign-out then 401")) authNote.push("sign-out then 401");
        }
        await stop(); fixtureAuth(JSON.stringify({ version: 1, passwords: {}, sessions: {} })); base = await start(); // back to setup mode for the deletes
      }
      for (const { ent, id } of [...[...ownRows].reverse(), ...[...extra].reverse(), ...[...made].reverse()]) {
        const r = await call("DELETE", `${ent.path}/${id}`);
        check(ent.name, "delete", "204", `${r.status} ${r.text}`, r.status === 204);
        if (ent.path === cr.path && id === cr.id0) cr.txt = cr.txt.replace("DELETE @", `DELETE ${r.status}`);
      }
    }
  } catch (e) { fails.push("FAIL self-test: " + short(e.message)); }
  finally { await stop(); if (!given) fs.rmSync(dir, { recursive: true, force: true }); }
  // Observed, not assumed: the test server exited and its data is gone.
  const stopped = !child || child.exitCode !== null || child.signalCode !== null;
  const cleaned = given || !fs.existsSync(dir);
  const sum = (k) => entities.reduce((a, e) => a + (k === "keys" ? rt.keysOf(e, ACCOUNT) : e[k] || []).length, 0);
  if (fails.length) { console.log(fails.join("\n") + `\nself-test failed: ${fails.length} check(s)`); return 1; }
  console.log(`self-test ok: ${entities.length} entities, ${sum("rules")} rules, ${sum("computed")} computed, ${sum("keys")} keys, ${sum("steps")} workflows, ${sum("rollups")} rollups` +
    (ACCOUNT ? `, accounts (${roles} roles)` : "") + ` (create, read, list, update, validate, 404, key, workflow, audit, csv, rollup, 409, persist${ACCOUNT ? ", auth, permissions" : ""}, delete)`);
  const lines = [], put = (k, v) => { if (v) lines.push(`${k}: ${v}`); };
  put("crud", cr.txt); put("validate", ev.get("validate")); put("rule", ev.get("rule")); put("key", ev.get("key")); put("workflow", ev.get("workflow"));
  put("rollup", ev.get("rollup")); put("reference", ev.get("reference") && ev.get("reference") + (ev.get("refmiss") ? ", " + ev.get("refmiss") : "")); put("audit", ev.get("audit"));
  put("csv", ev.get("csv")); put("persist", persisted && `${persisted} rows identical after restart`);
  if (ACCOUNT) {
    put("auth", authNote.join(", "));
    put("permissions", `${roles} roles x ${permEnts} entities plus ${ownRows.length} own-row fixtures agree with schema` + [ev.get("permWrite"), ev.get("permRead")].filter(Boolean).map((x, i) => (i ? ", " : "; e.g. ") + x).join(""));
    const names = (xs) => {
      const selected = new Set(xs);
      return entities.filter((e) => selected.has(e.name)).map((e) => e.name).join(" ") || "none";
    };
    for (const [role, t] of perRole) {
      put(`  ${role} on own-account rows`, `hidden: ${names(t.own.hidden)}; denied writes: ${names(t.own.denied)}; writes: ${names(t.own.writes)}`);
      put(`  ${role} on other-account rows`, `hidden: ${names(t.foreign.hidden)}; denied writes: ${names(t.foreign.denied)}; writes: ${names(t.foreign.writes)}`);
      put(`  ${role} on unowned rows`, `hidden: ${names(t.unowned.hidden)}; denied writes: ${names(t.unowned.denied)}; writes: ${names(t.unowned.writes)}`);
    }
  }
  put("cleanup", `${stopped ? "test server stopped, no process left running" : "test server STILL RUNNING"}, ${given ? "data kept in --data dir" : cleaned ? "temporary data removed" : "temporary data NOT removed"}`);
  if (lines.length) console.log(lines.join("\n"));
  if (warns.length) console.log(warns.join("\n"));
  return 0;
}
