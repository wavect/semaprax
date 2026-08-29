import { readFile } from "node:fs/promises";
import instantiate from "./generated/semaprax.bindings.js";

const corpus = JSON.parse(
  await readFile(new URL("./corpus.json", import.meta.url), "utf8"),
);
const wasm = new Uint8Array(
  await readFile(new URL("./generated/app.wasm", import.meta.url)),
);
const api = await instantiate(wasm);

function fromHex(hex) {
  if (hex.length % 2 !== 0) throw new Error("odd corpus hex");
  return Uint8Array.from(
    { length: hex.length / 2 },
    (_, index) => Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16),
  );
}

function materialize(row) {
  if (row.kind === "hex") {
    return {
      frame: fromHex(row.frame_hex),
      expected: row.valid ? fromHex(row.payload_hex) : null,
    };
  }
  if (row.kind !== "generated-index-mod-256") {
    throw new Error(`unknown corpus kind ${row.kind}`);
  }
  const payload = Uint8Array.from(
    { length: row.payload_length },
    (_, index) => index & 0xff,
  );
  const frame = new Uint8Array(8 + payload.length);
  frame.set([83, 80, 88, 49], 0);
  new DataView(frame.buffer).setUint32(4, payload.length, false);
  frame.set(payload, 8);
  return { frame, expected: payload };
}

function equal(actual, expected, label) {
  if (!(actual instanceof Uint8Array) || actual.length !== expected.length) {
    throw new Error(`${label}: byte length mismatch`);
  }
  for (let index = 0; index < expected.length; index += 1) {
    if (actual[index] !== expected[index]) {
      throw new Error(`${label}: byte mismatch at ${index}`);
    }
  }
}

const direct = api.functions["frame.payload"];
const maybe = api.functions["frame.payload-maybe"];
const result = api.functions["frame.payload-result"];
let directCalls = 0;
for (const row of corpus.cases) {
  const { frame, expected } = materialize(row);
  const optional = maybe(frame);
  const resolved = result(frame);
  if (row.valid) {
    if (optional === null) throw new Error(`${row.name}: unexpected None`);
    equal(optional, expected, `${row.name}/maybe`);
    if (resolved.ok !== true) {
      throw new Error(`${row.name}: unexpected Err(${resolved.error})`);
    }
    equal(resolved.value, expected, `${row.name}/result`);
    directCalls += 1;
    equal(direct(frame), expected, `${row.name}/direct`);
  } else {
    if (optional !== null) throw new Error(`${row.name}: unexpected Some`);
    if (resolved.ok !== false || resolved.error !== BigInt(row.error)) {
      throw new Error(`${row.name}: wrong error branch`);
    }
  }
}
if (directCalls !== corpus.cases.filter((row) => row.valid).length) {
  throw new Error("direct payload was not confined to valid fixtures");
}
console.log("frame-payload-web-v1-ok");
