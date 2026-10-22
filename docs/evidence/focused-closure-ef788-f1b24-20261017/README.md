# Focused compiler qualification, 17 October 2026

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Production source: `ef788f8237beeb16dcc1cfa9346fc1dbb6f69a4d`.
Earned-pin consumer source: `f1b24eda3f98e19c294f50eff388f642eabc7ccb`.

This is compiler qualification, not a live agent benchmark. It establishes no token, turn, billed-cost, application-fit or language-speedup advantage.

The five explicit owning build commands passed, with one Cargo job and private SSD artifacts. The serial selected runtime completed **97 passes and 15 failures out of 112**; no checks were omitted or marked ignored. Production-only `cargo clippy --locked --offline -p semaprax --lib --bin semaprax -- -D warnings` passed. No full gate or full test suite ran. The compact JSON lists every selected outcome and original receipt/log hashes.

The 15 failures remain failures: eight application/frontier cases (codec, schedule, catalog, normalization and NDJSON), the retained source/HIR authority check, two native codec cases, two paid-comparison fixture/ledger cases and two branch-capacity cases. The JSON is authoritative for the exact list. In particular the catalog tests named for 23 cases did not demonstrate completion of all 23 cases. Required application checks still meet the unchanged 64 MiB refusal frontier. #808 and unfinished dependent issues remain open.

## Actual arena trade-offs

Values below come from the unchanged complete arena owner in this run. `allocated` is cumulative allocation through destruction; `highwater` is peak live allocation. Every row finished with live=0, allocated=freed and invalid=false.

| Shape | Receiver | Reserved bytes | Allocated bytes | Highwater bytes |
|---|---|---:|---:|---:|
| Duplicate-heavy | Independent old | 4,513,594 | 745,221 | 34,533 |
| Duplicate-heavy | Current | 4,512,788 | 655,701 | 35,749 |
| Unique wide | Independent old | 918,794 | 152,141 | 30,885 |
| Unique wide | Current | 987,794 | 163,573 | 34,373 |
| Singleton | Independent old | 11,682 | 1,485 | 1,485 |
| Singleton | Current | 13,066 | 957 | 957 |

The arena reduces duplicate-heavy cumulative allocation and introduces unique-input metadata overhead. These fixtures do not establish application savings.

## Earned pins

The actual production artifact generated ten managed wires and both browser fixture wires. Read-only plans restored every complete historical wire after classifying only builder billing and artifact self-digests. Exactly eleven managed literals and two browser digest literals were updated. Browser project/workspace revisions stayed unchanged. Complete old source tables and controllers remain independently retained; current 42/old 41 source ownership, frozen 174 membership, FINAL6 source controls and 184-owner boundary controls passed their finite source checks. The production Rust delta between the two heads is confined to modules already gated by `cfg(test)`; production Clippy is not relabeled as a new-head producer build.

All nine exact pin consumers passed at the consumer head: three managed owners and six browser owners. An independent literal issue audit found six acceptance checks missing from the initial 112 selection. All six passed on the same current library artifact, with no additional build. This supplements the original 97/112 result; it does not turn its 15 failures into passes.

## Issue scope

#876 corrects the selected successor walk and restores the unchanged exact-work arena owner. All three additional selected behavioral checks passed after independent closure review. It withdraws the selected outer-directory saving; it makes no ordinary-language gain claim.

#875 adds ordinary PLAN persistent segments with complete old comparison/materialization/work/refusal controls. Its twelve current core owners passed, while application outcomes remain the recorded failures.

#854 adds paid canonical state arenas and borrowed signature lookup. Its eight required core/source owners passed, including the original arena owner recovered by #876; the table above reports the costs without claiming universal improvement.

Source-only repairs and the next #879/#880 implementation generation are separate from this report and remain unqualified here.
