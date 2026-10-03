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
`guard_i64_return` checks the exact retained inclusive range after a physical
foreign call and before publishing that value. A seeded out-of-range return
refuses with `SPX-FL306`. A successful value check is runtime evidence for that
value only. It says nothing about hidden side effects, callbacks, panics, or
shared-state mutation, including effects that happened before refusal.

The small diagnostic view shows its exact identity fields, conditions, guarded
range, all four behavior categories as `unknown` or `assumed_absent`, and
`foreign_internals_proved: false`. It is deterministic and grants no
execution, process, network, filesystem, publication, or signing authority.
The bounded tests live in the existing `native_rust_interop_v1` integration
harness as `foreign_law::` (3/3 passed) and the existing `project` harness as
`native_rust_scalar_callback::authenticated_project_foreign_law_view_retains_lock_and_conditions`
(1/1 passed). Both used `--locked --offline`, one Cargo job, and a private
checkout target under `target/law09`.

Remaining LAW-09 work: authenticate adapter artifact bytes at the physical
builder/invocation boundary, retain the guard in generated SDK calls, connect
conditional foreign conditions into actual caller proof certificates and
protected LawSet policy, and admit externally validated theorem evidence only
through a separately checked semantic association. Tests and signatures alone
must never satisfy that requirement.
