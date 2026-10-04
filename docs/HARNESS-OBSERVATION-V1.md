# Harness observation v1 (HP-15)

Audience: harness-provider and benchmark maintainers. Owned by
`crates/semaprax-harness/src/observe/`; verb `report <observations.jsonl> [--json]`.
Diagnostics: `SPX-HPO001` invalid event/oversize field, `HPO002` trace read/parse,
`HPO003` tokenizer helper unavailable, `HPO004` refused mixed-kind count arithmetic.

## What already exists (#355, #356, #357) and how this reuses it

The harness crate cannot link the compiler crate, so reuse means adopting
schemas, field names and measurement-boundary rules and interoperating with
their artifacts. No generic counter or dashboard is added.

| Existing | Where | Reuse here |
| --- | --- | --- |
| #355 payload token reduction, named cached tokenizer | `scripts/token_measurement.py`, `scripts/token_report.py`, `scripts/benchmark_compact_projection.py`, `docs/COMPACT-SEMANTIC-PROJECTION-V2.md` | `scripts/harness_tokenize.py` is a thin line-protocol wrapper over `token_measurement.load_tokenizer` (local tiktoken cache only, sockets refused while loading, `cl100k_base`/`o200k_base`, vocabulary fingerprint, `disallowed_special=()`). It counts nothing itself. |
| #356 opt-in session accounting | `packages/semaprax-agent-workflow` `ToolPayloadObserver`, `semaprax.token-observation.v1`, `semaprax.token-session.v1` | Same rules: metadata only (bytes/digest/tokens), `tokens` absent without a supplied tokenizer, bounded event cap with a visible dropped count, `partial` when drops exist, paired vs unpaired, a sink that is observational only. Field vocabulary (`tokenizer`, `fingerprint`, `baseline`/paired, `dropped`, `coverage`) is kept. |
| #357 report view | `token_report.py show` | Its labels are kept: reduction, growth and unavailable tokens are shown separately; a report is never provider spending. |
| Attempt accounting | `src/provider_adapter_sdk/observation.rs` `AttemptObservation` (request/response commitments, usage `tokens_in/out/cost_micros`, `Option` unknown) | Cost provenance uses the same explicit-unknown model. Authoritative attempt accounting stays there; this module is optional observability and never feeds back. |

Gap this module fills: attribution across additional stages (context, router,
wrapper, gateway). Not done here (needs root `src`/scripts edits outside this
lane): emitting `semaprax.token-observation.v1` rows for the existing
`token_report.py session` consumer; the end-to-end paired payloads map to its
`baseline`/`actual` pairs one-to-one, so a later adapter is mechanical.

## Event `semaprax.harness-observation.v1`

Additive, one JSON object per JSONL line. Members: `provider`, `capability`,
`stage` (`context_select|compression|dedup|decision|generation|command_view|skill_catalog|retrieval_wrapper|index_build|model_load`),
`role` (`transform|incurred|local`), `invocation{id,parent}`, `payload_id`,
`source_revision`, `config_revision`, `upstream_model` (`"unknown"` explicit),
`cache` (`miss|hit|bypass`), `availability` (`available|fallback|unavailable`),
`outcome`, `warmth` (`cold|warm|n/a`), `latency_ms`,
`cost{provider_billed|"unknown", local_compute_ms, hidden_attempts|"unknown"}`,
`before`/`after` (transform sizes), `model_visible`, `incurred`, digests.
Counts are `{tokenizer: {kind:"named",name,fingerprint} | {kind:"byte_only"}, value}`.
There is no free-text member; identity strings are capped at 256 bytes and an
event violating that is dropped and counted. Raw recovery stays the HP-08 cache.

## Measurement

`Tokenizer { name, fingerprint, count }` plus `try_count`. `byte-v1` is UTF-8
bytes and is always labeled `byte_only`. Named tokenizers are supplied
explicitly (`ExternalTokenizer::spawn` over the helper protocol). `measure(text, tok)`
counts the exact text given: callers pass the final serialized model-visible
envelope (metadata, descriptions, wrappers, JSON escapes included). Without a
tokenizer the count is absent (a "missing tokenizer" event), never zero.
Byte and named counts are never summed; each tokenizer identity is its own
report group and mixed-kind lineages are unpaired (`HPO004` for direct arithmetic).

## Attribution

Per payload, `transform` events form a lineage. Stage-local reductions are
listed; the headline is first `before` vs last `after` (which must be marked
`model_visible`): 1000 -> 800 -> 700 is 300 fewer, not 500. `incurred` events
(router/indexer/model requests that happened, failed attempts included) are
summed separately; `net_savings = reduction - incurred` (200 in the example,
negative values are shown as such). Identical text sent twice counts twice; a
cache hit is counted as a cache hit, not as a saving. Local compute, cold
index/model load, warm queries and decision latency are reported apart from
provider billing; unknown billing or hidden gateway attempts stay `"unknown"`.

## Coverage

`coverage` lists events, drops, declared host traffic (observed/unobserved),
named-measured / byte-only / missing-tokenizer events, paired / unpaired
payloads and `reasons`. `whole_task_claim_allowed` is true only with no drops,
declared zero unobserved host traffic, no missing tokenizers, no unpaired
payloads, no unknown hidden attempts, one measurement kind and at least one
pair. Otherwise `partial: true` appears in JSON and text.

## Isolation and sinks

`Observer::record` returns `()`. Invalid events, cap overflow and sink write
errors only increase `dropped`; dispatch, status, retries and budgets are
caller state and are untouched. Sinks are caller-owned: `MemorySink` and
`JsonlFileSink` (created, never overwritten, byte-bounded). `Observer::finish`
appends a `semaprax.harness-observation-summary.v1` line with drops and host
traffic. No remote telemetry.

## Tests

`cargo test --offline -p semaprax-harness --test harness_v1 observe::`. The
real-tokenizer test is `#[ignore]`: it needs `HP15_PYTHON` (python3 with
tiktoken) and `HP15_TIKTOKEN_CACHE_DIR` (cached encodings). Without
tiktoken the helper test asserts the explicit refusal instead.

## Token-observation export

`report <observations.jsonl> --export token-observation [--output <new file>] [--session <id>]` maps each event to
one `semaprax.token-observation.v1` row (`observe::export`). `Transform` events measure `after` against the
`before` baseline of the same named tokenizer (`status: measured`, `referenceKind: source_context`); a named count
without a baseline is `baseline_unavailable`; byte-only sizes are `tokenizer_unavailable` with `bytes` set;
unmeasured events are `incomplete`. `Incurred` events measure `incurred`. Metadata only: digests, sizes, identities.
Consume with `python3 scripts/token_report.py session --events <rows.jsonl> --output <report.json>`.
