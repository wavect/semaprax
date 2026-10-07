# ShiftSim live campaign, round 1

The campaign stopped before its planned five tasks per arm could be completed.
Each arm has one accepted task, one genuine acceptance failure, and three
quota-interrupted attempts. The sample is incomplete, so these records support
no comparative performance or efficiency headline.

The frozen run used `claude-sonnet-5-5` at medium effort and the same prompt,
specification, independent corpus, and 15-case acceptance command for both
arms. SEMAPRAX used Project v24 / `language-command-io.stream.v2` with the
`argv-utf8+stdin-stream.v1` route. The compiler source commit was
`1e0e988218b51ad626881b87c83c553fc7cddf37`; compiler binary SHA-256 was
`1e5b4b0e5bd1c3e27cd09e72c47e8853ac39d035732c8a087c20ec4d663c183e`. Frozen SPEC
SHA-256 is `5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00`.
Qualification evidence passed all 15 cases before this campaign.

## Outcomes and usage

The two genuine acceptance failures both passed their candidate build and
authored test scripts, then failed the independent oracle. SEMAPRAX attempt 1
rejected `max-cardinality-escaped-keys-and-ids` with status 2 and
`shiftsim: invalid request`. TypeScript attempt 1 returned `peak_queue: 0`
instead of the oracle's `1` in four cases: `large-leading-whitespace`,
`single-exact-deadline`, `completion-and-arrival-same-time`, and
`idle-gap-and-late`. These are failures against the frozen corpus.

The other six attempts were interrupted by the provider's weekly-limit 429.
SEMAPRAX attempt 3 had already accumulated 62 deduplicated assistant messages
with usage (63 provider-reported session turns) before interruption. The other
five quota-interrupted attempts had zero reported input, cache, and output
tokens; their single error event is not productive model usage.

| SEMAPRAX attempt | Outcome | Wall seconds | Provider turns / usage messages | Raw input + cache tokens | Provider output tokens | Provider API-equivalent cost | Final archive proxy |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Acceptance failure | 1242.410 | 78 / 78 | 9,312,433 | 135,908 | $3.951136 | 13,800 |
| 2 | Accepted | 969.329 | 97 / 96 | 10,460,645 | 112,972 | $3.863980 | 8,954 |
| 3 | Quota interruption after usage | 750.820 | 63 / 62 | 5,431,367 | 90,467 | $2.520115 | 9,196 |
| 4 | Quota interruption before usage | 2.874 | 1 / 1 | 0 | 0 | $0.000000 | no archive |
| 5 | Quota interruption before usage | 1.788 | 1 / 1 | 0 | 0 | $0.000000 | no archive |

| TypeScript attempt | Outcome | Wall seconds | Provider turns / usage messages | Raw input + cache tokens | Provider output tokens | Provider API-equivalent cost | Final archive proxy |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Acceptance failure | 100.317 | 8 / 7 | 122,372 | 13,570 | $0.230595 | 9,314 |
| 2 | Accepted | 55.285 | 6 / 5 | 65,369 | 8,513 | $0.147325 | 4,020 |
| 3 | Quota interruption before usage | 1.798 | 1 / 1 | 0 | 0 | $0.000000 | no archive |
| 4 | Quota interruption before usage | 1.904 | 1 / 1 | 0 | 0 | $0.000000 | no archive |
| 5 | Quota interruption before usage | 2.753 | 1 / 1 | 0 | 0 | $0.000000 | no archive |

Raw totals across the five recorded attempts are 25,204,445 input-plus-cache
tokens and 339,347 output tokens for SEMAPRAX; 187,741 input-plus-cache tokens
and 22,083 output tokens for TypeScript. Cache buckets remain separate in the
JSON evidence. The final-source figures are a posthoc legacy Claude BPE proxy
over each archived final inventory, not cumulative authored tokens, provider
output, or current-model billing tokens. The recorded tokenizer is
`@anthropic-ai/tokenizer` 0.0.4 with its bundled `claude.json` encoding; its
identity and fingerprint, source inventory hashes, and every attempt's raw
usage are in [`results-live.json`](results-live.json).

## Cost and time interpretation

The provider stream reported API-equivalent cost subtotals of $10.3352316 for
SEMAPRAX and $0.3779200 for TypeScript. Dividing each incomplete subtotal by
the single accepted task gives $10.3352316 and $0.3779200 per accepted task,
including the recorded failures and quota interruptions. These are incomplete
campaign subtotals, not stable per-task estimates. No account receipt or actual
subscription charge was available. The matching dated list-price estimates are
also recorded separately in the JSON; the price book is dated 2026-10-07 and
linked to the [Claude Sonnet 5.5 pricing documentation](https://platform.claude.com/docs/en/models/sonnet-5-5/overview).

Summed trial wall time was 2,967.221 seconds for SEMAPRAX and 162.057 seconds
for TypeScript. The full campaign elapsed time was 3,181.971 seconds. These
totals include the quota-interrupted attempts and are not a matched-sample
comparison.

A separate one-turn empty-task calibration reported 6,723 input-plus-cache
tokens, 4 output tokens, and $0.0059596 API-equivalent cost over 4.078 seconds.
It is a diagnostic session, not isolated fixed harness context, and was not
subtracted from any trial. The trial JSON also retains the historical “net
input” convention for audit; raw per-turn input/cache buckets are authoritative
and the historical field is not task-only input.

## Evidence and scope

[`results-live.json`](results-live.json) is the compact normalized record and
contains hashes for the original campaign, results, transcripts, and posthoc
source-proxy evidence. Full transcripts and candidate archives remain outside
the repository at `/Users/kevin/.codex/benchmark-runs/shiftsim-round1-20261007-r1`.
The campaign was frozen against one compiler source/binary identity; later
compiler changes do not alter these results.

The campaign plan captured issue 611 as open when it started. The current
[issue 611](https://github.com/wavect/semaprax/issues/611) is closed; its
qualification evidence establishes the stream route used here, while this
round's acceptance outcomes remain exactly as recorded above. No additional
issue was filed from these results.
