# CLI Token Measurement Evidence v1

- Status: additive local measurement-import format; no matched-trace validation or billing claim
- Audience: benchmark maintainers and reviewers

This document defines optional evidence sidecars consumed by
`benchmarks/cli-tokens-v1/live_campaign.py recount`. The existing per-turn
provider counters, legacy net-input metric, API-equivalent `result.total_cost_usd`,
rate-card estimate, and `cost.mjs` visible-output proxy remain separate fields.
Missing values remain `null`; this format never substitutes a calibration
baseline or infers a context bucket from a residual.

## Trial binding

For trial `semaprax-01`, the recount may read:

- `provider-receipts/semaprax-01.json`, plus the provider-export bytes it names;
- `request-context-traces/semaprax-01.json`.

Both sidecars use the exact binding below. `trial_id` is `<arm>-<number as two
digits>`. `campaign_sha256` and `results_sha256` hash the retained `campaign.json`
and original `results.json`; `prompt_sha256` and `transcript_sha256` hash the
recorded prompt and that trial's JSONL transcript. `model_id` must match the
single observed model. Recount rejects a sidecar whose binding differs.

```json
{
  "campaign_sha256": "<64 lowercase hex>",
  "results_sha256": "<64 lowercase hex>",
  "trial_id": "semaprax-01",
  "arm": "semaprax",
  "number": 1,
  "model_id": "<observed dated provider model id>",
  "prompt_sha256": "<64 lowercase hex>",
  "transcript_sha256": "<64 lowercase hex>"
}
```

The receipt sidecar has schema `semaprax.cli-tokens.provider-receipt-binding.v1`
and exact top-level keys `schema`, `binding`, `provenance`, and `billed`.
`provenance` contains `provider: "anthropic"`,
`source_kind: "caller_supplied_provider_export"`, a nonempty `reference`, a
relative `document_path` inside the artifact directory, and the document's
`document_sha256`. `billed` contains `currency: "USD"` and an exact nonnegative
decimal string `amount_decimal`. Recount hashes the sidecar and named document,
keeps both digests, and reports the amount as
`provider_receipt_reported_billed_usd`.

These hashes establish the association and byte identity only. They do not
verify who issued the document. Imported amounts are labeled
`bound_caller_supplied_origin_unverified`; `provider_receipt_actual_usd` remains
`null`. The reported cost per accepted task is available only when at least five
attempts for that arm have complete receipt imports, and its numerator includes
all attempts, including failures. It is not an account-billed headline.

The receipt record's payload shape is:

```json
{
  "schema": "semaprax.cli-tokens.provider-receipt-binding.v1",
  "binding": {
    "campaign_sha256": "<64 lowercase hex>",
    "results_sha256": "<64 lowercase hex>",
    "trial_id": "semaprax-01",
    "arm": "semaprax",
    "number": 1,
    "model_id": "<observed dated provider model id>",
    "prompt_sha256": "<64 lowercase hex>",
    "transcript_sha256": "<64 lowercase hex>"
  },
  "provenance": {
    "provider": "anthropic",
    "source_kind": "caller_supplied_provider_export",
    "reference": "invoice row or export record identifier",
    "document_path": "provider-receipts/documents/semaprax-01.pdf",
    "document_sha256": "<64 lowercase hex>"
  },
  "billed": { "currency": "USD", "amount_decimal": "0.1234" }
}
```

## Request-composition trace

The request trace has schema
`semaprax.cli-tokens.request-context-trace.v1` and exact top-level keys
`schema`, `binding`, `provenance`, and `turns`. Its provenance identifies the
provider, uses `source_kind: "caller_supplied_request_trace"`, and includes a
nonempty source reference. Turns must match transcript assistant message IDs in
their observed order and contain:

- a nonempty provider `request_id` and the exact `message_id`;
- SHA-256 identities for the system prompt, tool schema, and original task
  prompt;
- `composition` with `system_tokens`, `tool_schema_tokens`,
  `task_prompt_tokens`, and `conversation_history_tokens`, each a
  nonnegative integer or `null`; and
- `thinking_output_tokens`, a nonnegative integer or `null` when the trace
  reports it.

Each complete composition must sum exactly to that transcript message's raw
`input_tokens + cache_creation_input_tokens + cache_read_input_tokens`.
Missing provider counters or missing composition buckets make the trace
incomplete; they remain `null`. A changed system or tool-schema identity marks
the trace `schema_drift` and suppresses complete totals while retaining every
raw usage counter and supplied per-turn bucket. The task-prompt digest must
match the retained prompt. Thinking counts are reported separately and are not
added to output tokens.

The per-turn trace shape is:

```json
{
  "message_id": "the matching transcript assistant message ID",
  "request_id": "provider request ID",
  "system_prompt_sha256": "<64 lowercase hex>",
  "tool_schema_sha256": "<64 lowercase hex>",
  "task_prompt_sha256": "the retained prompt_sha256",
  "composition": {
    "system_tokens": 120,
    "tool_schema_tokens": 80,
    "task_prompt_tokens": 40,
    "conversation_history_tokens": 200
  },
  "thinking_output_tokens": null
}
```

Per-arm reports retain trace status and complete known subtotals. No cached
token category is subtracted, and no baseline subtraction is promoted to
task-only input. No ratio headline should use these fields until matched real
request traces for both arms have been validated. Caller-supplied trace hashes
bind bytes to a trial but do not authenticate provider origin. The explicit
`fixed_harness_context_tokens` subtotal is exactly system plus tool-schema
tokens from the supplied buckets; it is unavailable when either bucket is
missing.

## Verification ownership

Synthetic regression coverage is in
`benchmarks/cli-tokens-v1/test_live_campaign.py`. The focused filter is
`python3 benchmarks/cli-tokens-v1/test_live_campaign.py`; authored fixtures cover complete
and incomplete multi-turn traces, cache reads/writes, schema drift, missing
provider buckets, task-prompt binding, receipt binding, document digest/path
rejection, minimum receipt coverage, and the unchanged `null` actual-cost field.
These fixtures do not substitute for matched real-trace validation or a trusted
provider-origin verifier. The `cost.mjs` raw-provider-output versus visible
proxy separation has fixtures in `benchmarks/webapp-tokens-v2/test_cost.mjs`,
with the focused command `node --test benchmarks/webapp-tokens-v2/test_cost.mjs`.
