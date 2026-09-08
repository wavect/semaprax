# Generic Authored Variants v1

Status: local source/HIR/graph, cleanup-oracle, and all 18 runtime profiles pass
on the interpreter, native C11 O0/O2, and Core Wasm. Private Project/ProgramRoot
replay and exact public-profile rejection also pass; no hosted or public ABI
promotion is claimed.

Audience: compiler contributors and reviewers.

This additive private profile admits effect-free generic functions over authored
flat variants such as `Choice<Bytes, T>`, with exactly one owning parameter and
one explicit type parameter. `T` is instantiated with exactly `i64`, `i32`,
`u8`, `usize`, `char`, `f32`, `f64`, or `bool`. Every substitution must satisfy
the existing concrete owned-byte variant profile: one case owns direct `Bytes`
fields, and all remaining fields are Copy scalars. The compiler-owned `Option`
and `Result` identities retain their separately versioned admission.

Functions may relay the owning carrier, reconstruct an active case through an
exhaustive unguarded owning match, compose that match as a call argument, and
join owning `if` branches. Borrow matching returns a Copy scalar while retaining
the enclosing parameter's owner until normal lexical settlement. The result is
an admitted authored carrier or a Copy scalar; extra parameters are Copy values.
This does not add generic inference, constraints, nested owned payloads, public
aggregate ABI declarations, or an ambient allocation capability.

Source verification independently checks every concrete substitution. HIR checks
scoped generic parameter identities and exact declared case and field inventories,
constructor field order, pattern binding identities/types/modes, exhaustive cases,
and full materialized function meaning. Cleanup lowering moves the selected arm's
result into the match result before exiting the arm scope. Independent replay
rejects altered ownership and transfer facts. Native and Wasm joins consume
canonical continuation transfers in their recorded order; they do not replay
unselected arms or duplicate an already executed branch transfer.

The executable corpus belongs to `language::generic_authored_variants_next`,
`cleanup_plan::build::generic_variant::tests`, and
`owned_data::generic_owned_function_runtime::authored_variants`. Runtime coverage
includes both Data/Empty cases, all eight scalar markers, both owning `if`
branches, an owning match used directly as a call argument, borrow observation,
payload inspection, and precondition failure selected at each scalar instance.
Native probes count payload allocations and check exact failure status and result
sentinels at O0/O2. Wasm reentry uses a bounded two-entry owned-byte host and
compares bulk-copy counts with the corresponding bypass projection. Passing
local fixtures does not constitute hosted or public ABI promotion.

Private Project HIR linking retains exact authored variant signatures and stable
declaration facts. Selected public roots still pass the existing Public Scalar
Export Profile v1 checks. Exposing an owning variant reaches that public boundary
and rejects with SPX-W115; the earlier SPX-H006 came from rejecting private
linking before the public profile was evaluated.
