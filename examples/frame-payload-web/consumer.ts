import {
  instantiate,
  type OptionalBytes,
  type SemapraxResult,
} from "./generated/semaprax.bindings.js";

const runtime = await instantiate(new URL("./generated/app.wasm", import.meta.url));
const frame = new Uint8Array([83, 80, 88, 49, 0, 0, 0, 0]);
const direct: Uint8Array = runtime.functions["frame.payload"](frame);
const optional: OptionalBytes = runtime.functions["frame.payload-maybe"](frame);
const result: SemapraxResult<Uint8Array, bigint> =
  runtime.functions["frame.payload-result"](frame);
if (direct.length !== 0 || optional === null || optional.length !== 0 || !result.ok) {
  throw new Error("unexpected frame payload declaration behavior");
}
