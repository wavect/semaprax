# Intrinsic controller corrections

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only successor of frozen 85286. No production implementation changes, no Rust execution, and no old archive or diagnostic/assertion weakening.

The original intrinsic fixture placed `own` before the parameter name. The admitted positive places it after the colon (`value: own Iter<i64>`). The byte-exact invalid original is retained as an explicit full SPX-P106 diagnostic control (including span/path/help). Both original admitted receiver controls remain unchanged beyond this syntax correction.

The constructor whole-source inverse now matches the actual rustfmt module-registration order; its complete archived owner is unchanged. The semantic controller previously returned an owned Diagnostic through the allocation observation, so its backing survived the observed lifetime. It now compares every Diagnostic field and drops the complete result within the same observation. All refusal, first-refusal, invalid/live, allocated/freed, high-water and limit assertions remain unchanged. This fixes the observation scope; it does not claim production error backing is fully funded.

Complete pre-correction controller files are archived and pinned by a new full-source reconstruction control. The existing intrinsic projection also records the exact receiving-Drop hook, preserving its earlier complete archive/digests. No new Rust child joins are needed. Exact owning selectors are in pending-selectors.json. Root must compile and run these owners; positive fixture admission and all repaired owners are unverified here.
