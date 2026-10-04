# Law trust-chain view v1

Status: bounded exact-subject proof trust-chain view.

Audience: Project authors and maintainers evaluating bounded law and proof support.

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
`render_trust_chain_view_for_target` accepts an explicit requested target and
adapter identity; it refuses any target other than the certificate's Core
Wasm target and refuses every adapter identity because the certificate has no
authenticated runtime adapter association. Both checks happen before source
or artifact replay, so a wrong association cannot inherit another target's
result.
Each proof/lowering/runtime link names its trusted base: the caller-supplied
kernel capability, the exact compiler/code generator, or the absence of a
runtime executor and adapter observation. A caller that supplies a counterfeit
kernel capability is outside this API's trust boundary; the view does not
authenticate that caller.

The focused hostile ladder reseals certificate payloads after changing the
Lean statement, compiler version, profile, or artifact target, and separately
changes current source body, source law statement, and artifact bytes. Every
change refuses the view. A requested foreign target or adapter identity is
also refused before certificate replay. The compiled Core Wasm profile has no
caller-selectable lowering options; different emitted bytes fail the exact
artifact binding. This test covers association refusal, not an authenticated
runtime invocation receipt.

The scalar differential test in
`tests/scalar_status_backend_equivalence/differential/law_runtime_chain.rs`
compares a hand-written checked-i64 reference, the interpreter, and actually
emitted Core Wasm executed by Node. It covers overflow, signed minimum,
short-circuiting, a returned value, and pre/postcondition guard failures.
A seeded wrong emitted value must be reported as a translation discrepancy;
it is not a disproved source law. This is sampled test evidence for this
admitted scalar export profile. The separate `tests/wasm/law_runtime_structured.rs`
gate runs one authored record and variant through the interpreter and emitted
Core Wasm under a hand-written result reference, with a seeded changed body.
The scalar public export still refuses authored records and variants; the
structured gate exercises the general Core Wasm runtime route and does not
promote them into that export profile or assert an aggregate proof.

Future formal lowering is separate work: a machine-checked simulation or
preservation proof would need to relate checked source steps, compiler IR,
emitted Wasm steps, host imports, and runtime observations. This view does not
claim one. Runtime adapters and foreign target observations must be bound to
their own exact identities and execution evidence before that link can gain a
checked or tested status.

The Lean export's admitted subset excludes calls and declared effects, so no
foreign-boundary theorem is issued in this profile. The export reports those
declarations as unsupported with a closed reason, and the runtime link remains
unexecuted for them. The scalar differential fixture exercises retained
precondition and postcondition guards plus checked arithmetic failures in
actual emitted Wasm. Its hand-written reference and interpreter results agree
with the observed failures; no accepted source theorem removes those guards.
