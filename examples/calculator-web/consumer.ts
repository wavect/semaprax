import { instantiate, type ScalarResult } from "./generated/semaprax.bindings.js";

const runtime = await instantiate(
  new URL("./generated/app.wasm", import.meta.url),
);
const arithmetic: ScalarResult<bigint>[] = [
  runtime.functions["calculator.add"](19n, 23n),
  runtime.functions["calculator.subtract"](84n, 42n),
  runtime.functions["calculator.multiply"](6n, 7n),
  runtime.functions["calculator.divide"](84n, 2n),
];
for (const outcome of arithmetic) {
  if (!outcome.ok) {
    throw new Error(
      `unexpected calculator status ${outcome.status.domain_id}/${outcome.status.code}`,
    );
  }
  if (outcome.value !== 42n) {
    throw new Error("unexpected calculator result");
  }
}

const negative: ScalarResult<boolean> =
  runtime.functions["calculator.is-negative"](-1n);
const negated: ScalarResult<boolean> = runtime.functions["calculator.not"](true);
if (!negative.ok) {
  throw new Error(`unexpected predicate status ${negative.status.code}`);
}
if (!negated.ok) {
  throw new Error(`unexpected predicate status ${negated.status.code}`);
}
if (!negative.value || negated.value) {
  throw new Error("unexpected calculator predicate result");
}
