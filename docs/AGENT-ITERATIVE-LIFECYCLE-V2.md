# Agent iterative lifecycle v2

Status: local, partial; six focused iterative lifecycle tests pass.

Audience: compiler contributors and runtime integrators.

`agent_lifecycle::iterative::compile_agent_lifecycle_v2` binds the checked
initialize, observe, authorize and reduce operations from an unchanged
AgentDefinition v1, plus one explicitly selected persistent Step type identity.
It independently checks the whole module and uses the ordinary retained
interpreter preparation and execution path for every deterministic stage.

Step is a monomorphic authored variant with exactly Continue, Complete,
Suspend and Fail cases. Continue and Suspend carry the exact State record's
flat fields in declaration order; Complete carries the Result record's flat
fields in declaration order; Fail carries one i64 code. Field types and every
Step/case/field identity are checked. The admitted leaves are Bytes and the
retained seam's five scalar types. Nested carriers remain outside this profile.
The mapping is derived from checked declarations, never provided by a caller.

Execution initializes once and repeats observe, scripted proposal decoding,
authorize, injected read, and reduce. Only a checked reducer return selects the
next transition. Continue feeds its State to the following turn. Complete,
Suspend and Fail publish their terminal carrier and stop. Suspension is data;
this version does not accept it as durable restart authority.

Every turn runs the authorize stage anew. Its opaque, consumed grant binds the
source-revision-bearing lifecycle digest, turn ordinal, exact State, canonical
proposal, grant case and seal. Cancellation is checked at each deterministic
stage and before dispatch. A caller supplies the only host read implementation;
there is no ambient authority. A failed effect never reaches reduce.

The source-selected `compile_source_agent_lifecycle_v2` bridge derives the
Definition from the same checked module and selected Agent identity.

Caller ceilings bound iterations, deterministic stage count and interpreter
fuel per stage. Hard ceilings of 4096 iterations and 12289 stage records bound
the allocation regardless of caller input. Reducer capacity is reserved before host dispatch. Evidence
binds a length-framed invocation digest of exact task bytes, task budget, all
ordered proposal bytes and all three execution ceilings before any stage
boundary. It records actual stage order, turn and effect counts, authorization bindings and
a terminal-carrier digest, without exposing payloads. Its schema and digest
domain are additive v2; all existing Lifecycle, Definition and Runtime v1
artifacts remain unchanged. This is a local retained-interpreter profile;
typed operation registries, per-operation durable recovery and hosted clients
remain separate gates.

Focused gate: `cargo test --locked -p semaprax --all-features --lib
agent_lifecycle::iterative::tests` (six cases).

The canonical v2 document explicitly records initialize-once, the iteration
order, Continue targeting observe, terminal cases, and exact Step case/field
mappings. It does not embed the v1 lifecycle wire or acyclic-only nonclaims.
