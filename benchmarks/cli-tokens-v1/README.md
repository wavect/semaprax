# Command-line token benchmark v1: LogLens

A completely different application from the web benchmarks: a command-line
log analyzer ([SPEC.md](SPEC.md)). It parses about 250 Common Log Format
lines, counts by key, ranks, and prints a fixed text report or JSON. Golden
outputs come from a seeded oracle (`oracle.py`, which benchmark agents may not
read): `loglens sample.log` must print [expected.txt](expected.txt), and
`loglens sample.log --top 3 --json` must print [expected.json](expected.json).

Historical “net” values below use the earlier operational convention: sum each
turn's provider input, cache-write, and cache-read counts, then subtract the
first turn's input-plus-cache count once per turn. The first-turn baseline
includes the task prompt as well as harness context. These values are not
task-only model input or an upper bound; the live campaign labels this metric
`legacy_net_input_tokens` and reports raw provider usage separately.

## Round 3: matched live campaign, no qualified winner

The original full-corpus acceptance result was **0/5 for both SEMAPRAX and
TypeScript**, so cost per accepted task is undefined for both. The retrospective
scope split found 28 SPEC-aligned checks passing all five candidates per arm;
four extra CR/CRLF probes failed every candidate, although the frozen SPEC did
not define those line endings. The split does not replace the original result.

The same requested and observed model identity and effort were used for both arms. SEMAPRAX used more
measured input/cache usage (8,482,473 raw tokens vs. 565,101), provider output
(168,000 vs. 37,727), and list-price estimate ($4.664678 vs. $0.829111). These
are descriptive differences from this campaign, not a qualified win or a
causal language comparison. The final-source-only authored-token proxy was
31,725 vs. 15,969 and is reported separately from provider output and the
historical `legacy_net_input_tokens` convention.

See the [round-three report](ROUND-3-REPORT.md) for per-trial usage, wall time,
acceptance scope, transcript-backed retry evidence, and accounting caveats.
The additive round-three entry in [results-live.json](results-live.json) keeps
the round-one and round-two records intact; its compact hash and campaign
metadata are in [round3-accounted-evidence.json](round3-accounted-evidence.json).
Any changed guard-condition or other implementation should be measured as a
new rerun against this frozen baseline; [issue #612](https://github.com/wavect/semaprax/issues/612)
tracks that fair comparison.

## Round 1 (baseline, `f106fcebe`)

| | TypeScript (Node, no deps) | SEMAPRAX |
| --- | ---: | ---: |
| Completed | yes, all tests pass | **no** |
| Turns | 6 | 76 |
| Legacy net input tokens | 104,540 | 4,363,904 |
| Estimated cost (lower bound) | $0.20 | $1.94 |
| Authored tokens | 1,540 | (a 749-byte stub) |

SEMAPRAX could not express the program. The web benchmarks never needed text
processing in the language itself, because the projection generates it.
Here it is the whole task, and the core language cannot yet:

- accumulate text in a loop: `let mut` holds only Copy scalars (`SPX-U105`),
  and string literals or string-producing calls are not admitted in `while`
  bodies (`SPX-T252`);
- recurse deeply enough to walk 250 lines instead (about 256 frames);
- read a named file from the command profile, or write computed text more
  than once per execution path (`SPX-T269`);
- slice or search strings by offset, parse integers, or keep string
  collections or maps.

Each of these is now an improvement target. The next rounds re-run the same
spec as they land; see [results-live.json](results-live.json).

Round 3 used the pinned-model launcher and independent acceptance collector in
[LIVE-CAMPAIGN.md](LIVE-CAMPAIGN.md); its results are reported above and in the
linked round-three report.

## Round 2 (Owned String Loops, Text Toolkit, String Collections, factored replay)

| | TypeScript | SEMAPRAX run 1 | SEMAPRAX run 2 |
| --- | ---: | ---: | ---: |
| Completed | yes | **yes** | **yes** |
| Turns | 6 | 19 | 27 |
| Legacy net input tokens | 104,540 | 839,703 | 1,360,111 |
| Estimated cost | $0.20 | $0.65 | $0.88 |
| Authored tokens | 1,540 | 4,252 | 4,307 |

SEMAPRAX can now write the whole program, and both goldens and every exit
status pass. It still costs 3–4 times as much as TypeScript. The agents'
failed checks name the next targets: user functions with string parameters,
`match`, and `arg_utf8` inside loop bodies (`SPX-T252`, `SPX-T270`); `if`
as a statement and casts (`SPX-P106`); string reassignment from a branch
(`SPX-U105`); string parameters that consume their argument; and no `str`
to `string` conversion.

Round 4 completed with explicit LF/CRLF/CR requirements, five matched live runs
per arm: SEMAPRAX accepted 4/5 and TypeScript 5/5. See
[the round 4 report](ROUND-4-REPORT.md) and
[accounted evidence](round4-accounted-evidence.json) for usage, fixed-context
calibration, estimated cost per accepted task and saved-artifact identities.

## Next round preflight (2026-10-08)

The Sonnet round-5 plan preserves the exact round-4 specification and sample
hashes, with five attempts per arm. Its calibration was rejected by the
account weekly quota; **zero application trials launched**. The
[preflight record](round5-sonnet-preflight.json) binds the retained artifacts.
This is not a scored campaign or a language-comparison result.

A separate Codex GPT-6.1 Sol medium-effort entitlement preflight returned
`READY`. Its [record](codex-preflight-20261008.json) preserves raw input/cache/
cache-write/output counters and trace hashes. This read-only preflight is
separate from any matched application campaign and calibration. Its conditional
Standard short-context API rate-card estimate is not an actual subscription
billing receipt, and unavailable fixed-context composition remains null.
The [official model pricing](https://developers.openai.com/api/docs/models/gpt-6.1-sol)
was checked on 2026-10-08. A new provider campaign must retain its own prompt,
model, CLI, usage, acceptance and source provenance; results cannot be merged
into the historical Sonnet campaign.
