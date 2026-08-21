import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const directory = resolve(process.argv[2] ?? "target/meaning-web");
const expectation = process.argv[3] ?? "42";
const { imports, instantiateBytes } = await import(pathToFileURL(resolve(directory, "semaprax.js")));
const bytes = await readFile(resolve(directory, "app.wasm"));
const { instance } = await instantiateBytes(bytes);
let result;
let expectedTrap;
try {
  result = instance.exports.semaprax_main();
} catch (error) {
  if (!expectation.startsWith("error:")) throw error;
  expectedTrap = expectation.slice("error:".length);
  if (!(error instanceof Error) || !error.message.includes(expectedTrap)) {
    throw new Error(`expected trap containing ${JSON.stringify(expectedTrap)}, received ${error}`);
  }
}

if (expectation.startsWith("error:")) {
  if (result !== undefined) {
    throw new Error(`expected SEMAPRAX main to trap with ${expectation}, received ${result}`);
  }
} else if (result !== BigInt(expectation)) {
  throw new Error(`expected SEMAPRAX main to return ${expectation}, received ${result}`);
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

console.log(expectedTrap === undefined ? result.toString() : `error:${expectedTrap}`);
