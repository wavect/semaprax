// node server.mjs [--port N] [--host 127.0.0.1] [--data DIR]
// node server.mjs --self-test [--data DIR]   (verify the whole app against its schema; exit 0/1)
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { entities, enums, app } from "./schema.js";
import * as rt from "./runtime.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const opt = { port: "8080", host: "127.0.0.1", data: "./data" };
const argv = process.argv.slice(2), SELF = argv.includes("--self-test");
if (SELF) argv.splice(argv.indexOf("--self-test"), 1);
for (let i = 0; i < argv.length; i += 2) {
  const k = argv[i].replace(/^--/, "");
  if (!(k in opt) || argv[i + 1] === undefined || (SELF && k !== "data")) { console.error("usage: node server.mjs [--port N] [--host H] [--data DIR] | --self-test [--data DIR]"); process.exit(2); }
  opt[k] = argv[i + 1];
}
if (SELF) process.exit(await selfTest());
const LIMIT = 1 << 20;
const dbFile = path.join(path.resolve(opt.data), "db.json");
fs.mkdirSync(path.dirname(dbFile), { recursive: true });

// ---- state: path -> {ent, rows: Map<BigInt id,row>, next: BigInt} ----
const tables = new Map(entities.map((ent) => [ent.path, { ent, rows: new Map(), next: 1n }]));
function load() {
  if (!fs.existsSync(dbFile)) return;
  const db = rt.parseJSON(fs.readFileSync(dbFile, "utf8"));
  for (const [p, t] of tables) {
    for (const o of (db.rows && db.rows[p]) || []) {
      const { row, errors } = rt.decodeRow(t.ent, enums, o, true);
      if (errors.length || row.id === undefined) throw new Error(`corrupt ${dbFile}: ${p}: ${JSON.stringify(errors)}`);
      t.rows.set(row.id, row);
      if (row.id >= t.next) t.next = row.id + 1n;
    }
    const n = db.next && db.next[p] !== undefined ? BigInt(String(db.next[p].source ?? db.next[p])) : 1n;
    if (n > t.next) t.next = n;
  }
}
function save() {
  const rows = [], next = [];
  for (const [p, t] of tables) {
    rows.push(JSON.stringify(p) + ":[" + [...t.rows.values()].map((r) => rt.toJSON(t.ent, r, { strInts: true })).join(",") + "]");
    next.push(JSON.stringify(p) + ":" + JSON.stringify(t.next.toString()));
  }
  const tmp = dbFile + ".tmp";
  fs.writeFileSync(tmp, `{"version":1,"next":{${next.join(",")}},"rows":{${rows.join(",")}}}\n`);
  fs.renameSync(tmp, dbFile);
}
load();

// ---- http helpers ----
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".mjs": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8" };
const STATIC = new Set(["index.html", "app.js", "style.css", "schema.js", "runtime.js"]);
const send = (res, code, body = "", type = "application/json; charset=utf-8", extra = {}) => {
  res.writeHead(code, { "content-type": type, "cache-control": "no-store", "x-content-type-options": "nosniff", ...extra });
  res.end(body);
};
const fail = (res, code, msg) => send(res, code, JSON.stringify({ error: msg }));
const verrs = (res, errors) => send(res, 400, JSON.stringify({ errors }));
const out = (t, row) => rt.toJSON(t.ent, row, { computed: true });
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
function validate(t, input) {
  if (input === null || typeof input !== "object" || Array.isArray(input)) return { errors: [{ field: "", message: "body must be a JSON object" }] };
  const { row, errors, bad } = rt.decodeRow(t.ent, enums, input);
  for (const f of t.ent.fields) {
    if (f.type === "ref" && !bad.has(f.name) && !(tables.get(f.ref) && tables.get(f.ref).rows.has(row[f.name])))
      errors.push({ field: f.name, message: `${f.name} must reference an existing ${f.ref} row (id ${row[f.name]} not found)` });
  }
  errors.push(...rt.evalRules(t.ent, row, bad));
  return { row, errors };
}
function referrer(t, id) {
  for (const o of tables.values())
    for (const f of o.ent.fields)
      if (f.type === "ref" && f.ref === t.ent.path)
        for (const r of o.rows.values()) if (r[f.name] === id && !(o === t && r.id === id)) return `cannot delete ${t.ent.name} ${id}: referenced by ${o.ent.name} ${r.id} (field ${f.name})`;
  return null;
}
function persist(res, undo) {
  try { save(); return true; } catch (e) { undo(); fail(res, 500, "could not persist: " + e.message); return false; }
}

