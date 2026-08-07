import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const directory = resolve(process.argv[2] ?? "target/meaning-web");
const { imports } = await import(pathToFileURL(resolve(directory, "semaprax.js")));
const bytes = await readFile(resolve(directory, "app.wasm"));
const { instance } = await WebAssembly.instantiate(bytes, imports);
const result = instance.exports.semaprax_main();

if (result !== 42n) {
  throw new Error(`expected SEMAPRAX main to return 42, received ${result}`);
}

let overflowTrapped = false;
try {
  imports.env.spx_add((1n << 63n) - 1n, 1n);
} catch (error) {
  overflowTrapped = error instanceof RangeError;
}
if (!overflowTrapped) {
  throw new Error("WebAssembly host arithmetic did not reject i64 overflow");
}

console.log(result.toString());
