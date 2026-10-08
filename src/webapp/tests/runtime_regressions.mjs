// Black-box witnesses against compiler output, plus focused shared-runtime controls.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import http from "node:http";
import { spawn } from "node:child_process";
import { pathToFileURL } from "node:url";
const app = path.resolve(process.argv[2]);
const rt = await import(pathToFileURL(path.join(app, "runtime.js")));
const root = fs.mkdtempSync(path.join(os.tmpdir(), "semaprax-sg-http-"));
const data = path.join(root, "data");
const children = new Set();
const start = (dir = data, preload = null) => new Promise((resolve, reject) => {
  const child = spawn(process.execPath, [...(preload ? ["--import", preload] : []), path.join(app, "server.mjs"), "--port", "0", "--data", dir, "--setup"], { stdio: ["ignore", "pipe", "pipe"] });
  children.add(child); let output = "", errors = "";
  const timer = setTimeout(() => { child.kill(); reject(new Error("server startup timeout: " + errors)); }, 10000);
  child.stderr.on("data", (s) => errors += s);
  child.stdout.on("data", (s) => {
    output += s;
    const m = output.match(/listening on http:\/\/127\.0\.0\.1:(\d+)/);
    if (m) { clearTimeout(timer); resolve({ child, url: `http://127.0.0.1:${m[1]}` }); }
  });
  child.once("exit", (code) => { children.delete(child); clearTimeout(timer); reject(new Error(`startup exit ${code}: ${errors}`)); });
});
const stop = async (server) => {
  if (server.child.exitCode !== null) return;
  await new Promise((resolve) => { server.child.once("exit", resolve); server.child.kill("SIGTERM"); });
};
let server, cookie;
const mutationHeaders = async (auth) => {
  const response = await fetch(server.url + "/api/session/csrf", { headers: auth ? { cookie: auth } : {} });
  assert.equal(response.status, 200);
  const seed = response.headers.get("set-cookie").split(";")[0];
  return { cookie: [auth, seed].filter(Boolean).join("; "), "x-csrf-token": (await response.json()).token };
};
const call = async (method, route, body, auth = cookie) => {
  const authorization = ["GET", "HEAD"].includes(method) ? (auth ? { cookie: auth } : {}) : await mutationHeaders(auth);
  const response = await fetch(server.url + route, { method, headers: { ...(body ? { "content-type": "application/json" } : {}), ...authorization }, body: body === undefined ? undefined : JSON.stringify(body) });
  const text = await response.text();
  return { status: response.status, value: text ? JSON.parse(text) : null, location: response.headers.get("location"), cookie: response.headers.get("set-cookie")?.split(";")[0] };
};
const hold = async (route, body, auth = cookie) => {
  const authorization = await mutationHeaders(auth);
  return new Promise((resolve, reject) => {
  const bytes = JSON.stringify(body);
  const req = http.request(server.url + route, { method: "PUT", headers: { Expect: "100-continue", "content-length": Buffer.byteLength(bytes), "content-type": "application/json", ...authorization } });
  const result = new Promise((done, fail) => {
    req.on("response", (res) => { let text = ""; res.on("data", (s) => text += s); res.on("end", () => done({ status: res.statusCode, value: text ? JSON.parse(text) : null })); });
    req.on("error", fail);
  });
  req.on("error", reject);
  req.once("continue", () => resolve({ release: () => { req.end(bytes); return result; } }));
  req.flushHeaders();
  });
};
const item = (state = "Draft", user_id = 1) => ({ user_id, state, text: "retained" });
try {
  server = await start();
  const created = await call("POST", "/api/user", { email: "admin@example.test", active: true, admin: true, password: "password123" });
  assert.equal(created.status, 201);
  assert.equal((await fetch(server.url + "/api/session", { method: "DELETE" })).status, 401, "anonymous identity is checked before CSRF on sign-out");
  cookie = (await call("POST", "/api/session", { email: "admin@example.test", password: "password123" }, null)).cookie;
  assert.ok(cookie);
  assert.equal((await call("POST", "/api/user", { email: "other@example.test", active: true, admin: false, password: "password123" })).status, 201);
  const otherCookie = (await call("POST", "/api/session", { login: "other@example.test", password: "password123" }, null)).cookie;
  // Browser controls retain decimal-string decoding, while the JSON API
  // admits only number tokens for integer, float, and reference fields.
  assert.equal(rt.decodeValue({ type: "int" }, {}, "12")[0], 12n);
  assert.equal(rt.decodeValue({ type: "float" }, {}, "1.25")[0], 1.25);
  for (const [route, body, field] of [
    ["/api/number", { value: "12" }, "value"],
    ["/api/decimal", { amount: "1.25" }, "amount"],
    ["/api/item", item("Draft", "1"), "user_id"],
  ]) {
    const rejected = await call("POST", route, body);
    assert.equal(rejected.status, 400);
    assert.equal(rejected.value.errors[0].field, field);
  }
  // Postcondition values and precondition precedence are observable computed errors.
  for (const [value, expected] of [[7, 7], [0, { error: "postcondition" }], [-1, { error: "precondition" }]]) {
    const r = await call("POST", "/api/number", { value });
    assert.equal(r.status, 201); assert.deepEqual(r.value.checked, expected);
    assert.deepEqual(r.value.fault, { error: value < 0 ? "precondition" : "division_by_zero" });
    assert.equal(r.value.id, Number(r.location.split("/").at(-1)));
    assert.equal((await call("GET", r.location)).status, 200);
  }
  // Failed unique-key evaluations reject before persistence or audit.
  const good = await call("POST", "/api/keyed", { numerator: 1, denominator: 1 });
  assert.equal(good.status, 201);
  assert.equal((await call("POST", "/api/keyed", { numerator: 1, denominator: 1 })).status, 400);
  for (const body of [{ numerator: 1, denominator: 0 }, { numerator: "-9223372036854775808", denominator: -1 }]) {
    const r = await call("POST", "/api/keyed", body);
    assert.equal(r.status, 400); assert.match(r.value.errors[0].message, /division_by_zero|overflow/);
  }
  assert.equal((await call("GET", "/api/keyed")).value.length, 1);
  const ks = [{ name: "fixture", fields: ["n"], value: (r) => { rt.precondition(r.n > 0n); return r.n; } }];
  assert.match(rt.keyErrors(ks, { n: 0n }, [])[0].message, /precondition/);
  assert.match(rt.keyErrors(ks, { n: 2n }, [{ id: 1n, n: 0n }])[0].message, /stored key.*precondition/);
  assert.equal(rt.keyErrors(ks, {}, [], new Set(["n"])).length, 0);
  // Exact own-key semantics through create/update/persistence/restart.
  const proto = JSON.parse('{"__proto__":"ordinary text","constructor":"ctor","toString":"text"}');
  const p = await call("POST", "/api/proto", proto);
  assert.equal(p.status, 201); assert.equal(p.value.__proto__, "ordinary text");
  proto.__proto__ = "updated text";
  assert.equal((await call("PUT", p.location, proto)).value.__proto__, "updated text");
  assert.equal(rt.decodeRow({ fields: [{ name: "constructor", type: "string" }] }, {}, {}).errors.length, 1);
  const computed = rt.evalComputed({ computed: [{ name: "__proto__", value: () => "computed" }] }, {});
  assert.equal(computed.__proto__, "computed"); assert.equal(Object.getPrototypeOf(computed), null);
  // Held PUT sees deletion and current terminal workflow state.
  let row = await call("POST", "/api/item", item());
  let paused = await hold(row.location, item("Approved"));
  assert.equal((await call("DELETE", row.location)).status, 204);
  assert.equal((await paused.release()).status, 404);
  assert.equal((await call("GET", row.location)).status, 404);
  row = await call("POST", "/api/item", item());
  paused = await hold(row.location, item("Approved"));
  assert.equal((await call("PUT", row.location, item("Cancelled"))).status, 200);
  assert.equal((await paused.release()).status, 400);
  assert.equal((await call("GET", row.location)).value.state, "Cancelled");
  // Owner transfer, sign-out, deactivation and role change invalidate held authorization.
  row = await call("POST", "/api/item", item("Draft", 2));
  paused = await hold(row.location, item("Draft", 2), otherCookie);
  assert.equal((await call("PUT", row.location, item("Draft", 1))).status, 200);
  assert.equal((await paused.release()).status, 403);
  row = await call("POST", "/api/item", item("Draft", 2));
  paused = await hold(row.location, item("Draft", 2), otherCookie);
  assert.equal((await call("DELETE", "/api/session", undefined, otherCookie)).status, 204);
  assert.equal((await paused.release()).status, 401);
  const again = (await call("POST", "/api/session", { login: "other@example.test", password: "password123" }, null)).cookie;
  paused = await hold(row.location, item("Draft", 2), again);
  assert.equal((await call("PUT", "/api/user/2", { email: "other@example.test", active: false, admin: false })).status, 200);
  assert.equal((await paused.release()).status, 401);
  row = await call("POST", "/api/item", item("Draft", 2));
  paused = await hold(row.location, item("Draft", 2));
  assert.equal((await call("PUT", "/api/user/1", { email: "admin@example.test", active: true, admin: false })).status, 200);
  assert.equal((await paused.release()).status, 403);
  assert.equal((await call("PUT", "/api/user/1", { email: "admin@example.test", active: true, admin: true })).status, 200);
  // Canonical alias cannot admit a second stale writer. Separate directory works.
  const alias = path.join(root, "alias"); fs.symlinkSync(data, alias, "dir");
  await assert.rejects(start(alias), /writer claim/);
  const independent = await start(path.join(root, "other-data")); await stop(independent);
  // Failure at any mirror staging destination leaves state and audit unchanged.
  for (const name of ["audit.jsonl", "db.json", "auth.json"]) {
    const file = path.join(data, name), backup = file + ".backup";
    const before = fs.readFileSync(path.join(data, "state.json"), "utf8");
    fs.renameSync(file, backup); fs.mkdirSync(file);
    assert.equal((await call("POST", "/api/number", { value: 9 })).status, 500);
    assert.equal(fs.readFileSync(path.join(data, "state.json"), "utf8"), before);
    fs.rmdirSync(file); fs.renameSync(backup, file);
  }
  const persisted = await call("POST", "/api/number", { value: 10 });
  assert.equal(persisted.status, 201);
  const history = (await call("GET", persisted.location + "/history")).value;
  assert.equal(history.length, 1);
  await stop(server);
  // Publication snapshot repairs missing mirrors without replaying a mutation twice.
  for (const file of ["db.json", "auth.json", "audit.jsonl"]) fs.unlinkSync(path.join(data, file));
  server = await start();
  assert.equal((await call("GET", persisted.location)).value.value, 10);
  assert.equal((await call("GET", persisted.location + "/history")).value.length, 1);
  assert.equal((await call("GET", p.location)).value.__proto__, "updated text");
  await stop(server); server = await start();
  assert.equal((await call("GET", persisted.location + "/history")).value.length, 1);
  // Inject a filesystem failure after the publication pivot, without changing
  // the generated runtime. The response explicitly reports committed uncertainty.
  await stop(server);
  const injection = path.join(root, "fail-mirror.mjs");
  fs.writeFileSync(injection, `import fs from "node:fs";
const original = fs.renameSync; let armed = false;
fs.renameSync = function(from, to) {
  if (String(to).endsWith("state.json")) armed = true;
  if (armed && String(to).endsWith("audit.jsonl")) { armed = false; throw new Error("injected post-publication mirror failure"); }
  return original.call(this, from, to);
};`);
  server = await start(data, pathToFileURL(injection).href);
  const uncertain = await call("POST", "/api/number", { value: 11 });
  assert.equal(uncertain.status, 503); assert.equal(uncertain.value.committed, true);
  await stop(server); server = await start();
  const recovered = (await call("GET", "/api/number")).value.find((r) => r.value === 11);
  assert.ok(recovered);
  assert.equal((await call("GET", `/api/number/${recovered.id}/history`)).value.length, 1);
  // Whole-state validation rejects existing stored key failures at startup,
  // before any HTTP mutation or new audit can be admitted.
  await stop(server);
  const snapshotPath = path.join(data, "state.json"), originalSnapshot = fs.readFileSync(snapshotPath, "utf8");
  const invalidSnapshot = JSON.parse(originalSnapshot), invalidDb = JSON.parse(invalidSnapshot.db);
  invalidDb.rows.keyed[0].denominator = "0";
  invalidSnapshot.db = JSON.stringify(invalidDb);
  fs.writeFileSync(snapshotPath, JSON.stringify(invalidSnapshot));
  await assert.rejects(start(), /stored constraints fail keyed:.*division_by_zero/s);
  assert.equal(JSON.parse(fs.readFileSync(snapshotPath, "utf8")).db, invalidSnapshot.db);
  await stop(server); fs.writeFileSync(snapshotPath, originalSnapshot); server = await start();
  // Crash recovery is deliberately conservative, and never steals a live claim.
  await new Promise((resolve) => { server.child.once("exit", resolve); server.child.kill("SIGKILL"); });
  await assert.rejects(start(), /writer claim/);
  const owner = JSON.parse(fs.readFileSync(path.join(data, ".writer-lock/owner.json"), "utf8"));
  assert.throws(() => process.kill(owner.pid, 0));
  fs.rmSync(path.join(data, ".writer-lock"), { recursive: true });
  server = await start();
  assert.equal((await call("GET", persisted.location + "/history")).value.length, 1);
  console.log("SG webapp runtime regressions passed (local Node HTTP/filesystem only)");
} finally {
  await Promise.all([...children].map((child) => stop({ child })));
  fs.rmSync(root, { recursive: true, force: true });
}
