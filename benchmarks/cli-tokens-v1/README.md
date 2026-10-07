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

Round 3's matched live campaign uses the pinned-model launcher and independent
acceptance collector in [LIVE-CAMPAIGN.md](LIVE-CAMPAIGN.md).

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
