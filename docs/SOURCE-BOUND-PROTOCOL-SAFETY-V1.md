# Source-bound finite protocol safety v1

Status: bounded LAW-10 profile. The completion matrix records the executable
result and remaining law-policy integration.

This profile reuses a retained Project `session protocol` declaration and the
ordinary checked-HIR interpreter. Every transition must name the same stable
`via` dispatcher ID. The dispatcher is an explicit, pure, effect-free
`(state: i64, event: i64) -> i64` function. State codes follow the declared
state order; event codes follow the declared transition order. `-1` means the
event is disabled. A nonnegative return encodes `next_state * 2 +
charge_command_bit`. The bit must match whether the declared event has the
selected charge label. The checker executes the dispatcher for **every**
finite state/event pair and compares it with every declared transition and
every disabled pair before exploring. A missing `via`, uncovered transition,
foreign/effectful dispatcher, choice transition, out-of-domain return, or
incomplete source execution refuses. This gives a concrete, exhaustive
association for this narrow pure dispatcher, not a hand-authored edge list.

The existing deterministic breadth-first `check_safety` engine explores the
source-derived table. Its state tracks the declared protocol state, whether
the selected success state has been reached, and whether a charge command was
emitted afterward. A reachable post-success charge violates
`no_charge_command_after_success`. Retries and failures are ordinary declared
transitions. `ModelChecked` is reported only on fully closed finite
exploration; an exhausted state, transition or depth bound remains a separate
incomplete result. No fairness assumption is admitted. The report binds the
retained Project revision, protocol source digest and stable ID, dispatcher ID,
ordered state/event domains, initial/success states, charge label, complete
transition coverage, fairness `none`, all three bounds, source-derived table,
and exploration counters. Replay executes the source and checker again.

A counterexample names each source/protocol transition by state, label, target,
`via` ID, source path and line. The checker re-executes each trace step through
the admitted source dispatcher and checks the reached invariant. A failed
replay remains `abstract_only`; only a successful replay is labeled
`concrete_source_replay`. The read-only authenticated Project route performs
its ordinary before/after held-input checks and returns canonical JSON.

This is a finite safety claim about one pure source dispatcher for one modeled
request. It does not assert that all callers use that dispatcher or that a
payment provider delivers, settles, or charges exactly once. It grants no
payment or other effect authority, and it proves no liveness or availability.
A protocol `via` binding alone still attests only checked-node identity, not
message ordering. The profile refuses effects and foreign calls rather than
assuming them away. Choice branches and general state-rich source functions
remain outside this initial structured profile.

The focused gate is `cargo test --locked --offline --test project
source_protocol_law::` with one Cargo job and a checkout-private target.
Remaining work includes attaching this source-bound result to protected LawSet
policy and extending the structured source association beyond the narrow pure
scalar dispatcher without treating an abstract model as a source theorem.
