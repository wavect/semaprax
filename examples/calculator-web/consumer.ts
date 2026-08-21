import { instantiate, type ScalarResult } from "./generated/semaprax.bindings.js";

const runtime = await instantiate(
  new URL("./generated/app.wasm", import.meta.url),
);
const outcome: ScalarResult<bigint> = runtime.functions["calculator.add"](
  19n,
  23n,
);
if (outcome.ok && outcome.value !== 42n) {
  throw new Error("unexpected calculator result");
}
