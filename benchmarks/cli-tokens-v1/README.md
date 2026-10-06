# Command-line token benchmark v1: LogLens

A completely different application from the web benchmarks: a command-line
log analyzer ([SPEC.md](SPEC.md)). It parses about 250 Common Log Format
lines, counts by key, ranks, and prints a fixed text report or JSON. Golden
outputs come from a seeded oracle (`oracle.py`, which benchmark agents may not
read): `loglens sample.log` must print [expected.txt](expected.txt), and
`loglens sample.log --top 3 --json` must print [expected.json](expected.json).

## Round 1 (baseline, `f106fcebe`)

| | TypeScript (Node, no deps) | SEMAPRAX |
| --- | ---: | ---: |
| Completed | yes, all tests pass | **no** |
| Turns | 6 | 76 |
| Net task input | 104,540 | 4,363,904 |
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
