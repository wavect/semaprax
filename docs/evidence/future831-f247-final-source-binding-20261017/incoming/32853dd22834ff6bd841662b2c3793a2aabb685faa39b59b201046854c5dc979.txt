# Live source-audit custody

Status: private source-custody evidence; full compiler qualification remains pending.

Audience: compiler maintainers and source-custody reviewers.

The selected-session and legacy-nominal source controls bind their complete live
inputs directly with `include_str!`. Their earlier source-reader definitions are
also inputs to the historical CI formatting registry. This packet preserves those
two complete earlier definitions and their fixed registry hashes, so historical
readers can authenticate the live files before replaying the unchanged formatting
inverses.

`inverse.json` maps each complete current file to its complete archived definition.
The original twelve receiver proofs and the formatting registry are unchanged.
The shared source-reader controls check all fourteen inputs, archived bytes,
predecessor hashes, changed-input refusal and forged-predecessor refusal.

The earlier arena, receiver, selected-session, nominal and lifecycle evidence
packets retain every original edit, original archive and original hash. Additional
exact inverse spans account for later source-inventory additions. Each owning
control keeps its existing source projection: direct live inputs where declared,
and authenticated historical CI projections where declared. No runtime assertion,
builder limit or source-audit assertion is removed.

These are source-custody controls. They do not establish successful execution of
the complete compiler, HIR replay, allocation observers or the full quality profile.

Validation executed seven unchanged owning source-control tests and the two
unchanged source-archive helper bodies with the actual `source_reflows` module,
including all four shared source-custody and mutation controls. Each was compiled
and run separately with static standard-library linkage. The highest observed
compiler child-tree RSS was 202,048 KiB, within the 200 MiB guard; the highest test
RSS was 13,504 KiB. Independent reconstruction also checked all 75 complete inputs,
retention of every prior edit and fixed archive hash, and changed-byte refusal.
