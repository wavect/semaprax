# Law trust-chain view v1

`proof_export::render_trust_chain_view` emits
`semaprax.law-trust-chain-view.v1` for one exact Lean certificate. It first
replays the certificate against current source and regenerated Core Wasm, then
checks caller-supplied artifact bytes. A caller may supply the pinned Lean
kernel to replay its result. These checks finish before any JSON is returned;
source, profile, compiler, target, theorem, or artifact drift refuses the view.

The JSON has seven ordered conceptual links: `law_statement`,
`normalized_subject`, `checked_source_semantics`, `external_proof_result`,
`compiler_lowering_identity`, `artifact_binding`, and `runtime_boundary`.
Each link carries an explicit status. Without a kernel capability, proof is
`recorded_only`; with successful exact replay it is
`proved_by_replayed_kernel`. Lowering is always
`trusted_unproved_lowering`. Artifact bytes are `checked_exact_bytes`.
Runtime is `unexecuted` with no adapter identity. A consumer must not infer a
runtime result or translation theorem from either the proof or artifact hash.

The scalar differential test in
`tests/scalar_status_backend_equivalence/differential/law_runtime_chain.rs`
compares a hand-written checked-i64 reference, the interpreter, and actually
emitted Core Wasm executed by Node. It covers overflow, signed minimum,
short-circuiting, a returned value, and pre/postcondition guard failures.
A seeded wrong emitted value must be reported as a translation discrepancy;
it is not a disproved source law. This is sampled test evidence for this
admitted scalar export profile. Authored records and variants are currently
refused by that export profile and require their owning runtime route.

Future formal lowering is separate work: a machine-checked simulation or
preservation proof would need to relate checked source steps, compiler IR,
emitted Wasm steps, host imports, and runtime observations. This view does not
claim one. Runtime adapters and foreign target observations must be bound to
their own exact identities and execution evidence before that link can gain a
checked or tested status.
