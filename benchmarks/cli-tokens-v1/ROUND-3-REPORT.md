# LogLens round 3: descriptive report

**No qualified winner.** The original full acceptance outcome is 0/5 for both
SEMAPRAX and TypeScript. Every trial failed the original harness because of
four CR/CRLF probes that are absent from the frozen specification. A post-run
scope split shows that all five candidates per arm passed the 28 checks
classified as SPEC-aligned. That diagnostic does not replace or rewrite the
original result, and this report makes no scored winner claim.

This report describes the completed campaign at frozen compiler commit
`225f35ce1555e3f03ce200800612d2e3c2b6b43f` and frozen SPEC SHA-256
`f4bdeec94d85a554c1c0fa536091b4f50120fa76059d81b61abc523b0146a5d3`.
Both arms requested and observed `claude-sonnet-5-5` at medium effort. The
compiler executable build label was `02776ce87`; its SHA-256 is
`5ed55c5b14ba4972652e148b0a5d844995a131e24e434107d9b3135411ccd756`.
The independent recount completed all 10 planned attempts and retained a hash
for every transcript. The compact evidence file
[`round3-accounted-evidence.json`](round3-accounted-evidence.json) records
campaign, compiler, tokenizer, pricing, acceptance-scope, per-trial usage,
source-inventory, prompt, candidate-manifest, and transcript hashes without
copying transcript contents.

## Acceptance scope

The independent harness ran 33 checks per candidate. Twenty-eight were
classified as SPEC-aligned by the post-run scope assessor; all 28 passed for
all ten candidates. The additional reverse flag-order probe also passed in all
ten candidates. The four additional line-ending checks—CRLF text, CRLF JSON,
CR text, and CR JSON—failed in all ten candidates. The frozen SPEC defines no
CR or CRLF input behavior. The original combined outcome remains `not_accepted`
for all ten candidates, so accepted-task cost is **null** for each arm.

Builds and candidate-authored test scripts passed for all ten candidates. The
table below keeps the four original harness failures visible rather than
counting the retrospective SPEC-only classification as acceptance.

## Campaign measurements

| Measure | SEMAPRAX | TypeScript |
|---|---:|---:|
| Original full-corpus accepted | 0/5 | 0/5 |
| SPEC-aligned checks passing, post-run | 5/5 × 28 | 5/5 × 28 |
| Provider `num_turns`, total | 163 | 32 |
| Deduplicated assistant messages with usage, total | 161 | 30 |
| Raw `input_tokens` | 322 | 60 |
| Cache creation tokens | 338,843 | 89,135 |
| Cache read tokens | 8,143,308 | 475,906 |
| Raw input plus cache, total | 8,482,473 | 565,101 |
| Provider output tokens | 168,000 | 37,727 |
| Historical net-input convention, total | 7,324,722 | 349,371 |
| List-price estimate for five trials | $4.664678 | $0.829111 |
| Provider-reported API-equivalent cost for five trials | $4.664678 | $0.829111 |
| Model-session wall time, aggregate | 1,564.087 s | 283.028 s |
| Model-session wall time, median per trial | 235.649 s | 51.346 s |
| Final-source tokenizer proxy, median per trial | 6,439 | 3,202 |

The raw provider usage buckets are reported separately and are the authoritative
input/cache/output measurements. Cache creation was reported entirely at the
one-hour TTL for both arms. The `historical net-input convention` reproduces
the prior report formula: sum deduplicated per-turn input, cache-write, and
cache-read counters, then subtract the first-turn input-plus-cache total once
per turn. That first-turn baseline includes the task prompt and harness context;
this value is an operational historical measure, not task-only input.

