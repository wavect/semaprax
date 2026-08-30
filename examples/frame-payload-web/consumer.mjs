import { runCorpus } from "./corpus-runner.mjs";
import { readFile } from "node:fs/promises";
import instantiate from "./generated/semaprax.bindings.js";

const corpus = JSON.parse(
  await readFile(new URL("./corpus.json", import.meta.url), "utf8"),
);
const wasm = new Uint8Array(
  await readFile(new URL("./generated/app.wasm", import.meta.url)),
);
const api = await instantiate(wasm);

runCorpus(api, corpus);
console.log("frame-payload-web-v1-ok");
