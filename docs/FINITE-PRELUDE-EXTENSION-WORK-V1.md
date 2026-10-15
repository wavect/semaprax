# Finite Prelude Extension Work v1

Audience: compiler contributors, tool authors, and reviewers.

Status: private source implementation, not compiled or executed. The actual
ordinary core-module receiver retains the complete selected extension suffix.
All prior no-owner APIs and complete old constructors remain independent.
No P, graph/cache schema, byte/physical cap or replay work bound changes.

This is a NEW compiler-internal finite work contract. The base producer's
FixedSchemaPayer, two-type/nine-descriptor work_model and full old controls stay
unchanged. No old account is reset or enlarged; no successful work bridge or
process-global payload cache is introduced.

The live Invocation owns one inline nine-slot catalog in compiler source order:
Vec, Box, Iter, IterStep, List, ListStep, StdinReader, Map, Set. Source verification
still precedes activation. Only exact immutable compiler AST prefixes of lengths
2,3,4,6,8,9,11 are accepted. Each slot is acquired at most once in that invocation,
only when selected. Exact source-object coordinates select its complete payload;
matching ID/name bytes in authored or decoded data grant no authority. Prefix
two performs no extension construction. A later larger selected prefix acquires
only its new slots, and no failure permits rebuilding any counter.

Each slot installs its own ExtensionPayer on the same byte callback, charging
its actual Self/Result/receipt carriers before work. The inline catalog and
mapped receiving results are also paid before initialization. Each allowance is
the literal-operation census in extension_model.rs, consumed at actual sites in
extension_producer.rs:

```
type = 3 entry/kind/parameter-shape visits + identity bytes + 2*type-name bytes
     + sum(parameter initializer + 2*parameter-name bytes)
case = 1 initializer + identity bytes + 2*case-name bytes
field = 1 initializer + identity bytes + 2*field-name bytes + type traversal
T traversal = 1 enter + len(T) + len(parameter-name)
Nominal<T> traversal = 1 enter + ordered prior-name comparison bytes
                    + T traversal
variant = type + sum(case + sum(fields))
```

The bound for each actual type is independently derived from its literal
compiler producer, including both complete variants and their nominal argument
fields. The actual producer resolves dependencies through already-held earlier
slots; comparisons debit both byte lengths before comparing. The fixed selected
prefix bound is the sum of its slot bounds, not a generic allowance. Successful
consumption must equal every bound exactly. A changed schema requires a contract
and source-proof update; no numeric margin is permitted. Work cannot finance
base construction, authored resolution, replay or downstream activity.

Names use the base producer's two copy/validation passes; identities use its
funded identity constructor. Whole parameter/case vectors and nominal owners are
funded before publication. Record fields are genuinely empty. Partial variant
fields use paid inline guards whose iterative disposer follows the exact one-
argument compiler shape; changed/deeper shapes fail before construction. All
map/name/view receivers in each module remain separately paid. Cases and their
nested fields are retained in one canonical case owner, without per-field Arcs.

The counter authenticates the same original byte owner with zero bytes before
each work debit. It retains its exact first FixedFailure<E:Copy>, including a
Detached byte tag across scope restoration, without requery or Clone. Refused
work records requested/remaining once. Invocation retains first ExtensionWork
separately from base Work and Bytes and never reconstructs either failed owner.
A successful cached module use still authenticates its actual receiving owner;
new metadata/map/string and payload-view carriers are paid in that module.
No heap is allocated merely to clone the already-funded nominal owner.
Failure is sticky for the WHOLE Invocation: a later base-only request cannot
recover after an extension error. "Base two unchanged" describes its schema,
producer and exact work bound, not permission to ignore a selected invocation
failure or reuse a successful base sub-result after another producer refused.

The outer distinct ExtensionWork branch publishes the fixed H006 text
`prelude extension producer work bound refused` by moving the existing paid
IdentityReservation buffers; a const length check proves it fits their original
capacity. No byte Refused is fabricated or rich error formatted after refusal.

Required controls: complete old/current indexes and wire over every existing
prefix; repeated same-invocation module pointer equality and unchanged producer
receipt; explicit none/foreign AST views; actual constructor plus final Drop in
one observer; exact/short/zero bytes/work, every acquisition refusal, sticky work,
foreign/restored byte tags, and the full ordinary source/HIR/diagnostic controls.
These source-authored controls are not runtime evidence or an allocation/peak
saving claim. The fixed base work model, corrected P and old receipts persist.

## Final-link lifetime decision

The earlier core budget ends at workspace_graph.rs4115..4162 before the returned
WorkspaceGraphBuild is constructed. Later final-link APIs accept no payer/lease;
retaining its Rc cannot reactivate that spent invocation. This packet therefore
optimizes only the actual live core extension path, not that final-link boundary.
A later paid final-link receiver needs its own explicit real caller authority.
