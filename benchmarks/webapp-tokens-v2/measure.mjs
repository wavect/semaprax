// Static token measurement for TeamDesk Enterprise. See README.md.
//
//   node measure.mjs [--tokenizer <dir containing node_modules/@anthropic-ai/tokenizer>]
//
// The models match ../webapp-tokens-v1/measure.mjs: batched (headline),
// per_file (upper bound), and language_attributable (spec excluded).
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

const typescript = [
  "shared/schema.ts", "server/index.ts", "src/views.tsx", "src/main.tsx", "src/api.ts",
  "package.json", "tsconfig.json", "index.html", "vite.config.ts", ".gitignore",
].map((f) => `typescript/${f}`);
const spec = count(text("SPEC.md"));
const arms = {
  typescript: {
    reference: 0,
    authored: tokens(typescript),
    files: typescript.length,
    // The v2 arm deleted node_modules after verifying; its tsc/vite output has
    // the v1 shape, so the v1 green cycle stands in for it.
    green: count(text("changes/typescript-green-cycle.txt")),
  },
  "semaprax webapp": {
    reference: count(text("reference/semaprax-webapp.md")),
    authored: tokens(["semaprax/teamdesk.spx"]),
    files: 1,
    green: count(text("changes/semaprax-green-cycle.txt")),
  },
};
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
const ts = arms.typescript;
const ratio = (a, b) => +(b / a).toFixed(2);
const spx = arms["semaprax webapp"];
console.log(JSON.stringify({
  tokenizer,
  spec,
  arms,
  typescript_over_semaprax: {
    authored: ratio(spx.authored, ts.authored),
    language_attributable: ratio(spx.language_attributable, ts.language_attributable),
    batched: ratio(spx.batched.total, ts.batched.total),
    per_file: ratio(spx.per_file.total, ts.per_file.total),
  },
}, null, 2));
