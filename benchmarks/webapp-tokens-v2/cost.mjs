// Estimated billed cost of one Claude Code agent transcript (JSONL).
//
//   node cost.mjs --tokenizer <dir> <name>=<transcript.jsonl> ...
//
// Provider usage counters and the visible-output tokenizer proxy are kept
// separate. The visible-output count is a lower bound; a provider-reported
// output count may include hidden thinking but is not a billing receipt.
// Neither this script nor result.total_cost_usd establishes account-billed
// cost. Prices are claude-sonnet-5-5 list prices in USD per million tokens,
// from the bundled Claude API reference (cached 2026-09-25).
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
    if (m.id && m.usage && typeof m.usage === "object") {
      const previous = usage.get(m.id) || {};
      const merged = { ...previous };
      for (const key of [
        "input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens",
        "output_tokens", "thinking_tokens",
      ]) {
        const value = m.usage[key];
        if (Number.isSafeInteger(value) && value >= 0) merged[key] = value;
        else if (!(key in merged)) merged[key] = null;
      }
      usage.set(m.id, merged);
    }
    for (const b of m.content || []) {
      const key = m.id + JSON.stringify(b);
      if (seen.has(key)) continue;
      seen.add(key);
      if (b.type === "text") visible += b.text + "\n";
      if (b.type === "tool_use") visible += JSON.stringify(b.input) + "\n";
    }
  }
  const sum = (k) => {
    const values = [...usage.values()].map((u) => u[k]);
    if (values.length === 0 || values.some((value) =>
      !Number.isSafeInteger(value) || value < 0)) return null;
    return values.reduce((a, value) => a + value, 0);
  };
  const priced = (tokens, rate) => tokens === null ? null : (tokens * rate) / 1e6;
  const tokens = {
    input: sum("input_tokens"),
    cache_write: sum("cache_creation_input_tokens"),
    cache_read: sum("cache_read_input_tokens"),
    output_provider_reported: sum("output_tokens"),
    thinking_provider_reported: sum("thinking_tokens"),
    output_visible: count(visible),
  };
  const usd = {
    input: tokens.input === null || tokens.cache_write === null || tokens.cache_read === null
      ? null
      : (tokens.input * PRICE.input + tokens.cache_write * PRICE.cacheWrite5m + tokens.cache_read * PRICE.cacheRead) / 1e6,
    output_lower_bound: priced(tokens.output_visible, PRICE.output),
    output_provider_usage_rate_card_estimate: priced(tokens.output_provider_reported, PRICE.output),
  };
  usd.total_lower_bound = usd.input === null ? null : usd.input + usd.output_lower_bound;
  for (const k of Object.keys(usd)) {
    if (usd[k] !== null) usd[k] = +usd[k].toFixed(4);
  }
  rows[name] = { model, turns: usage.size, tokens, usd };
}
console.log(JSON.stringify({ prices_usd_per_mtok: PRICE, runs: rows }, null, 2));
