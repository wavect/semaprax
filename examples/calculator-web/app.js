import { instantiate } from "./generated/semaprax.bindings.js";

const runtime = await instantiate(new URL("./generated/app.wasm", import.meta.url));
const form = document.querySelector("#calculator");
const result = document.querySelector("#result");

form.addEventListener("submit", (event) => {
  event.preventDefault();
  try {
    const left = BigInt(document.querySelector("#left").value);
    const right = BigInt(document.querySelector("#right").value);
    const operation = document.querySelector("#operation").value;
    const outcome = runtime.call(operation, left, right);
    result.value = outcome.ok
      ? outcome.value.toString()
      : `semantic failure: ${outcome.status.domain_id}/${outcome.status.code}`;
  } catch (error) {
    result.value = error instanceof Error ? error.message : String(error);
  }
});
