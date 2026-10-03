# Foreign Law Trust Frontier v1

Status: bounded LAW-09 implementation tranche. The completion matrix records the
executable gate and remaining source/proof integration. This report does not
certify a foreign implementation.

`native_rust_binding::foreign_law::derive` consumes one existing
`ScalarBindingPlan`, checks it again against the exact resolved Rust import,
and binds its Cargo alias, package name/version/source digest, index digest,
feature digest, Rust path/signature/receiver, target, and physical symbol. A
Project route derives the canonical dependency-lock digest from retained
Project state and finds the selected import in authenticated linked HIR. The
caller supplies the generated adapter's exact digest and selected runtime
target. Both are reported and included in the summary identity; only the
adapter owner can authenticate those bytes. The Project route returns a
read-only JSON view under the ordinary before/after held-input recheck. It
neither builds nor invokes the Rust package.

One declared summary has a stable assumption ID, proposition digest, explicit
assumptions about effects, callbacks, panics, and shared state, and an optional
inclusive i64 return range. These are **assumptions**, even when a signature,
Cargo build, or test succeeds. A strict law refuses them (`SPX-FL303`). A law
that explicitly permits them receives the exact condition IDs, such as
`foreign.combine.behavior:no_callbacks`, to retain transitively. An absent
condition refuses (`SPX-FL302`). A theorem-requiring law always refuses
(`SPX-FL301`); this profile has no externally checked foreign proof token.
The existing LAW-06 solver path continues to refuse foreign calls.

`replay` rederives the report. A different lock, feature set, target, symbol,
adapter digest, proposition, or declared behavior refuses. Report fields are
private so a caller cannot construct a guard-bearing report directly.
`guard_i64_return` checks the exact retained inclusive range in the read-only
API; a seeded out-of-range return refuses with `SPX-FL306`. In the generated
SDK, the same range check runs after the physical foreign call and before the
value crosses into Semaprax. A violation returns nonretryable import status
code 40909 in the import's declared failure domain. The existing scalar import
ABI admits the Import transport class; a Contract-class import status is
rejected as an adapter failure. A successful value check is runtime evidence
for that value only. It says nothing about hidden side effects, callbacks, panics, or
shared-state mutation, including effects that happened before refusal.

The small diagnostic view shows its exact identity fields, conditions, guarded
range, all four behavior categories as `unknown` or `assumed_absent`, and
`foreign_internals_proved: false`. It is deterministic and grants no
execution, process, network, filesystem, publication, or signing authority.
The bounded diagnostic tests live in `native_rust_interop_v1 foreign_law::`
(3/3) and `project native_rust_scalar_callback::authenticated_project_foreign_law_view_retains_lock_and_conditions`
(1/1). The exact generated-SDK test
`public_sdk::indexed_tests::indexed_project::guarded_indexed_project_sdk_checks_physical_return_before_semantic_publication`
passed 1/1 with `--locked --offline`, one Cargo job and the private
`target/law09`. It compiled and ran the published SDK against a real Rust
implementation: an in-range call succeeded, a seeded out-of-range call
returned exact import status 40909, a wrong selected import refused before
publication, and the frontier's adapter digest matched the published manifest.

A second, deliberately narrow route derives a conditional caller certificate
from checked Project HIR. It admits only a named public i64 export whose body
directly returns one guarded foreign import call with scalar parameter or
literal arguments. Local statements, arithmetic, branch conditions, extra
calls, contracts and yielding refuse. The certificate replays exact Project
graph and source identity, selected binding, adapter digest, declaration and
law; all requested foreign behavior assumptions remain in its condition list.
It proves the direct source route, not the Rust implementation or an executed
runtime call.

`LawSelector::ForeignGuardedCaller` binds the public caller, import and exact
range into the protected LawSet inventory. Coverage stays open. The read-only
`ForeignCallerCertificate::verify_published_guard` checks an explicit
published SDK package against an independently held builder manifest digest:
Project revision, graph, target, all eight listed file hashes and generated
return guard must match. Forged digests and changed files refuse with
`SPX-FL310`. This read-only check does not create a strict LawSet token because
its expected digest is supplied by the caller. The guarded builder now retains
the exact frontier in its private `ProjectNativeRustSdkBundle` and can issue
`GuardedForeignCallerEvidence` only after matching the caller, source, target,
manifest digest and all package bytes. Its replay repeats those checks. This
is a builder-owned publication token, but no core LawSet or strict route yet
consumes it; protected coverage remains open. Guard source authentication is
not evidence that a call executed.

No externally validated theorem identity is provisioned in this profile;
theorem requirements continue to refuse until a separately checked semantic
association exists. Exact published-package replay does not claim an OS
sandbox against concurrent same-principal mutation. Tests and signatures
alone never establish the foreign implementation's behavior.
