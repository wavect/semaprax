import { createReadStream, lstatSync, realpathSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.SEMAPRAX_HTTPS_PACKAGE_ROOT;
if (!configuredRoot) throw new Error("SEMAPRAX_HTTPS_PACKAGE_ROOT is required");
const generatedRoot = realpathSync(resolve(configuredRoot));
if (!statSync(generatedRoot).isDirectory()) throw new Error("generated package is not a directory");
const harnessRoot = realpathSync(fileURLToPath(new URL(".", import.meta.url)));
const portText = process.env.SEMAPRAX_HTTPS_BROWSER_PORT ?? "4189";
if (!/^[0-9]+$/.test(portText)) throw new Error("browser fixture port must be decimal");
const port = Number(portText);
if (!Number.isSafeInteger(port) || port < 1024 || port > 65535) {
  throw new Error("browser fixture port is outside the admitted range");
}

const types = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

function regularWithin(root, relative) {
  const candidate = resolve(root, relative);
  const prefix = `${root}${sep}`;
  if (candidate !== root && !candidate.startsWith(prefix)) return undefined;
  try {
    return lstatSync(candidate).isFile() ? candidate : undefined;
  } catch {
    return undefined;
  }
}

createServer((request, response) => {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { Allow: "GET, HEAD" });
    response.end();
    return;
  }
  let pathname;
  try {
    pathname = decodeURIComponent(new URL(request.url, "http://127.0.0.1").pathname);
  } catch {
    response.writeHead(400);
    response.end();
    return;
  }
  const generated = pathname.startsWith("/generated/");
  const relative = generated ? pathname.slice("/generated/".length) : pathname.slice(1) || "index.html";
  const path = regularWithin(generated ? generatedRoot : harnessRoot, relative);
  if (!path) {
    response.writeHead(404);
    response.end();
    return;
  }
  const size = statSync(path).size;
  response.writeHead(200, {
    "Cache-Control": "no-store",
    "Content-Length": size,
    "Content-Type": types.get(extname(path)) ?? "application/octet-stream",
  });
  if (request.method === "HEAD") response.end();
  else createReadStream(path).pipe(response);
}).listen(port, "127.0.0.1", () => {
  process.stdout.write(`https-browser-server-ready:${port}\n`);
});
