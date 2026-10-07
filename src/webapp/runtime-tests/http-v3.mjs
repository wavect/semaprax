import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
const here = path.dirname(fileURLToPath(import.meta.url)), data = path.join(here, "http-data");
fs.mkdirSync(data);
fs.writeFileSync(path.join(here, "schema.js"), `export const app={title:"Contracts"}; export const enums={}; export const account={entity:"account",login:"name",allowed:()=>true}; export const entities=[{name:"Account",path:"account",fields:[{name:"name",type:"string"}]},{name:"Booking",path:"booking",fields:[{name:"room",type:"int"},{name:"start",type:"int"},{name:"end",type:"int"}],constraints:[{name:"no_overlap",other:"booking",fields:["start"],test:(r,o)=>r.room!==o.room||r.end<=o.start||r.start>=o.end}]}];`);
const child = spawn(process.execPath, [path.join(here, "server.mjs"), "--port", "0", "--data", data, "--setup"], { stdio: ["ignore", "pipe", "pipe"] });
let diagnostics = ""; child.stderr.on("data", (c) => { diagnostics += c; });
try {
  const base = await new Promise((resolve, reject) => {
    let output = "";
    const timer = setTimeout(() => reject(new Error("startup timeout " + diagnostics)), 10000);
    child.on("exit", () => { clearTimeout(timer); reject(new Error("startup exit " + diagnostics)); });
    child.stdout.on("data", (c) => { output += c; const match = /listening on (http:\/\/[^/]+)\//.exec(output); if (match) { clearTimeout(timer); resolve(match[1]); } });
  });
  let sid = "", seed = "", token = "";
  const refresh = async () => {
    const response = await fetch(base + "/api/session/csrf", { headers: sid ? { cookie: sid } : {} });
    seed = response.headers.get("set-cookie").split(";")[0]; token = (await response.json()).token;
  };
  const call = (method, route, body, csrf = token) => fetch(base + "/api/" + route, { method, headers: { cookie: [sid, seed].filter(Boolean).join("; "), "x-csrf-token": csrf, "content-type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body) });
  await refresh();
  assert.equal((await call("POST", "account", { name: "owner", password: "long-password" }, "")).status, 403);
  assert.equal((await call("POST", "account", { name: "owner", password: "long-password" })).status, 201);
  assert.equal((await call("GET", "booking")).status, 401);
  const response = await call("POST", "session", { login: "owner", password: "long-password" });
  assert.equal(response.status, 200); sid = response.headers.get("set-cookie").split(";")[0];
  assert.equal((await call("POST", "booking", { room: 1, start: 0, end: 10 })).status, 403, "pre-login token is stale");
  await refresh();
  assert.equal((await call("POST", "booking", { room: 1, start: 0, end: 10 })).status, 201);
  const before = fs.readFileSync(path.join(data, "db.json"), "utf8");
  assert.equal((await call("POST", "booking", { room: 1, start: 9, end: 12 })).status, 400);
  assert.equal(fs.readFileSync(path.join(data, "db.json"), "utf8"), before, "rejected constraint must leave persistent rows unchanged");
  assert.equal((await call("POST", "booking", { room: 1, start: 10, end: 12 })).status, 201);
  assert.equal((await call("PUT", "booking/1", { room: 1, start: 0, end: 11 })).status, 400);
  assert.equal((await call("DELETE", "booking/1", undefined, "0".repeat(64))).status, 403);
  for (let n = 0; n < 5; n++) assert.equal((await call("POST", "session", { login: "unknown", password: "bad" })).status, 401);
  const limited = await call("POST", "session", { login: "unknown", password: "bad" });
  assert.equal(limited.status, 429); assert.equal(limited.headers.get("retry-after"), "60");
  assert.equal((await call("DELETE", "session")).status, 204);
  assert.equal((await call("GET", "booking")).status, 401);
} finally {
  if (child.exitCode === null) { const stopped = new Promise((resolve) => child.once("exit", resolve)); child.kill("SIGTERM"); await stopped; }
  fs.rmSync(data, { recursive: true, force: true });
}
console.log("v3 HTTP contracts passed");
