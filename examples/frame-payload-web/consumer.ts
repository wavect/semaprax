import {
  instantiate,
  type OptionalBytes,
  type SemapraxResult,
} from "./generated/semaprax.bindings.js";

// Fetching belongs to the consumer, not the generated authority-free runtime.
const response = await fetch(new URL("./generated/app.wasm", import.meta.url));
if (!response.ok) throw new Error("cannot load frame payload Wasm");
const wasm = new Uint8Array(await response.arrayBuffer());
const runtime = await instantiate(wasm);
const frame = new Uint8Array([83, 80, 88, 49, 0, 0, 0, 0]);
const direct: Uint8Array = runtime.functions["frame.payload"](frame);
const optional: OptionalBytes = runtime.functions["frame.payload-maybe"](frame);
const result: SemapraxResult<Uint8Array, bigint> =
  runtime.functions["frame.payload-result"](frame);
if (direct.length !== 0 || optional === null || optional.length !== 0 || !result.ok) {
  throw new Error("unexpected frame payload declaration behavior");
}