The matched empty-task calibration observed first-turn input-plus-cache usage
of 6,723 tokens. Subtracting the legacy tokenizer proxy of 21 tokens for its
calibration prompt gives a **6,702-token one-turn context proxy**. It is reported
separately and was not subtracted from trial usage. The calibration cost was
$0.008791; including it, campaign list-price estimate totals $5.502580. These
are list-price estimates using the 2026-10-07 price book from the
[official Claude Sonnet 5.5 pricing page](https://platform.claude.com/docs/en/models/sonnet-5-5/overview);
provider result totals are API-equivalent metadata, not billing receipts. No
receipt amount was available.

The legacy Claude tokenizer proxy counted final candidate source inventories,
not generated output or cumulative authored edits. SEMAPRAX’s 31,725 proxy
tokens and TypeScript’s 15,969 proxy tokens should therefore be read separately
from provider output (168,000 and 37,727). The tokenizer was
`@anthropic-ai/tokenizer@0.0.4`, bundled Claude BPE `claude.json`, using
`countTokens` normalization; its dependency was `tiktoken@1.0.22`. It is not an
exact token count for the observed model or provider billing.

The source inventory covers final `.spx`/`.ts` implementation files plus
candidate-authored scripts, tests, manifests, and text documentation; it omits
known generated/dependency directories and binary files. The provider result
also reports thinking-token detail (60,864 SEMAPRAX; 6,924 TypeScript) alongside
the output bucket; it is retained as a provider detail, not treated as authored
source.

## Per-trial measurements

`Turns/messages` shows provider-reported `num_turns` followed by the number of
deduplicated assistant messages with usage. “Raw input+cache” is the sum of
`input_tokens`, cache creation, and cache read. Source tokens are final-source
proxy counts.

| Arm | Trial | Original result | Turns/messages | Raw input+cache | Output tokens | List estimate | Wall time | Source proxy |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| SEMAPRAX | 1 | not accepted | 27/27 | 1,084,815 | 24,331 | $0.658206 | 216.449 s | 6,572 |
| SEMAPRAX | 2 | not accepted | 66/66 | 4,189,152 | 60,816 | $1.832924 | 648.886 s | 6,527 |
| SEMAPRAX | 3 | not accepted | 25/24 | 1,056,011 | 26,565 | $0.700162 | 235.649 s | 6,252 |
| SEMAPRAX | 4 | not accepted | 25/25 | 1,211,366 | 28,657 | $0.771282 | 239.680 s | 5,935 |
| SEMAPRAX | 5 | not accepted | 20/19 | 941,129 | 27,631 | $0.702104 | 223.423 s | 6,439 |
| TypeScript | 1 | not accepted | 7/6 | 145,359 | 7,203 | $0.193631 | 53.996 s | 3,317 |
| TypeScript | 2 | not accepted | 8/7 | 182,563 | 10,122 | $0.242938 | 77.949 s | 3,854 |
| TypeScript | 3 | not accepted | 5/5 | 60,711 | 6,545 | $0.117001 | 48.973 s | 2,855 |
| TypeScript | 4 | not accepted | 6/6 | 85,484 | 6,998 | $0.135947 | 51.346 s | 3,202 |
| TypeScript | 5 | not accepted | 6/6 | 90,984 | 6,859 | $0.139594 | 50.764 s | 2,741 |

Provider `num_turns` and deduplicated assistant messages with usage differ on
four trials; both measures are retained rather than conflated. Provider output
also includes a separate thinking-token detail in the raw result stream; the
provider output bucket remains the reported output measure.

## Implementation and observed friction

The TypeScript arm produced a strong idiomatic baseline: typed option parsing,
Node built-ins, `Map`/`Set` aggregation, `BigInt` arithmetic for byte totals,
stable tie-breaking, and dependency-free build and tests. All five TypeScript
candidate test scripts passed, as did the five SEMAPRAX candidate test scripts.
This is a description of the archived candidates, not evidence that language
alone caused the measured differences.

All ten final agent responses mention the absence of `expected.txt` and
`expected.json` in the sparse checkout. The SPEC includes their required sample
outputs inline, but the seed contained only `SPEC.md` and `sample.log`. This
caused repeated file discovery and caveat turns. SEMAPRAX trial 1 also corrected
its own mistaken expected path count. These are avoidable prompt turns: the
round-two blind spot carried into all ten final responses, despite the outputs
being inline in the SPEC.

SEMAPRAX trial 2 was the 66-provider-turn outlier: 41 turns above the arm's
25-turn median and 39 above its next-highest trial. Its transcript shows the
main retry cluster was native implementation debugging. The agent first
reported a native crash, then repeatedly bisected the parser and report code
with reduced programs; it replaced a method-scanning loop with a
`string_find`-based check, changed the report construction and argument scan,
and reran interpreter/native comparisons before building the test script. The
transcript also records an edit attempt rejected by macOS `sed`, followed by a
Python-based edit. Those are concrete sources of additional turns, not a claim
that all 41 excess turns were avoidable. Across all SEMAPRAX transcripts, the
most actionable diagnostics were:

| Observed failure and retry | Follow-up target |
|---|---|
| `SPX-O101` “use of resource `paths` after ownership was moved” and `SPX-O107` “resource `uniq` may have been moved on another control-flow path”; agents changed ownership/branch structure and retried. | [#597](https://github.com/wavect/semaprax/issues/597): reduce string-parameter consumption friction and make the borrow fix explicit. |
| `SPX-U105` “explicit mutation v1 supports only scalar Copy values” when reassigning a string or collection; agents rewrote the state update around the restriction. | [#591](https://github.com/wavect/semaprax/issues/591): general `let mut` string reassignment. |
| `SPX-H006` “borrowed call lacks an exact place origin” while parsing/borrowing line data; the agent tried alternate helper and ownership shapes. | [#609](https://github.com/wavect/semaprax/issues/609): locate remaining `H006` diagnostics. |
| `SPX-T205` on `usage(message)` (“expects `str`, received `string`”); the final trial-1 helper takes `borrow str` and converts only when concatenating. Trial 4 also saw `is_request` expect `Slice<u8>`; its final helper accepts `borrow Slice<u8>` and the caller constructs a byte range. | Better exact signature discovery and call-site conversions; these are observed friction points, not all covered by one open issue. |
| `SPX-T252` in trial 4 rejected a string-valued `while` condition; the final implementation iterates on scalar byte offsets and lengths. | [#592](https://github.com/wavect/semaprax/issues/592): admit string conditions with a per-iteration cleanup region. |
| Trial 3's `str_starts_with(prefix)` call failed because `prefix` expected `str`, and trial 1's string-valued `usage` argument failed similarly. | Clarify standard-library signatures and conversions at the call site; keep distinct from project cross-module signature limits. |
| In the 66-turn run, small parser/report variants produced `SPX-T202` unknown `busy`/`size`, plus `SPX-P104`/`SPX-P201` while generating reduced source; later iterations restored a compiling candidate. | Keep generated-source and bisect experiments in the retry analysis; do not attribute their cost to the final implementation alone. |

Separately, all five SEMAPRAX candidates emitted `SPX-I307` at least once when
rebuilding to an existing single-file output path. This is an invocation
friction signal; the campaign can prevent it with fresh output paths and should
not count it as a language/API limitation. The CLI/run guidance work in
[closed issue #603](https://github.com/wavect/semaprax/issues/603) is a follow-up
context for a new campaign, not a cause established by this frozen round. The
SEMAPRAX source proxy is also about twice the TypeScript proxy
in aggregate (31,725 vs. 15,969 tokens); source size is descriptive and does
not establish whether API friction, implementation choice, or language caused
the difference.

For the next campaign, state explicitly that golden files are absent from the
sparse seed and that their outputs are inline in the SPEC, or include the
expected files as public seed inputs. Then compare a fresh campaign after
freezing any chosen compiler/API/docs targets; preserve the frozen report as the
baseline and do not attribute this round’s token difference to later changes.

This report freezes round 3’s original SPEC SHA. Any new CR/CRLF contract and
its acceptance cases belong to a later benchmark version; they must not be
retroactively applied to this campaign.
