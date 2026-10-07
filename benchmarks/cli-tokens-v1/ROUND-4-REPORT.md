# LogLens round 4: matched live results

**TypeScript required fewer turns and lower estimated cost in this campaign.**
SEMAPRAX passed 4/5 tasks; TypeScript passed 5/5. These results do not support a
SEMAPRAX token-efficiency advantage or an assertion that meaningful gains are
exhausted. All attempts, including the failed SEMAPRAX candidate, remain in the
cost-per-accepted-task numerator.

Both arms used and observed `claude-sonnet-5-5`, medium effort, five independent
runs each, with matched application requirements and balanced order
S1, T1, T2, S2, S3, T3, T4, S4, S5, T5. The SEMAPRAX arm used the native compiler;
the baseline was a Node/TypeScript application with the same independent
acceptance checks. Each trial began in a fresh minimal Git seed repository
containing only SPEC and sample input; its prompt allowed candidate files only.
No prior candidate implementation or original repository history was supplied.
Prompt bytes vary with the isolated candidate path and requested language;
per-trial prompt hashes reproduce from the same disclosed harness template.
The timeout was 1,800 seconds per trial; no trial timed out.

The source/harness/compiler commit was
`2f2ec1acf8756610bf9fc194a43f0df27aadcbd6`; the frozen executable SHA-256 was
`a8527bee158f6b6d1822e106b7022a354900a81a0bd57f4c888cfb9b823e2b77`.
The frozen SPEC SHA-256 was
`51bf564cb9f7eadd4b3bd12fac53b7857befa09b3ef757d9c34400a8509fdeae`.
Unlike round 3, this round explicitly required LF, CRLF and CR line endings
before any agent ran. Round 3 remains unrescored and independently preserved.

## Measurements

| Measure, all five attempts per arm | SEMAPRAX | TypeScript |
|---|---:|---:|
| Full-corpus acceptance | 4/5 | 5/5 |
| Provider session turns, total | 136 | 34 |
| Deduplicated assistant messages carrying usage | 131 | 30 |
| Raw input tokens | 262 | 60 |
| Cache creation tokens, all one-hour TTL | 366,750 | 121,606 |
| Cache read tokens | 7,481,491 | 595,732 |
| Raw input plus cache, total | 7,848,503 | 717,398 |
| Historical net-input convention, total | 6,894,561 | 498,938 |
| Provider output tokens | 156,767 | 43,557 |
| Final-source legacy tokenizer proxy, total | 37,300 | 17,522 |
| Final-source legacy tokenizer proxy, median | 7,660 | 3,666 |
| Estimated list-price cost, all attempts | $4.531491 | $1.041260 |
| Estimated cost per accepted task | $1.132873 | $0.208252 |
| Model-session wall time, total | 1,381.843 s | 322.019 s |
| Model-session wall time, median | 274.213 s | 63.077 s |

Raw provider input, cache creation, cache reads and output are separate usage
buckets. The historical net-input formula sums deduplicated per-turn input and
cache counters, then subtracts first-turn input-plus-cache multiplied by the
number of those messages. That baseline includes the task prompt and harness
context. This is the legacy operational convention, **not task-only input and
not an upper bound**. Cached input still costs money and remains in the priced
usage. Provider session turns differ from usage-bearing message IDs; both are
shown without treating tool calls as turns.

The matched empty-task calibration observed 6,723 first-turn input-plus-cache
tokens. Subtracting only its 21-token legacy prompt proxy yields a separate
**6,702-token one-turn fixed-context proxy**. It does not isolate each trial's
repeated fixed context and was not subtracted from trial totals. Calibration
cost was $0.008791, reported separately; including it, the whole campaign cost
estimate was $5.581542. Overall campaign wall time was 1,781.304 seconds,
including calibration, independent acceptance and bookkeeping; the arm table
reports model-session time.

Prices use the 2026-10-07 [official Sonnet 5.5 price book](https://platform.claude.com/docs/en/models/sonnet-5-5/overview):
$2/M input, $4/M one-hour cache creation, $0.20/M cache read and $10/M output.
Provider API-equivalent reported totals were $4.531492 and $1.041260; small
rounding differences are retained. These are estimates/metadata, **not billing
receipts**; actual account-billed amounts are unavailable.

“Authored tokens” can only be approximated here by final candidate source:
`@anthropic-ai/tokenizer@0.0.4`, bundled legacy Claude BPE, NFKC normalization,
`tiktoken@1.0.22`. This proxy includes final implementation, authored tests,
scripts, manifests and text documentation; it excludes dependencies, generated
artifacts, binary files and deleted/rewritten drafts. It is not cumulative
agent-authored generation or exact current-model/billing tokenization.
Provider output includes thinking; the separately reported thinking details
were 57,886 tokens for SEMAPRAX and 7,631 for TypeScript and are not added again.

## Acceptance and next targets

All ten candidates built and passed their own test scripts. The independent
corpus checked 33 cases per candidate: 32 are classified as explicit SPEC
checks and one is an extra reversed-option-order robustness probe. Full corpus
acceptance is authoritative; the post-run classification changes no scores.
Every TypeScript candidate passed every check. SEMAPRAX trial 4 failed
`blank-lines-text` and `blank-lines-json` with checked `semaprax.text.v1/1`;
all other checks and all four other SEMAPRAX candidates passed.

The failed candidate computed `string_byte_at(text, q + 1)` before guarding
that line as valid, so blank lines could produce an out-of-range read. That is
an application bug caught by the independent acceptance corpus. It is not
rescored as a compiler failure, and the corpus must remain intact.

Transcripts also show repeated compiler friction: agents tried to pass
`borrow str` to `string_byte_at`, invented `i64_from_u8`, retried branch string
reassignment and map renewal after U105/T252, and manually located an H006
shared-loan overlap without a source span. String operations beyond a named
`string_len` read remain refused in while conditions. Follow-up compiler,
language and help work is tracked by OPT #591, #592, #594, #597, #609 and #615.
These targets are hypotheses for the next matched round, not measured gains.
The separate ShiftSim stdin-stream work (#611) remains a third-application
qualification step; it is not included in this CLI result.

## Evidence and reproduction

[`round4-accounted-evidence.json`](round4-accounted-evidence.json) contains the
campaign identity, price book, tokenizer fingerprint, per-trial acceptance,
usage, final source inventories, prompts, candidate and transcript hashes.
`results-live.json` appends this round while retaining all earlier runs.
Raw transcripts and rebuildable candidates remain locally in
`/Users/kevin/.codex/benchmark-runs/loglens-round4-20261007-r1`; transcript
contents are not copied into Git. Evidence is local saved-campaign evidence,
not a hosted or current-head benchmark run.

Recount the saved artifacts without rerunning agents or acceptance:

```sh
python3 benchmarks/cli-tokens-v1/live_campaign.py recount --round 4 \
  --artifacts /Users/kevin/.codex/benchmark-runs/loglens-round4-20261007-r1
```

The parser deduplicates assistant message IDs, retains discrepancies between
per-message snapshots and authoritative provider result totals, authenticates
round/SPEC identity and hashes each saved transcript. Recounting does not
mutate the original results or repair failed candidates. A fresh campaign must
choose and disclose its own compiler, prompts and frozen source identity.
