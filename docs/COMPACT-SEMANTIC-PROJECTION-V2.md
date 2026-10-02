# Compact semantic projection: model-text v2

Status: additive local implementation. [Version 1](COMPACT-SEMANTIC-PROJECTION-V1.md)
text and binary bytes remain unchanged. Model-text uses format version 2 and
an explicit `model-text` encoding selection; it does not replace full JSON.
Audience: agent and tool authors using model-text encoding and compiler contributors.

## Wire and deterministic selection

The UTF-8 envelope begins `SEMAPRAX-MODEL-TEXT 2\n`, followed by the same four
length-framed metadata fields as v1, in order: `profile`, `root`,
`source_revision`, `source_digest`. Each field is `name byte_length value\n`.
Next comes `dict N\n`, exactly N raw JSON string literals each followed by LF,
and `body\n` followed by the selected JSON bytes with dictionary references.
The digest uses v1’s existing domain-separated SHA-256 over the source length
and exact reconstructed selected bytes.

Documents smaller than 16,384 bytes use an empty dictionary.
Otherwise, only JSON string literals at least 16 bytes long, including quotes
and escape spelling, that occur at least twice enter the dictionary. Entries
are sorted by their exact UTF-8 bytes and numbered from zero. The body replaces
these literals with `@` followed by the canonical unsigned decimal index,
without a closing marker. Quotes delimit remaining inline strings, so `@` and
`~` inside them remain literal content. Short or infrequent strings remain
inline. Selection is a fixed deterministic heuristic; no tokenizer, network,
or model is consulted during encoding.

## Validation and replay

`encode_model_text` consumes an existing opaque `CompactProjection`.
`decode_model_text` independently parses, bounds expansion before appending,
checks the selected-byte digest, and normalizes through the existing encoder.
Re-encoding must reproduce v2 exactly, rejecting alternate dictionary order,
duplicates, unnecessary references, and noncanonical numbers/headers.
`decode_model_text_and_verify` also verifies the expected profile, root and
source revision with the existing binding diagnostic. Integrity is not origin
authentication: the CLI replay route additionally regenerates the selected
producer and requires exact selected-content equality.

Existing limits apply: 16 MiB wire and reconstructed source, 65,536 dictionary
entries, 1 MiB per entry and 4 KiB per metadata field. Decoder errors use the
existing compact diagnostic family; v1 compatibility and error meanings remain
unchanged. Decoded v2 values can be serialized as ordinary v1 projections.

## Interfaces and measurement

All six existing selected profiles admit `--encoding model-text` in the CLI
and `encoding: "model-text"` in `workspace/compact-projection`, including its
MCP forwarding route. The service reports `format_version: 2`. Negotiation
requires the exact version/encoding/profile intersection; v1 `text` or `binary`
cannot negotiate as v2. Existing binary-before-text preference is retained,
with model-text following those encodings when multiple offers are common.

`scripts/benchmark_compact_projection.py` measures full JSON, v1 text, and v2
model-text with cached cl100k/o200k tokenizers and replays all forms, including
binary. It counts the whole envelope. Small inputs can grow from fixed metadata;
this format claims neither universal savings nor token/billing authority.
Results record exact bytes, hashes, tokenizer versions, and vocabulary fingerprints.

`scripts/token_report.py` is the additive per-input measurement helper. Its
`projection` route invokes an explicitly named local compiler for `graph`,
`context`, or `task-context`, replays the selected wire against that producer,
and compares `same_selected_json` with the complete text or model-text
envelope. It reruns the producer to refuse a changing source or selected
revision. `compare` measures two caller-supplied UTF-8 files and labels that
relationship `reference_only_not_verified`; it establishes no task-quality or
semantic equivalence. The report's deterministic comparison identity binds
profile, hashed root/selection/options, selected source revision, exact byte
facts, tokenizer fingerprint and arithmetic. It records no source text, raw
wire payload, absolute path, model/billing count, money, or telemetry.

Only cached `cl100k_base` and `o200k_base` are admitted. Missing assets fail
unless `--allow-bytes-only` is selected; that report sets every token and
savings field to null. Tokenizer selection is separate from task-context's
existing selection-budget tokenizer. Reports are atomically created and refuse
replacement without `--overwrite`.

`token_report.py session --events events.jsonl --output report.json` consumes
metadata-only `semaprax.token-observation.v1` JSONL from the optional session
observer. It emits aggregate coverage and totals grouped by tokenizer and
fingerprint, boundary, and reference kind without retaining event/session IDs,
source revisions, subjects, or digests.

## Local measurements

The committed [measurement report](../benchmarks/compact-semantic-projection-v2/local-token-measurements.json)
uses cached tiktoken 0.12.0 with cl100k_base and o200k_base. Every wire replay
matches the selected full bytes; graph cases also match the ordinary graph
producer. Counts include the complete envelope.

| Selected view | Full bytes | Model-text bytes | cl100k full → model-text | o200k full → model-text |
|---|---:|---:|---:|---:|
| examples/banking_ledger.spx | 139,770 | 104,579 | 37,846 → 32,486 | 38,779 → 33,052 |
| Task context: ledger.apply | 6,069 | 6,322 | 1,679 → 1,802 | 1,721 → 1,844 |
| examples/http_app_routing.spx | 610,864 | 440,692 | 161,861 → 134,165 | 165,329 → 135,959 |
| Task context: app.main | 19,793 | 15,974 | 5,602 → 5,010 | 5,718 → 5,080 |
| examples/calculator-project/semaprax.toml | 9,893 | 10,138 | 2,500 → 2,615 | 2,521 → 2,639 |
| examples/frame-payload-project/semaprax.toml | 6,615 | 6,860 | 1,766 → 1,886 | 1,784 → 1,905 |

The two large graphs save 14–18% of measured tokens; the HTTP task context
saves 11%. Small Project graphs and the banking task context grow 5–7%
because the envelope dominates. Consumers should use full JSON for those
small views when token cost is the priority. Version 1’s measured token
regression remains documented; byte savings alone do not imply token savings.
