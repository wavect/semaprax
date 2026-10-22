# #836 actual677 compile correction — test only

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Immediate source677e658f6f3671fdcc5942c1f3522598d83ab7b2. The actual101 build
failed with exit101 before any Rust test body ran; two reported E0599 errors
were in the new test at lines247/248. `child_count`/`child` belong to
ResolvedStatement, not ResolvedExpr. The error census is retained byte-exact;
parent retains the whole failed build history. Other13errors are a separate
weighted-lane old-copy wiring fix.

Replace only that erroneous test traversal with the existing public(crate)
HIR lexical_exit::ChildCursor, whose Iterator yields borrowed ResolvedExpr
children, including block statements/tail and while condition/body. The test
still visits every nested store and asserts exactly one store with exactly
three runtime children/one type argument. All source success/graph/cache/
interpreter-result and arity/selector/type stable negative checks are unchanged.
No new body/selector, production behavior, fixture or semantic assertion changes.

The shared cfg(test) source helper adds a complete current→677 first hop before
every final_ready_input named domain. Two full sources (test and source helper)
restore exact677; all old JSON/archive/assertions stay immutable. Future MAIN or
other fix assembly must authenticate final→this checkpoint independently before
these expected hashes, preserving the separate MAIN/generic cursor domains.

Actual bounded Python command:
`python3 docs/evidence/field-update-hir-visitor-836-20261015/verify.py`
It passes both full inverses, the exact one-expression test-body replacement,
and the entire unchanged prior joined source proof in its two authenticated677
inputs, retaining25old836checks/222protectedpacket files. Source-only proof is
not a compile/test pass. No Cargo, test, Clippy or Node execution occurred.

Existing selected owner remains:
`vec_ops::field_update::tests::while_source_selector_is_metadata_and_hir_keeps_three_runtime_children`.
