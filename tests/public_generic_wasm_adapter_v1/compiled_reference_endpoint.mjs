// Issue #229 evidence: call the genuinely compiled `.wasm` module's real
// exported scalar function through the exact production JS glue this
// compiler's own `build_web_with_scalar_exports` emits
// (`semaprax.bindings.js` -> `semaprax.js`), never a hand-rolled ABI shim.
// All arithmetic (the byte-position swap) happens inside the compiled Wasm
// bytecode; this script only supplies one packed `i64` argument per call
// and reads back the returned `i64`.
//
// Invoked as: node compiled_reference_endpoint.mjs <export-id> <i64-arg>...
// Prints one JSON line: {"results":[{"ok":true,"value":"..."}, ...]}

import fs from "fs";
import { instantiateBytes } from "./semaprax.bindings.js";

const [exportId, ...args] = process.argv.slice(2);
if (!exportId || args.length === 0) {
  throw new Error("usage: compiled_reference_endpoint.mjs <export-id> <i64-arg>...");
}

const wasmBytes = new Uint8Array(fs.readFileSync("./app.wasm"));
const runtime = await instantiateBytes(wasmBytes);
const results = args.map((argument) => {
  const outcome = runtime.functions[exportId](BigInt(argument));
  return outcome.ok
    ? { ok: true, value: outcome.value.toString() }
    : { ok: false, status: outcome.status };
});
console.log(JSON.stringify({ results }));
