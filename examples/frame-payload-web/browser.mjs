import instantiate from "./generated/semaprax.bindings.js";
import { runCorpus } from "./corpus-runner.mjs";

async function load(path) {
  const response = await fetch(new URL(path, import.meta.url));
  if (!response.ok) throw new Error(`cannot load ${path}`);
  return response;
}

try {
  const corpus = await (await load("./corpus.json")).json();
  const wasm = new Uint8Array(await (await load("./generated/app.wasm")).arrayBuffer());
  const runtime = await instantiate(wasm);
  const result = runCorpus(runtime, corpus);
  document.querySelector("#result").textContent = JSON.stringify(result);
  document.documentElement.dataset.status = "passed";
} catch (error) {
  document.querySelector("#result").textContent = String(error);
  document.documentElement.dataset.status = "failed";
}
