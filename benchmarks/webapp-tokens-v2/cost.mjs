// Estimated billed cost of one Claude Code agent transcript (JSONL).
//
//   node cost.mjs --tokenizer <dir> <name>=<transcript.jsonl> ...
//
// Input-side cost is exact from the provider usage each assistant turn
// records (uncached input, 5-minute cache writes, cache reads). The
// transcripts do not keep reliable output-token counts, and thinking is
// redacted, so output is a lower bound: the visible text and tool-call
// inputs the agent produced, counted with the offline Claude tokenizer.
// Prices are claude-sonnet-5-5 list prices in USD per million tokens, from
// the bundled Claude API reference (cached 2026-09-25).
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const PRICE = { input: 2.0, cacheWrite5m: 2.5, cacheRead: 0.2, output: 10.0 };
const flag = process.argv.indexOf("--tokenizer");
const count = createRequire(join(process.argv[flag + 1], "noop.js"))("@anthropic-ai/tokenizer").countTokens;

const rows = {};
for (const arg of process.argv.slice(2)) {
  if (!arg.includes("=")) continue;
  const [name, path] = arg.split("=");
  const usage = new Map();
  const seen = new Set();
  let visible = "";
  let model = null;
  for (const line of readFileSync(path, "utf8").split("\n")) {
    let o;
    try { o = JSON.parse(line); } catch { continue; }
    if (o.type !== "assistant") continue;
    const m = o.message;
    model ??= m.model;
    if (m.usage) usage.set(m.id, m.usage);
    for (const b of m.content || []) {
      const key = m.id + JSON.stringify(b);
      if (seen.has(key)) continue;
      seen.add(key);
      if (b.type === "text") visible += b.text + "\n";
      if (b.type === "tool_use") visible += JSON.stringify(b.input) + "\n";
    }
  }
  const sum = (k) => [...usage.values()].reduce((a, u) => a + (u[k] || 0), 0);
  const tokens = {
    input: sum("input_tokens"),
    cache_write: sum("cache_creation_input_tokens"),
    cache_read: sum("cache_read_input_tokens"),
    output_visible: count(visible),
  };
  const usd = {
    input: (tokens.input * PRICE.input + tokens.cache_write * PRICE.cacheWrite5m + tokens.cache_read * PRICE.cacheRead) / 1e6,
    output_lower_bound: (tokens.output_visible * PRICE.output) / 1e6,
  };
  usd.total_lower_bound = usd.input + usd.output_lower_bound;
  for (const k of Object.keys(usd)) usd[k] = +usd[k].toFixed(4);
  rows[name] = { model, turns: usage.size, tokens, usd };
}
console.log(JSON.stringify({ prices_usd_per_mtok: PRICE, runs: rows }, null, 2));
