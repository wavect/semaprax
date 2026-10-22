# Complete corrective-batch source composition

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Frozen Rust/source basis: `d39fe676a7c1df3323888b25c9481f7bbb65feaa`. All Rust, SEMAPRAX source and Cargo files remain byte-exact to that basis. This source-only proof does not claim compilation, runtime acceptance or benchmark gains. The actual failed 23e8 build (five errors, nine warnings, zero test bodies, no production compile) is preserved separately.

The new bounded compositor starts from **all 84 previous active packets**, retaining the existing derived manifests, then adds the mechanical corrections, exact foreign diagnostic strengthening, selected Bytes classification, Source36 adapter and integration digest correction. Result: **90 packets / 795 rows**, 46 additive full authenticated checkpoint joins, every old original/hook preserved, and all 36 complete source inverses passing. The Git LRU peak was 16,775,517 bytes with its unchanged 16 MiB/128-entry bounds. Historical scripts, proofs and archives stay exact.

Independent joined verification passed: 3,217 old hooks retained, all 53 independent receivers unchanged, both original capacity bounds retained, and all 32+6 archived starter patch paths restored to their immutable full sources. All 38 current starter bindings passed, with one added authenticated join for the explicit starter module path. The old binding file remains untouched.

Verification commands from repository root (no Rust execution):

```sh
python3 docs/evidence/incremental-source-composition-d39fe67-20261015/compose.py --verify-only --basis d39fe676a7c1df3323888b25c9481f7bbb65feaa --extra-packet docs/evidence/source36-after-23e8-20261015/source-inverses.json --extra-packet docs/evidence/source36-after-23e8-20261015/source-projections.json --extra-packet docs/evidence/field-update-integration-digest-correction-23e8-20261015/source-inverses.json --source36-runner docs/evidence/source36-after-23e8-20261015/verify_source36.py
python3 docs/evidence/incremental-source-composition-d39fe67-20261015/verify_joined.py --basis d39fe676a7c1df3323888b25c9481f7bbb65feaa
```

Next: fresh exact-source registration and qualification using V2 tooling (original module parser with real explicit paths). Preserve all 144 pending owners and add only the two directly affected selected Bytes classifier controls. No full suite or gates; unit-first serial build, exact runtime owners, scoped production Clippy and actual pin regeneration remain pending.
