import { instantiateBytes as instantiateRuntimeBytes, semanticStatus, wasmSha256 as runtimeWasmSha256 } from "./semaprax.js";
const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
const EXPECTED_WASM_SHA256 = "4707bbe39a5773fc894ca02e7198cdb1ce6b97f3fec15a4e8bbc219b5657f0bb";
if (runtimeWasmSha256 !== EXPECTED_WASM_SHA256) throw new Error("SEMAPRAX scalar binding/runtime digest disagreement");
const ENTRIES = Object.freeze([["callback.advance",Object.freeze({raw:"spx_scalar_63616c6c6261636b2e616476616e6365",params:Object.freeze(["i64","i64"]),result:"i64"})]]);
const EXPORT_IDS = Object.freeze(ENTRIES.map(([id]) => id));
const FACTS = Object.create(null);
for (const [id, fact] of ENTRIES) Object.defineProperty(FACTS, id, { value: fact, enumerable: true });
Object.freeze(FACTS);
function argument(value, type, index) {
  if (type === "i64") {
    if (typeof value !== "bigint" || value < SPX_MIN || value > SPX_MAX) throw new TypeError(`argument ${index} must be a signed 64-bit bigint`);
    return value;
  }
  if (typeof value !== "boolean") throw new TypeError(`argument ${index} must be boolean`);
  return value ? 1 : 0;
}
function result(value, type) {
  if (type === "i64") {
    if (typeof value !== "bigint" || value < SPX_MIN || value > SPX_MAX) throw new TypeError("SEMAPRAX adapter returned invalid i64");
    return value;
  }
  if (value !== 0 && value !== 1) throw new TypeError("SEMAPRAX adapter returned non-canonical bool");
  return value === 1;
}
function invoke(instance, id, values) {
  const fact = FACTS[id];
  if (fact === undefined) throw new RangeError(`unknown SEMAPRAX scalar export: ${id}`);
  if (values.length !== fact.params.length) throw new TypeError(`SEMAPRAX scalar export ${id} expects ${fact.params.length} arguments`);
  const raw = instance.exports[fact.raw];
  if (typeof raw !== "function") throw new Error(`SEMAPRAX scalar adapter missing: ${fact.raw}`);
  try { return Object.freeze({ ok: true, value: result(raw(...values.map((value, index) => argument(value, fact.params[index], index))), fact.result) }); }
  catch (error) {
    const status = semanticStatus(error);
    if (status !== null) return Object.freeze({ ok: false, status });
    throw error;
  }
}
function facade(instance) {
  const functions = Object.create(null);
  for (const id of EXPORT_IDS) Object.defineProperty(functions, id, { value: (...values) => invoke(instance, id, values), enumerable: true });
  return Object.freeze({ functions: Object.freeze(functions), call: (id, ...values) => invoke(instance, id, values) });
}
export async function instantiateBytes(bytes) { const linked = await instantiateRuntimeBytes(bytes); return facade(linked.instance); }
export async function instantiate(url = new URL("./app.wasm", import.meta.url)) { const response = await fetch(url); return instantiateBytes(await response.arrayBuffer()); }
export const exportIds = EXPORT_IDS;
