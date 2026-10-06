// Static token measurement for the TeamDesk benchmark. See README.md.
//
//   node measure.mjs [--tokenizer <dir containing node_modules/@anthropic-ai/tokenizer>]
//
// Without a tokenizer the script falls back to ceil(bytes / 4) and says so.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const flag = process.argv.indexOf("--tokenizer");
let count = (text) => Math.ceil(Buffer.byteLength(text) / 4);
let tokenizer = "bytes/4 estimate";
if (flag > 0) {
  const require = createRequire(join(process.argv[flag + 1], "noop.js"));
  count = require("@anthropic-ai/tokenizer").countTokens;
  tokenizer = "@anthropic-ai/tokenizer (offline Claude BPE)";
}
const text = (path) => readFileSync(join(here, path), "utf8");
const tokens = (paths) => paths.reduce((sum, path) => sum + count(text(path)), 0);
// Tokens an agent writes for a -U0 patch: the removed and added lines.
const editTokens = (path) =>
  count(text(path).split("\n").filter((l) => /^[-+](?![-+]{2} )/.test(l)).map((l) => l.slice(1)).join("\n"));
const reads = (arm) =>
  Object.fromEntries(text(`changes/${arm}-reads.txt`).trim().split("\n").map((l) => {
    const [n, ...files] = l.split(" ");
    return [n, files.map((f) => `${arm === "typescript" ? "typescript" : "semaprax"}/${f}`)];
  }));

const tsFiles = {
  client: ["src/views.tsx", "src/api.ts", "src/main.tsx", "index.html", "vite.config.ts", "package.json", "tsconfig.json", "shared/schema.ts"],
  server: ["server/index.ts"],
}.client.map((f) => `typescript/${f}`);
const tsClient = tsFiles;
const tsServer = ["typescript/server/index.ts", "typescript/.gitignore"];

const spec = count(text("SPEC.md"));
const slice = count(text("semaprax-today/customer_api.spx"));
// The slice's own author split it into ~9 KB shared and ~10.7 KB per-entity
// code; ten entities need the shared part once and the rest ten times.
const sharedShare = 9 / 19.7;
const todayServerEstimate = Math.round(slice * sharedShare + 10 * slice * (1 - sharedShare));

const arms = {
  typescript: {
    reference: 0,
    authored: tokens([...tsClient, ...tsServer]),
    files: tsClient.length + tsServer.length,
    green: count(text("changes/typescript-green-cycle.txt")),
  },
  "semaprax-today (lower bound)": {
    reference: count(text("reference/semaprax-today.md")),
    authored: tokens(tsClient) + slice,
    files: tsClient.length + 1,
    green: count(text("changes/typescript-green-cycle.txt")) + count(text("changes/semaprax-today-green-cycle.txt")),
  },
  "semaprax-today (estimate)": {
    reference: count(text("reference/semaprax-today.md")),
    authored: tokens(tsClient) + todayServerEstimate,
    files: tsClient.length + 1,
    green: count(text("changes/typescript-green-cycle.txt")) + count(text("changes/semaprax-today-green-cycle.txt")),
  },
  "semaprax webapp": {
    reference: count(text("reference/semaprax-webapp.md")),
    authored: tokens(["semaprax/teamdesk.spx"]),
    files: 1,
    green: count(text("changes/semaprax-green-cycle.txt")),
  },
};

// Two session models. Both start from the spec plus the arm's reference and
// end with a turn that reads the green verification output.
// - per_file: one file per turn, every turn re-sending what came before.
//   It charges multi-file stacks for every extra turn, an upper bound.
// - batched: every file is written in one turn, which is how the recorded
//   live agents worked. This is the realistic model and the headline.
// language_attributable leaves out the spec both arms read identically:
// reference + authored + verification output, each counted once.
for (const arm of Object.values(arms)) {
  const base = spec + arm.reference;
  const per = arm.authored / arm.files;
  let input = 0;
  for (let turn = 0; turn < arm.files; turn++) input += base + per * turn;
  input += base + arm.authored + arm.green;
  arm.per_file = { input: Math.round(input), output: arm.authored, total: Math.round(input) + arm.authored };
  const batchedInput = base + (base + arm.authored + arm.green);
  arm.batched = { input: batchedInput, output: arm.authored, total: batchedInput + arm.authored };
  arm.language_attributable = arm.reference + arm.authored + arm.green;
}

const changes = {};
for (const arm of ["typescript", "semaprax"]) {
  const r = reads(arm);
  changes[arm] = [1, 2, 3].map((n) => ({ read: tokens(r[n]), edit: editTokens(`changes/${arm}-${n}.patch`) }));
}

const ts = arms.typescript;
const ratio = (a, b) => +(a / b).toFixed(2);
const out = {
  tokenizer,
  spec,
  arms: Object.fromEntries(Object.entries(arms).map(([name, a]) => [name, {
    ...a,
    vs_typescript: {
      batched: ratio(a.batched.total, ts.batched.total),
      per_file: ratio(a.per_file.total, ts.per_file.total),
      language_attributable: ratio(a.language_attributable, ts.language_attributable),
    },
  }])),
  changes,
};
console.log(JSON.stringify(out, null, 2));
