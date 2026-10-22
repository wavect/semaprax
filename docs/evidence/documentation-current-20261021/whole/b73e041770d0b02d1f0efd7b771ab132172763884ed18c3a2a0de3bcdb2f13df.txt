# Literal acceptance supplement for #877 and #878

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

All **23 exact checks passed**, using the existing library artifact compiled at `f1b24eda3f98e19c294f50eff388f642eabc7ccb`. The actual checkout was `9334eec5a785ed8b67a3698da019fbabd0cea01a`; every selected whole source file was byte-identical and the checkout delta consisted only of the preceding qualification documentation. No Cargo command, rebuild or full suite ran for this supplement. Each test ran serially with process-group resource monitoring and settlement.

The original focused run remains 97/112; these results supplement it and do not erase its 15 failures. Actual application memory-limit outcomes remain failed. There is no new agent token, turn, billed-cost, wall-time speedup or application-fit claim.

The supplement exercises the omitted complete old/current HIR contents, ordering, both comparators, owned corpus, source custody, long-tail boundaries, six-field refusal/Drop behavior, borrowed merge keys, owned-suffix handoff and the genuine 65,536/65,537 route boundary. See the exact selectors and hashes in `qualification.json`.

## #878 measured trade-offs

These are the actual fixture outputs; `held` retains the test's metric name. Allocated is cumulative through destruction. Every reported heap measurement finished live=0, allocated=freed and invalid=false in its owning check.

| Fixture | Metric | Independent old | Current |
|---|---|---:|---:|
| Borrowed merge, paths | held | 12,520 | 54,604 |
| Borrowed merge, paths | allocated | 9,152 | 8,224 |
| Borrowed merge, paths | highwater | 5,696 | 4,744 |
| Borrowed merge, calls | held | 18,848 | 56,212 |
| Borrowed merge, calls | allocated | 11,984 | 9,832 |
| Borrowed merge, calls | highwater | 7,528 | 6,088 |
| Owned suffix | held | 40,683 | 94,691 |
| Owned suffix | allocated | 28,256 | 22,248 |
| Owned suffix | highwater | 13,424 | 13,184 |

The eliminated copies reduce heap allocation in these fixtures, but new receiving/lookup controls increase charged bookkeeping. This is not a net builder-byte improvement. Further reduction of that overhead is separate follow-up work; no fee may be removed without eliminating its actual receiving role.

All ten previously unrun dedicated #878 owners passed here, in addition to its original copy-before-work owner in the earlier run. Their full independent languages/work/order, owner destruction order, exact physical refusal/floor/foreign/sticky diagnostics and Drop assertions remain unchanged.

## #877 remains open

All thirteen omitted #877/boundary owners passed, and the sequence-only 4,096/8,192-element fixtures report lower paid/heap costs. The one-element fixture has identical costs. These sequence-only outputs explicitly exclude the comparator, so they do not quantify total comparison-work overhead or establish an application improvement. A test-only reporter at the real old/current comparator boundaries is being added to the next large source batch before claiming this remaining acceptance requirement complete.
