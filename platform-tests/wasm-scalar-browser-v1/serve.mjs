import { createReadStream, promises as fs } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";

const configuredRoot = process.env.SEMAPRAX_CALCULATOR_ROOT;
if (!configuredRoot) throw new Error("SEMAPRAX_CALCULATOR_ROOT must name the calculator directory");
const root = resolve(configuredRoot);
const rootPrefix = `${root}${sep}`;
const mime = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

function contentType(pathname) {
  const extension = pathname.slice(pathname.lastIndexOf("."));
  return mime.get(extension) ?? "application/octet-stream";
}

createServer(async (request, response) => {
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
  const path = resolve(root, `.${pathname === "/" ? "/index.html" : pathname}`);
  if (path !== root && !path.startsWith(rootPrefix)) {
    response.writeHead(404);
    response.end();
    return;
  }
  let metadata;
  try {
    metadata = await fs.stat(path);
  } catch {
    response.writeHead(404);
    response.end();
    return;
  }
  if (!metadata.isFile()) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.writeHead(200, {
    "Content-Length": metadata.size,
    "Content-Type": contentType(pathname),
    "Cache-Control": "no-store",
  });
  if (request.method === "HEAD") {
    response.end();
  } else {
    createReadStream(path).pipe(response);
  }
}).listen(4173, "127.0.0.1", () => {
  process.stdout.write("scalar-browser-server-ready\n");
});