async function api(req, res, parts) {
  const t = tables.get(parts[0]);
  if (!t) return fail(res, 404, "unknown entity");
  const m = req.method, hasId = parts.length === 2;
  if (parts.length > 2 || (hasId && !/^[1-9]\d{0,18}$/.test(parts[1]))) return fail(res, 404, "not found");
  const id = hasId ? BigInt(parts[1]) : null;
  const cur = hasId ? t.rows.get(id) : null;
  if (!hasId) {
    if (m === "GET") return send(res, 200, "[" + [...t.rows.values()].map((r) => out(t, r)).join(",") + "]");
    if (m !== "POST") return fail(res, 405, "method not allowed");
  } else {
    if (m !== "GET" && m !== "PUT" && m !== "DELETE") return fail(res, 405, "method not allowed");
    if (!cur) return fail(res, 404, `${t.ent.name} ${id} not found`);
    if (m === "GET") return send(res, 200, out(t, cur));
    if (m === "DELETE") {
      const why = referrer(t, id);
      if (why) return fail(res, 409, why);
      t.rows.delete(id);
      if (!persist(res, () => t.rows.set(id, cur))) return;
      return send(res, 204);
    }
  }
  const text = await readBody(req, res);
  if (text === null) return;
  let input;
  try { input = rt.parseJSON(text); } catch { return verrs(res, [{ field: "", message: "body is not valid JSON" }]); }
  const { row, errors } = validate(t, input);
  if (errors.length) return verrs(res, errors);
  if (hasId) {
    row.id = id; t.rows.set(id, row);
    if (!persist(res, () => t.rows.set(id, cur))) return;
    return send(res, 200, out(t, row));
  }
  row.id = t.next; t.next += 1n; t.rows.set(row.id, row);
  if (!persist(res, () => { t.rows.delete(row.id); t.next = row.id; })) return;
  send(res, 201, out(t, row), undefined, { location: `/api/${t.ent.path}/${row.id}` });
}

const server = http.createServer((req, res) => {
  let parts;
  try { parts = new URL(req.url, "http://x").pathname.split("/").filter(Boolean).map(decodeURIComponent); } catch { return fail(res, 404, "not found"); }
  if (parts[0] === "api") return api(req, res, parts.slice(1)).catch((e) => { console.error(e); if (!res.headersSent) fail(res, 500, "internal error"); });
  const name = parts.length === 0 ? "index.html" : parts.length === 1 ? parts[0] : "";
  if ((req.method === "GET" || req.method === "HEAD") && STATIC.has(name)) {
    return fs.readFile(path.join(here, name), (e, buf) => (e ? fail(res, 404, "not found") : send(res, 200, req.method === "HEAD" ? "" : buf, TYPES[path.extname(name)])));
  }
  fail(res, 404, "not found");
});
server.listen(Number(opt.port), opt.host, () => console.log(`${app.title} listening on http://${opt.host}:${server.address().port}/ (data: ${dbFile})`));

// ---- --self-test: run the real server as a child on port 0, drive it from the schema alone ----
async function selfTest() {
  const given = argv.includes("--data");
  if (given && fs.existsSync(opt.data) && fs.readdirSync(opt.data).length) { console.error(`self-test: refusing non-empty data dir ${opt.data}`); return 1; }
  const dir = given ? path.resolve(opt.data) : fs.mkdtempSync(path.join(os.tmpdir(), "selftest-"));
  const fails = [], warns = [], short = (v) => { const s = typeof v === "string" ? v : String(v); return s.length > 120 ? s.slice(0, 120) + "..." : s; };
  const check = (ent, what, want, got, ok) => { if (!ok) fails.push(`FAIL ${ent} ${what}: ${want} got ${short(got)}`); return ok; };
  let child = null;
  const start = () => new Promise((resolve, reject) => {
    child = spawn(process.execPath, [fileURLToPath(import.meta.url), "--port", "0", "--data", dir], { stdio: ["ignore", "pipe", "pipe"] });
    let buf = "";
    const timer = setTimeout(() => reject(new Error("server did not start: " + buf)), 10000);
    child.stderr.on("data", (c) => { buf += c; });
    child.stdout.on("data", (c) => { buf += c; const m = /listening on (http:\/\/\S+?)\/ /.exec(buf); if (m) { clearTimeout(timer); resolve(m[1] + "/api/"); } });
    child.on("exit", () => { clearTimeout(timer); reject(new Error("server exited: " + buf)); });
  });
  const stop = () => new Promise((resolve) => { if (!child || child.exitCode !== null) return resolve(); child.removeAllListeners("exit"); child.on("exit", resolve); child.kill("SIGTERM"); });
  let base;
  const call = async (method, p, body) => {
    try {
      const r = await fetch(base + p, { method, headers: body === undefined ? {} : { "content-type": "application/json" }, body });
      return { status: r.status, text: await r.text() };
    } catch (e) { return { status: 0, text: String(e) }; }
  };
  // body text -> decoded row (with id), or null
  const dec = (ent, text) => { try { const d = rt.decodeRow(ent, enums, rt.parseJSON(text), true); return d.errors.length ? null : d.row; } catch { return null; } };
  const same = (ent, a, b) => !!b && ent.fields.every((f) => a[f.name] === b[f.name]);
  const byPath = new Map(entities.map((e) => [e.path, e]));
  // dependency order: refs first
  const order = [], seen = new Set();
  const visit = (e, stack) => {
    if (seen.has(e.path)) return;
    if (stack.includes(e.path)) { fails.push(`FAIL ${e.name} synthesize: cyclic or self reference ${e.path}`); return; }
    for (const f of e.fields) if (f.type === "ref" && byPath.has(f.ref)) visit(byPath.get(f.ref), [...stack, e.path]);
    seen.add(e.path); order.push(e);
  };
  entities.forEach((e) => visit(e, []));
  try {
    base = await start();
    const refs = {}, made = [];
    for (const ent of order) {
      const n = ent.name, s = rt.synthesizeRow(ent, enums, refs);
      if (!s.row) { fails.push(`FAIL ${n} synthesize: cannot synthesize a valid ${n}: ${short(s.fail)}`); break; }
      const row = s.row, body = rt.toJSON(ent, row);
      let r = await call("POST", ent.path, body), got = dec(ent, r.text);
      if (!check(n, "create", "201 + echo", `${r.status} ${r.text}`, r.status === 201 && same(ent, row, got))) continue;
      const id = got.id; refs[ent.path] = id; made.push({ ent, row, id });
      r = await call("GET", `${ent.path}/${id}`);
      const gb = r.status === 200 ? rt.parseJSON(r.text) : {};
      check(n, "read", "200 + equal", `${r.status} ${r.text}`, r.status === 200 && same(ent, row, dec(ent, r.text)));
      for (const c of ent.computed || []) {
        if (!check(n, "computed", `field ${c.name}`, "missing", Object.hasOwn(gb, c.name))) continue;
        const v = gb[c.name];
        if (v !== null && typeof v === "object" && v.error) warns.push(`warning: ${n}.${c.name} computed error ${v.error}`);
      }
      r = await call("GET", ent.path);
      let list = []; try { list = JSON.parse(r.text); } catch {}
      check(n, "list", `200 containing id ${id}`, `${r.status} ${r.text}`, r.status === 200 && Array.isArray(list) && list.some((o) => String(o.id) === String(id)));
      r = await call("PUT", `${ent.path}/${id}`, body);
      check(n, "update", "200 + equal", `${r.status} ${r.text}`, r.status === 200 && same(ent, row, dec(ent, r.text)));
      r = await call("POST", ent.path, "{}");
      let errs = []; try { errs = JSON.parse(r.text).errors || []; } catch {}
      check(n, "validate", `400 with an error per field (${ent.fields.length})`, `${r.status} ${r.text}`, r.status === 400 && ent.fields.every((f) => errs.some((e) => e.field === f.name)));
      r = await call("GET", `${ent.path}/${id + 1000n}`);
      check(n, "404", "404", `${r.status} ${r.text}`, r.status === 404);
    }
    if (!fails.length) {
      const tried = new Set();
      for (const { ent, row } of made) for (const f of ent.fields) if (f.type === "ref") {
        const tg = made.find((m) => m.ent.path === f.ref);
        if (tg && !tried.has(f.ref)) {
          tried.add(f.ref);
          const r = await call("DELETE", `${f.ref}/${tg.id}`);
          check(tg.ent.name, "409", "409 while referenced", `${r.status} ${r.text}`, r.status === 409);
        }
        const r = await call("POST", ent.path, rt.toJSON(ent, { ...row, [f.name]: 999999n }));
        check(ent.name, "ref", `400 for missing ${f.name}`, `${r.status} ${r.text}`, r.status === 400);
      }
      await stop(); base = await start();
      for (const { ent, row, id } of made) {
        const r = await call("GET", `${ent.path}/${id}`);
        check(ent.name, "persist", "200 + equal after restart", `${r.status} ${r.text}`, r.status === 200 && same(ent, row, dec(ent, r.text)));
      }
      for (const { ent, id } of [...made].reverse()) {
        const r = await call("DELETE", `${ent.path}/${id}`);
        check(ent.name, "delete", "204", `${r.status} ${r.text}`, r.status === 204);
      }
    }
  } catch (e) { fails.push("FAIL self-test: " + short(e.message)); }
  finally { await stop(); if (!given) fs.rmSync(dir, { recursive: true, force: true }); }
  const rules = entities.reduce((a, e) => a + (e.rules || []).length, 0), comp = entities.reduce((a, e) => a + (e.computed || []).length, 0);
  if (fails.length) { console.log(fails.join("\n") + `\nself-test failed: ${fails.length} check(s)`); return 1; }
  console.log(`self-test ok: ${entities.length} entities, ${rules} rules, ${comp} computed (create, read, list, update, validate, 404, 409, persist, delete)`);
  if (warns.length) console.log(warns.join("\n"));
  return 0;
}
