# Language-Native Agent Object and Unified Harness v1

Audience: compiler contributors, Agent Runtime contributors, provider-adapter
authors, and semantic-workspace integrators.

Status: bounded phase-1 compiler slice implemented locally, extended by the
additive Agent Proposal Schema v1 grammar and decoder, the additive
AgentDefinition v2 / AgentDeployment v1 separation, the additive Agent
Lifecycle v1 compiled stage binding and single acyclic execution, and the
additive Agent Checkpoint v1 revision-bound durable slice over that lifecycle's
single external boundary; long-term language, harness, effects, and durability
goals remain proposed and unsupported.

## Purpose

The mature goal is for a Semaprax agent to be a compiled language object whose
semantic graph defines state, context construction, available actions, model
requirements, capabilities, budgets, transitions, validation, and evidence
obligations. A model implements a typed stochastic operation inside that
object. A small generic runtime interprets it.

This document freezes only the first additive implementation boundary:

```text
canonical AgentDefinition v1
        |
        +--> canonical AgentGraph v1
        |
        `--> byte-identical Agent Runtime Profile v1
                         |
                         `--> existing Agent<H>
```

The additive AGENT-03 frontend and lowering slice admits the closed `.spx`
Agent declaration described by [Language-native Agent syntax v1](LANGUAGE-NATIVE-AGENT-SYNTAX-V1.md).
It lowers through this unchanged canonical definition contract. Runtime v1
material is structured definition data; the
compiler, rather than the author, supplies its frozen schema and nonclaims.

For each admitted source Agent, Project construction retains one HIR-equivalent
Agent node and the existing compiler-produced AgentDefinition v1, AgentGraph v1,
and Runtime Profile v1 bytes. The default canonical workspace places those
exact products in its existing `AgentDefinitions` node. This adds no execution,
provider, tool, filesystem, process, network, or publication authority.

## Semantic object

The long-term object is conceptually:

```text
Agent<Task, State, Observation, Proposal, Outcome, Result>
```

It owns six roles:

```text
initialize(Task) -> State
observe(borrow State) -> Observation
model propose(borrow Observation) -> Proposal
authorize(borrow State, borrow Proposal) -> Authorized<Proposal> | Rejection
effect execute(own Authorized<Proposal>) -> Outcome
reduce(own State, Proposal, Outcome) -> AgentStep<State, Result>
```

`initialize`, `observe`, `authorize`, and `reduce` are deterministic.
`propose` is model-bound and stochastic. `execute` is the only effect role.
The current compiler records those identities and relationships but does not
execute the operations.

## AgentDefinition v1

The schema identity is `semaprax.agent-definition.v1`. A document is compact
UTF-8 JSON with exactly one terminal LF, no other line endings, no BOM, maximum
size 1,310,720 bytes, and maximum parsed depth 16. Objects are closed and key
order is canonical.

The top-level order and fields are:

1. `schema`: the exact schema identity;
2. `agent_id`: a canonical stable identifier;
3. `types`: the six type roles in normative order;
4. `operations`: the six operation roles in normative order; and
5. `runtime_v1`: structured model, tool, policy, and limit material for the
   bounded compatibility projection.

Canonical identifiers are 1–240 ASCII bytes containing only letters, digits,
`.`, `-`, and `_`.

### Type roles

Each closed type row contains `role`, then `stable_id`. The exact order is:

```text
task, state, observation, proposal, outcome, result
```

Every type stable ID is unique. These IDs are semantic identities rather than
display names or inferred aliases.

### Operation roles

Each closed operation row contains `role`, `stable_id`, then `kind`. The exact
rows are:

| Role | Kind |
| --- | --- |
| `initialize` | `deterministic` |
| `observe` | `deterministic` |
| `propose` | `model` |
| `authorize` | `deterministic` |
| `execute` | `effect` |
| `reduce` | `deterministic` |

Every operation stable ID is unique, and no operation, type, or agent identity
may collide with another. A model operation is not represented as a
deterministic function, and an effect operation is not represented as model
authority.

### Runtime v1 compatibility projection

`runtime_v1` is a closed object containing `models`, `tools`, `policy`, and
`limits` in that order. Its nested rows use the exact corresponding Runtime v1
field order and closed shapes. It does not contain a schema, agent identity, or
nonclaims: the compiler derives those from the definition and the frozen
Runtime v1 contract.

The compiler renders a canonical `semaprax.agent-runtime-profile.v1` document,
then admits it through the unchanged public `Agent::<H>::new` constructor with
a zero-authority validation host. It does not invoke the host, tokenize, run a
task, contact a provider, or invoke a tool. Therefore the existing Profile v1
schema, diagnostics, known answers, and runtime behavior remain authoritative
and byte-frozen.

The v1 compatibility projection deliberately keeps deployment and semantic
material together. The additive AgentDefinition v2 and AgentDeployment v1
contracts below split concrete provider/model binding from the source-owned
definition; v1 remains their exact projection.

## AgentGraph v1

The schema identity is `semaprax.agent-graph.v1`. The compiler emits compact
canonical UTF-8 JSON with one terminal LF and a maximum size of 1,572,864 bytes.
That independent output cap is the 1,310,720-byte definition cap plus 262,144
bytes of graph headroom. Stable identifiers occupy a fixed number of repeated
slots and are individually limited to 240 bytes. The compiler still measures
the completed graph and rejects `graph_bytes` above the cap; admission of the
embedded Runtime v1 profile alone does not waive that output bound.
Its ordered fields are:

1. `schema`;
2. `definition_digest`;
3. `agent_id`;
4. `types`;
5. `operations`;
6. `derived_types`;
7. `relationships`;
8. `model_contract`;
9. `context_plan`;
10. `proposal_contract`;
11. `capability_manifest`;
12. `effect_bindings`;
13. `limits`;
14. `approval_requirements`;
15. `terminal_conditions`;
16. `evidence_obligations`;
17. `references`;
18. `runtime_v1_profile_digest`; and
19. `nonclaims`.

The graph repeats the admitted stable type and operation nodes and derives the
following fixed relationship sequence:

```text
initialize CONSUMES Task
initialize RETURNS State
observe BORROWS State
observe RETURNS Observation
propose BORROWS Observation
propose RETURNS Proposal
authorize BORROWS State
authorize BORROWS Proposal
authorize RETURNS Result<Authorized<Proposal>, Rejection>
execute CONSUMES Authorized<Proposal>
execute RETURNS Outcome
reduce CONSUMES State
reduce USES Proposal
reduce USES Outcome
reduce RETURNS AgentStep<State, Result>
```

Graph-local `@authorized_proposal`, `@rejection`, `@authorization_result`,
`@suspension`, `@agent_failure`, and `@agent_step` nodes encode these structural
wrapper types. `@agent_step` records the exact payloads of `continue(State)`,
`complete(Result)`, `suspend(State, Suspension)`, and `fail(AgentFailure)`. `@`
cannot occur in an authored stable identifier. The authorized proposal is
explicitly opaque, runtime-minted, and single-use. Recording these boundaries
does not yet implement token minting, consumption, suspension, or failure
execution.

The model contract projects required locality, minimum quality, required model
capabilities, and the bounded v1 compatibility route. The context plan records
the fixed task/objective/context ordering. The proposal contract exposes the
closed v1 `final` and `tool` transport variants and allowed tools. Capabilities,
complete read-only tool effect bindings, limits, terminal conditions, evidence
schemas, and empty ProgramGraph/workspace/test/validation references are
directly inspectable. Concrete model locality and quality attributes, tokenizer
data, and price data remain deployment-like Profile v1 material bound by
`runtime_v1_profile_digest`, not AgentGraph model semantics.

## Agent Proposal Schema v1

The schema identity is `semaprax.agent-proposal-schema.v1`. It is the additive
compiler product that replaces the compatibility projection's authored action
schemas with a closed grammar derived from the program's own verified types.
It changes no AgentDefinition, AgentGraph, or Runtime v1 byte.

The compiler takes one checked `.spx` module and one canonical
AgentDefinition, resolves the definition's Proposal-role stable identity to an
actual record or variant declaration in that module's HIR — the same HIR
ordinary execution uses — and derives a closed schema and a typed decoder.

### Admitted subset

The first slice admits one closed monomorphic scalar record or variant:

| Declared type | Representation | Wire form |
| --- | --- | --- |
| `bool` | `bool` | JSON `true` or `false` |
| `i32` | `i32` | exact decimal string, `-2147483648`–`2147483647` |
| `i64` | `i64` | exact decimal string, `-9223372036854775808`–`9223372036854775807` |
| `u8` | `u8` | exact decimal string, `0`–`255` |
| `usize` | `u64` | exact decimal string, `0`–`18446744073709551615` |
| `string` | `string` | JSON string, at most 4096 bytes |

Exact integers travel as decimal strings so every consumer preserves values
outside the range a JSON number is guaranteed to carry, including values above
JavaScript's safe-integer bound. The accepted decimal is canonical: no `+`, no
exponent, no fraction, no surrounding space, no leading zero except the single
digit `0`, and no `-0`.

Everything else is rejected with an explicit diagnostic rather than widened: a
generic declaration, a class or resource declaration, an unresolved identity, a
non-persistent (`automatic`) field or case identity, an empty variant, and any
`unit`, `char`, `f32`, `f64`, `Bytes`, `Str`, `Slice<u8>`, fixed-array, type
parameter, or nested nominal field. A by-value recursive type never reaches
HIR; the resolver rejects it first. There is no loosely typed object escape
hatch.

### Documents

The schema document is compact UTF-8 JSON with exactly one terminal LF and a
maximum size of 262,144 bytes. Its ordered fields are `schema`, `agent_id`,
`proposal_type_id`, `proposal_type_revision`, `shape`, `wire`, and `nonclaims`.
A record `shape` carries `kind` and ordered `fields`; a variant `shape` carries
`kind` and ordered `cases`, each with its own ordered `fields`.

The document carries semantic identities and exact representations only. No
display name enters it, so a display rename of the type, a field, or a case
preserves both `proposal_type_revision` and the schema digest, while an actual
type change — a different representation, an added or removed field or case, or
a changed stable identity — invalidates both and stales every proposal bound to
them.

The proposal document is `semaprax.agent-proposal.v1`: compact UTF-8 JSON with
exactly one terminal LF and a maximum size of 65,536 bytes. Its ordered fields
are `schema`, `agent_id`, `proposal_schema_digest`, and `value`. A record
`value` is `{"fields":{…}}`; a variant `value` is `{"case":…,"fields":{…}}`.
Field keys are stable identities in declaration order. The decoder re-renders
the admitted document and requires byte equality, so a reordered key, a
duplicate key, an added key, and a missing terminal LF all fail closed.

The compiled product also reports the AgentDefinition digest it resolved the
role from and the module's `graph` revision. The revision is a fact about the
module, not a binding: it deliberately does not enter the schema document,
because an unrelated edit elsewhere in the module must not invalidate a
proposal grammar.

### Authority

A proposal is data. Decoding one produces a `DecodedProposal` of exact scalar
values and nothing else. It constructs no `Authorized<T>`, no publication
token, and no capability, and it performs no provider, tool, filesystem,
process, network, or approval effect. A model may be asked to generate the
grammar or the document; decoder validation stays mandatory either way, and a
proposal that names another agent, another grammar revision, or an unknown
case or field is rejected before any effect.

Generated Python, TypeScript, and Rust consumers of this grammar are not part
of this slice; the decimal-string integer wire is the contract that lets one be
written, not evidence that one exists.

## AgentDefinition v2 and AgentDeployment v1

These two additive documents separate source-owned agent semantics from an
explicit deployment and model binding. They change no AgentDefinition v1,
AgentGraph v1, or Runtime v1 byte: v1 is preserved as an exact projection.

### What each document owns

| Owned by source (`semaprax.agent-definition.v2`) | Owned by deployment (`semaprax.agent-deployment.v1`) |
| --- | --- |
| the six type and six operation identities | concrete `models` rows: provider, model, locality, quality tier, tokenizer, context, price, capabilities |
| `tools`: the complete tool contracts, with their effects and required capabilities | `selection`: `allowed_provider_ids` and `allowed_model_ids` |
| `requirements`: `required_locality`, `minimum_quality_tier`, `required_model_capabilities`, `required_capabilities`, `allowed_tool_ids`, `required_target_features` | `grants`: `granted_capabilities`, `allowed_tool_ids`, `target_features` |
| `ceilings`: the maximum value of each of the 22 Runtime v1 limits | `limits`: the effective value of each of those limits |

Both documents are compact UTF-8 JSON with exactly one terminal LF, closed
objects, canonical key order, a maximum size of 1,310,720 bytes, and a maximum
parsed depth of 16. Identifier lists are strictly increasing, so a duplicate or
an unsorted grant is rejected rather than normalized.

Neither schema has a field that can hold a credential, a secret, a token, or an
environment reference. Because the objects are closed, adding one is rejected
as noncanonical rather than ignored, and nothing in the binding path reads the
environment, the filesystem, or the network. Live authorities and secrets stay
with the host, exactly as Runtime v1 already requires.

### Binding

`bind_agent_deployment` takes the two documents and nothing else — there is no
host parameter, so no provider can be contacted while compatibility is being
decided. It rejects, with `SPX-G556` and the exact failing field:

| Field | Rejected because |
| --- | --- |
| `definition_digest` | the deployment names a different semantic revision |
| `granted_capabilities` | the deployment grants a capability the source does not require |
| `allowed_tool_ids` | the deployment allows a tool the source does not allow |
| `tool_capabilities` | an allowed tool needs a capability the deployment does not grant |
| `target_features` | a required target feature is unavailable in this deployment |
| `limits` | an effective limit exceeds the source ceiling |
| `selection` | a model row is not selected by both allowed lists |
| `required_locality` | a selected model is remote where the source requires local only |
| `minimum_quality_tier` | a selected model is below the source's minimum tier |
| `required_model_capabilities` | a selected model lacks a required model capability |

A deployment may always narrow: fewer turns, a smaller budget, fewer granted
capabilities, fewer allowed tools. It can never add authority the source
contract does not carry, and the host's own grant remains separate and live.

Target features are opaque canonical identifiers compared by exact subset. This
document claims no backend admission or target implementation for them.

### The bound product

`semaprax.agent-bound-deployment.v1` is compact UTF-8 JSON with one terminal LF
and a maximum size of 262,144 bytes. Its ordered fields are `schema`,
`agent_id`, `definition_digest`, `deployment_id`, `deployment_digest`,
`effective`, `v1_definition_digest`, `agent_graph_digest`,
`runtime_v1_profile_digest`, and `nonclaims`. It authenticates both revisions
and publishes the effective selection and limits, but no tokenizer, price, or
tool schema material, and no credential.

Substituting an eligible provider or model changes `deployment_digest` and the
bound digest while `definition_digest` is unchanged. Changing a type or
operation identity, a tool contract, an effect or capability requirement, or a
ceiling changes `definition_digest` and stales every existing deployment and
bound product built on it.

### v1 compatibility and migration

`migrate_agent_definition_v1` admits a v1 document through the unchanged v1
compiler and then splits it. The source contract receives every capability its
own declared tools need and may allow every tool it declares; the deployment
narrows to exactly the v1 policy's grants. Binding the resulting pair
reproduces the original v1 document byte for byte, and therefore its AgentGraph
and Runtime v1 profile known answers. The caller supplies the deployment
identity; the compiler invents none.

Runtime v2 is not wired to descriptive AgentGraph JSON here. The bound product
is an independently checked binding, and execution still runs through the
frozen Runtime v1 projection.

## Agent Lifecycle v1

The schema identity is `semaprax.agent-lifecycle.v1`. It is the additive
compiler product that binds an AgentDefinition's four **deterministic**
operation identities to actual verified functions in one checked module and
executes one acyclic lifecycle over them. It changes no AgentDefinition,
AgentGraph, Proposal Schema, or Runtime v1 byte.

### Stage binding

The compiler resolves each of `initialize`, `observe`, `authorize` and
`reduce` to a `ResolvedFunction` in the same HIR ordinary execution uses, and
validates, in this order and with the exact failing field named:

| Checked | Rejected because |
| --- | --- |
| `<role>.unresolved` | the operation identity names no function, or its identity is not persistent |
| `<role>.effects` | a deterministic stage declares an effect |
| `<role>.arity` | the parameter count is not the one the stage graph requires |
| `<role>.ownership` | a parameter's ownership mode contradicts the graph edge |
| `<role>.type` | a parameter's type is not the bound role type |
| `<role>.result` | the return type is not the bound role type |
| `<role>.retained_call` | the interpreter's own admission rejects the function |
| `task_type.*`, `outcome_type.*` | the role type is not an admitted `{ Bytes, i64 }` record |
| `proposal_type.*` | the Proposal role is not a record of admitted exact scalars |
| `authorize.decision.*` | the decision is not a two-case grant/refusal variant |
| `stage_graph.acyclic`, `stage_graph.order` | the derived stage graph has a cycle, or no unique order |

The admitted signatures are:

```text
initialize(own Task)                                 -> State
observe(borrow State)                                -> Observation
authorize(borrow State, <proposal projection>)       -> Decision
reduce(own State, <proposal projection>, own Outcome)-> Result
```

The ownership modes are exactly the AgentGraph v1 relationships: `initialize`
and `reduce` **consume** the carrier the graph says they consume, `observe`
and `authorize` **borrow** the state. They are read from HIR, not from the
document, so a source ownership change is a compile-time rejection rather than
a backend accident.

### Admitted stage vocabulary

Stage arguments and results are exactly the retained interpreter seam's closed
vocabulary: `bool`, `i32`, `i64`, `u8`, `usize`, owned `Bytes`, and bounded
records and owned-byte variants over those leaves. **`string` is not admitted
in a stage value**, because a `String` leaf would be a new owned cleanup leaf
kind ahead of the shared cleanup machinery and the native and Wasm backends.

That has one visible consequence. Proposal Schema v1 admits only records and
variants, while a by-value nominal stage parameter must be a Copy `class`, so a
Proposal carrier cannot cross the seam as a nominal value. The Proposal
therefore crosses as its **exact ordered scalar projection**, keyed by the
proposal type's own persistent field identities and validated against the
`authorize` and `reduce` parameter lists. A `string` proposal field is
rejected with `proposal_type.field.representation` rather than truncated.

The `Decision` variant `authorize` returns is derived from the validated
signature, not authored: exactly two cases, exactly one of which owns a
`Bytes` seal alongside one `i64` budget — that case is the grant — and the
other carries exactly one `i64` refusal code and owns nothing.

### The opaque one-use authorization

`Authorized` is the value the authorizing transition produces. It is not a
boolean and it is not a hash a caller can hand back:

- its fields are private to `src/agent_lifecycle/authorization.rs`, so no
  struct literal can name them from anywhere else, in or out of the crate;
- it derives nothing — no `Clone`, no `Copy`, no `Default`, no `From` — and it
  has no public constructor;
- the crate's single mint site is a private function in that module, called
  from exactly one place: the function that runs the validated authorize
  stage, which cannot be reached without an `AuthorizeStage` that only the
  stage binder constructs, requires the retained product to name that exact
  validated function, and mints only on the validated grant case of the
  validated decision variant; and
- it is consumed by move at the effect boundary, so one grant admits at most
  one effect.

Therefore `observe`, `reduce`, and model output have no route to one. A
proposal remains data.

Its binding is the domain-separated digest over the lifecycle digest, the
canonical identity-keyed encoding of the state carrier, the exact proposal
document bytes, the grant case identity, and the seal the program itself
constructed. Reproducing that string grants nothing, because no API accepts one
in place of an `Authorized`; and spending an authorization independently
recomputes the binding from the state and proposal actually presented, so a
substituted state or a substituted proposal fails closed with `SPX-G571`
**before** the read operation is called.

### Execution and terminal conditions

One run is one acyclic pass. `propose` is a scripted, offline document
admitted only through the derived Proposal Schema v1 decoder. `execute` is one
explicitly injected `AgentReadOperation` and nothing else: the lifecycle opens
no file, spawns no process, reads no environment variable, and contacts no
network. Cancellation is observed at every stage boundary, and interpreter fuel
is charged per stage.

The Observation `observe` returns is computed and identity-checked, but this
slice builds no model context from it: the proposal document is supplied by the
caller, so semantic context construction remains a nonclaim.

| Status | Reached when |
| --- | --- |
| `completed` | `reduce` published a Result |
| `rejected` | the authorize stage refused, or a deterministic stage did not decide |
| `model_failed` | the scripted proposal is outside the grammar or the projection |
| `effect_failed` | the injected read operation failed or exceeded its byte bound |
| `cancelled` | cancellation was observed at a stage boundary |
| `budget_exhausted` | a stage exhausted its per-stage fuel or its call depth |

The evidence document is `semaprax.agent-lifecycle-evidence.v1`: compact
canonical UTF-8 JSON with one terminal LF carrying the status, the closed
reason, one row per executed stage with its outcome, step count and cleanup-event
count, and the authorization's minted/spent facts. It carries identities and
counts only, never stage payload bytes, and the same inputs replay to the same
bytes and the same digest.

### Public Rust surface

```rust
let lifecycle = semaprax::agent_lifecycle::compile_agent_lifecycle(
    module_source,
    module_path,
    definition_source,
)?;
semaprax::agent_lifecycle::verify_agent_lifecycle_bundle(
    module_source,
    module_path,
    definition_source,
    lifecycle.canonical_json(),
)?;
let run = lifecycle.run(&task, proposal_document, &mut read, budget, &cancellation)?;
```

`CompiledAgentLifecycle`, `LifecycleRun`, `StageRecord`, `Authorized` and
`AuthorizedRequest` expose only immutable canonical documents, identities,
digests and closed statuses. There is no CLI surface, and `AgentDefinition`
gains two additive read-only `type_id`/`operation` accessors and no other
change.

### Executable gate

The `agent_runtime_v1` harness's `agent_lifecycle_v1` module proves:

- deterministic binding of the four deterministic identities to real verified
  functions, with the published ownership modes read from HIR;
- one acyclic pass to `completed`, with every stage settling its owned leaves
  and one call to the injected read operation;
- byte-identical evidence and evidence digest on replay, carrying no payload;
- exact lifecycle-bundle replay and tamper rejection;
- an authorization that differs for a different state, a different proposal and
  a different policy, and that is unchanged by a pure display rename;
- `rejected` on an authorize refusal, with the exact refusal code and zero host
  calls; `model_failed` for a stale grammar digest, a cross-agent proposal, a
  reordered document, an out-of-range integer and a missing terminal LF, each
  with zero host calls; `effect_failed` for a failing read; `cancelled` before
  `initialize`; and `budget_exhausted` at two distinct stages. Cancellation is
  checked before all four of `initialize`, `observe`, `authorize` and
  `execute`; only the pre-`initialize` boundary is deterministically reachable
  from a whole-run caller and therefore only that one is executed, while the
  other three are implemented and unexercised; and
- seven stage-binding rejections — an unresolved identity, two incorrect
  ownership modes, an incompatible result type, a declared effect on a
  deterministic stage, an unadmitted proposal field representation, and a
  decision variant that is not a two-case grant/refusal — each before any run.

The crate-internal `agent_lifecycle::tests` module additionally proves the
single mint site, the absence of `Clone`/`Default`, the acyclic and uniquely
ordered stage graph with a cycle and an ambiguous graph both rejected, the five
separated binding inputs, and the refusal to spend an authorization into a
substituted state or a substituted proposal with the read operation never
reached. The harness's external-consumer probe additionally proves that no
consumer can construct, clone, or default an `Authorized`.

The frozen AgentDefinition, AgentGraph and Runtime v1 profile known answers are
re-asserted unchanged after a lifecycle compiles and runs over the same
definition.

## Agent Checkpoint v1

The schema identity is `semaprax.agent-checkpoint.v1`. It is the additive
durable slice over Agent Lifecycle v1. It changes no AgentDefinition,
AgentGraph, Runtime v1, Proposal Schema v1, deployment, or lifecycle byte, and
it adds no CLI surface: `semaprax agent` still refuses `resume` and
`reconcile`, and continues to, because a verb is only worth exposing once its
durable path and every rejection case execute.

A durable run is the Lifecycle v1 pass split at its single external boundary.
The deterministic prefix — `initialize`, `observe`, the scripted offline
proposal, `authorize` — is re-executable by construction, because the lifecycle
compiler already rejects a declared effect on all four deterministic roles. The
one registered read is not re-executable, so the run commits an **intent**
before crossing the boundary and a **settled observation** after it.

### What a checkpoint is bound to

`bind_durable_agent` takes one checked module, one `BoundAgentDeployment`, and
one caller-supplied policy epoch, and compiles the lifecycle from the bound
product's own Runtime v1 definition projection. Every generation is bound to
nine facts, each of which the live invocation recomputes for itself rather
than reading out of the checkpoint:

| Bound fact | Reported drift |
| --- | --- |
| the caller's policy/cancellation epoch | `policy_epoch_revoked` |
| the source-owned semantic definition digest | `definition_drift` |
| the deployment digest | `deployment_drift` |
| the bound-product digest | `bound_deployment_drift` |
| the State role's stable identity | `state_schema_drift` |
| the derived proposal-grammar digest | `proposal_schema_drift` |
| the compiled lifecycle digest | `lifecycle_drift` |
| the exact module source digest | `source_drift` |
| the caller's task digest | `task_drift` |

A drifted checkpoint re-runs no stage and writes no generation. The state
carrier and the proposal document are additionally bound by digest inside the
journal, so a resume that supplies a different proposal fails with
`proposal_digest_mismatch` before the boundary is approached.

Beyond the binding, a generation carries the program counter (`prefix`,
`intent`, `settled`, `abandoned`, `reduced`, `delivered`), both budget ledgers,
the retention mode, and the hash-chained operation journal.

### A resumed run cannot forge an authorization

Nothing in a checkpoint is an input to minting one. A checkpoint carries no
`Authorized`, no grant seal, and no state carrier — only digests. There is no
decoder from checkpoint bytes back to a `RetainedValue`, so a resumed run
cannot reconstruct the state a grant was made against. It recomputes that state
by re-running `initialize` on a caller-supplied task and re-runs the validated
authorizing transition through the crate's only mint site. Only then is the
freshly derived operation identity compared against the journal's recorded one;
a mismatch drops the grant unspent as `operation_identity_mismatch`.

A forged, truncated, or reordered journal therefore has exactly two possible
effects: the resume refuses, or the resume declines to perform an effect it
would otherwise have performed. Neither direction produces authority.

### Uncertainty, and what is never done automatically

- **Crash before the intent is durable.** The boundary was never approached.
  A resume performs it exactly once.
- **Crash after the intent is durable.** Delivery is *uncertain*, whether or
  not the read actually ran. A resume never retries. Without reconciliation it
  ends in the terminal `unknown` state and leaves the generation reconcilable;
  with `Reconciliation::Settled` it continues from the host's observation
  without crossing the boundary; with `Reconciliation::Abandoned` it commits a
  terminal abandonment.
- **A read that reports failure** is treated as uncertain too. A reported
  failure is not evidence of non-occurrence, so the journal stays at its
  intent.
- **Crash after the settlement is durable.** The resume completes from the
  recorded observation and crosses the boundary zero times.
- **Crash after the reduction, before delivery.** The resume recomputes the
  deterministic reduction, requires its digest to equal the recorded one, and
  delivers.
- **A delivered or abandoned generation** is terminal: the resume re-runs no
  stage at all.

Cancellation is observed at every deterministic stage boundary and once more
immediately before the intent is committed, and deliberately not after it:
abandoning a run between its intent and its settlement would manufacture the
uncertainty the intent exists to bound.

### Budgets are never refunded

Two ledgers carry forward in the checkpoint. The effect-grant ledger is
consumed at the intent, before the boundary, and stays consumed whatever the
operation's fate. The interpreter-fuel ledger covers the whole durable run, so
re-executing the deterministic prefix on a resume spends from the same
remaining total: a resumed run always ends with strictly less fuel than one
that never crashed.

### Retention and redaction

Caller-supplied inputs are never retained. The task objective and the proposal
document reach the checkpoint only as digests, at every generation and in every
mode, and the caller supplies them again at resume. The only external datum a
checkpoint may retain is the settled observation, and only under the explicit
`Retention::ObservationBytes` mode. Under `Retention::ObservationDigestOnly`
the observation is reduced to its digest too, and a resume then requires a host
reconciliation whose bytes reproduce that digest — a mismatch is
`reconciled_observation_digest_mismatch`, an absence is
`redacted_observation_requires_reconciliation`.

### Storage contract and atomicity

Persistence is the caller's. `CheckpointStore` is a trait the compiler
implements nowhere; the durable path opens no file, spawns no process, and
contacts no network. Its declared contract is that a commit either replaces the
whole stored generation or leaves the previous one intact — a filesystem store
satisfies it by writing a sibling temporary and renaming it over the target.

Recovery does not take that contract on trust. Checkpoint bytes are
self-verifying: `AgentCheckpoint::decode` requires the document to reparse
under a closed key set, the journal to decode with strictly advancing entry
ranks, the recomputed chain link to equal the stored one, the stored program
counter to agree with the journal's last entry, and the canonical re-rendering
to equal the supplied bytes exactly. A partially written generation therefore
fails closed with `SPX-G573` rather than being adopted.

### Executable gate

The `agent_runtime_v1` harness's `agent_checkpoint_v1` module proves:

- one complete durable run: five generations, one per journal boundary, one
  boundary crossing, and a final generation whose counter is `delivered` and
  whose effect-grant ledger is exhausted;
- the checkpoint's opacity — no authorization value, no seal, no state
  carrier, no task or proposal payload in any generation;
- crash injection at all five boundaries with the outcomes listed above, each
  followed by the recovery a restarted process would perform, asserting the
  total number of boundary crossings in every case;
- abandonment as a terminal reconciliation, and a delivered generation that
  performs nothing;
- a reported effect failure that stays uncertain, and a store failure at the
  intent that never reaches the boundary;
- no budget refund on resume, and an exhausted effect grant that stays
  exhausted;
- the eight drift rejections and the two caller-input rejections above, each
  re-running no stage and writing no generation;
- torn, renumbered, truncated, transposed, counter-mutated, nonclaim-stripped
  and key-extended documents, each rejected with `SPX-G573`;
- the declared secret-retention policy, with sentinels in both the caller's
  task and the external observation, and the three redacted-resume outcomes;
- an atomic write-and-rename store completing a run, a contract-violating
  store whose torn generation is refused by recovery while its last whole
  generation still decodes and resumes to the uncertain path; and
- the unchanged frozen AgentDefinition, AgentGraph and Runtime v1 profile
  digests after a crashed-and-resumed durable run.

The crate-internal `agent_lifecycle::durable::tests` module additionally proves
what the public surface cannot: that an internally consistent forgery — a
rewritten and correctly rechained journal that decodes exactly like a genuine
checkpoint — still cannot mint an authorization, and that the journal chain is
sensitive to truncation, reordering and single-field substitution. The
crate-wide mint-site gate is extended over all three durable sources.

### Public Rust surface

```rust
let agent = semaprax::agent_lifecycle::bind_durable_agent(
    module_source,
    module_path,
    &bound_deployment,
    policy_epoch,
)?;
let run = agent.start(
    &task, proposal_document, &mut read, budget, retention,
    &cancellation, &mut store, crash,
)?;
let stored = semaprax::agent_lifecycle::AgentCheckpoint::decode(&bytes)?;
let resumed = agent.resume(
    &stored, &task, proposal_document, &mut read, reconciliation,
    &cancellation, &mut store,
)?;
```

`AgentCheckpoint` has no public constructor and no `Clone`: it is produced by a
durable run or reconstructed from bytes that reproduce it exactly.

### Nonclaims

A checkpoint is **not authenticated**. It carries no key material and no
signature, so a party who can rewrite the caller's storage can also recompute
the journal chain; checkpoint integrity is the caller's storage contract, and
each generation republishes that dependence in its own nonclaim list. Provider
billing and external exactly-once execution stay nonclaims unless the external
system guarantees them independently. A settled observation is proof data about
what a host reported, never permission to perform anything.

## Digests

Digests are lowercase `sha256:` values over domain bytes followed by the exact
canonical document bytes:

```text
AgentDefinition:  "semaprax.agent-definition.digest.v1\0"
AgentGraph:       "semaprax.agent-graph.digest.v1\0"
Runtime profile:  "semaprax.agent-runtime.profile-digest.v1\0"
Proposal schema:  "semaprax.agent-proposal-schema.digest.v1\0"
Proposal type:    "semaprax.agent-proposal-type.revision.v1\0"
Definition v2:    "semaprax.agent-definition.digest.v2\0"
Deployment:       "semaprax.agent-deployment.digest.v1\0"
Bound deployment: "semaprax.agent-bound-deployment.digest.v1\0"
Lifecycle:        "semaprax.agent-lifecycle.digest.v1\0"
Lifecycle run:    "semaprax.agent-lifecycle-evidence.digest.v1\0"
Authorization:    "semaprax.agent-lifecycle.authorization.v1\0"
Checkpoint:       "semaprax.agent-checkpoint.digest.v1\0"
Checkpoint run:   "semaprax.agent-checkpoint-evidence.digest.v1\0"
Journal chain:    "semaprax.agent-checkpoint.journal.v1\0"
Checkpoint state: "semaprax.agent-checkpoint.state.v1\0"
Checkpoint prop.: "semaprax.agent-checkpoint.proposal.v1\0"
Checkpoint result:"semaprax.agent-checkpoint.result.v1\0"
Observation:      "semaprax.agent-checkpoint.observation.v1\0"
Module source:    "semaprax.agent-checkpoint.source.v1\0"
Task:             "semaprax.agent-checkpoint.task.v1\0"
```

The proposal-type revision is taken over the exact bytes
`{"proposal_type_id":<id>,"shape":<shape>}`.

The graph binds both the exact definition and exact v1 profile. An admitted
stable-ID rename changes the definition and graph identities. Invalid operation
kinds are rejected rather than normalized into a different graph.

## Public Rust surface

The additive API is:

```rust
let compiled = semaprax::agent_definition::compile_agent_definition(source)?;
let definition = compiled.definition();
let graph = compiled.graph();
let profile = compiled.runtime_v1_profile();
semaprax::agent_definition::verify_agent_graph_bundle(source, profile, graph.canonical_json())?;
let agent = compiled.instantiate(host, cancellation)?;
```

`AgentDefinition`, `AgentGraph`, and `CompiledAgentDefinition` expose only
immutable canonical source, identities, digests, and the compatibility
projection. They expose no constructor that can bypass compiler admission and
no provider, tool, filesystem, process, network, approval, or publication
authority.

The additive proposal-grammar surface is:

```rust
let schema = semaprax::agent_proposal::compile_agent_proposal_schema(
    module_source,
    module_path,
    definition_source,
)?;
semaprax::agent_proposal::verify_agent_proposal_schema_bundle(
    module_source,
    module_path,
    definition_source,
    schema.schema().canonical_json(),
)?;
let decoded = schema.decode(untrusted_model_output)?;
```

`AgentProposalSchema`, `CompiledAgentProposalSchema`, and `DecodedProposal`
expose only immutable canonical documents, identities, digests, and exact
decoded scalars. `AgentDefinition` gains one additive read-only
`proposal_type_id` accessor and no other change.

The additive definition/deployment surface is:

```rust
let (definition_v2, deployment) =
    semaprax::agent_deployment::migrate_agent_definition_v1(v1_source, deployment_id)?;
let bound = semaprax::agent_deployment::bind_agent_deployment(&definition_v2, &deployment)?;
semaprax::agent_deployment::verify_bound_agent_deployment_bundle(
    &definition_v2,
    &deployment,
    bound.canonical_json(),
)?;
let agent = bound.instantiate(host, cancellation)?;
```

`AgentDefinitionV2`, `AgentDeployment`, and `BoundAgentDeployment` expose only
immutable canonical documents, identities, digests, and the exact v1
projection. `bind_agent_deployment` takes no host and no capability.

The additive [Agent Payment Harness v1](AGENT-PAYMENT-HARNESS-V1.md) binds this
exact compilation product to one independently admitted Economic Agent Policy,
constructs Runtime v1 without caller-side profile extraction, and carries a
completed final message into the existing authority-separated payment state
machine. It does not change the v1 graph bytes or imply language-level
transition execution.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-G501` | The definition is not canonical closed AgentDefinition v1 JSON. |
| `SPX-G502` | A semantic identity, role, bound, or derived Profile v1 invariant failed. |
| `SPX-G503` | Supplied AgentGraph bytes do not equal the independently recompiled graph. |
| `SPX-G504` | Supplied Profile v1 bytes do not equal the independently recompiled projection. |
| `SPX-G548` | The Proposal role does not resolve to an admitted closed record or variant. |
| `SPX-G549` | Supplied proposal-schema bytes do not equal the independently rederived grammar. |
| `SPX-G550` | The proposal is not canonical closed `semaprax.agent-proposal.v1` JSON. |
| `SPX-G551` | A proposal identity, case, field, representation, or exact integer bound failed. |
| `SPX-G552` | The document is not canonical closed `semaprax.agent-definition.v2` JSON. |
| `SPX-G553` | An AgentDefinition v2 identity, requirement, or ceiling invariant failed. |
| `SPX-G554` | The document is not canonical closed `semaprax.agent-deployment.v1` JSON. |
| `SPX-G555` | An AgentDeployment identity, list, or limit invariant failed. |
| `SPX-G556` | The deployment is incompatible with its semantic definition. |
| `SPX-G557` | Supplied bound-product bytes do not equal the independently rebound product. |
| `SPX-G570` | A lifecycle stage identity, signature, ownership mode, effect, role type, decision shape, or stage-graph invariant failed. |
| `SPX-G571` | A lifecycle invocation was refused before host work: an authorization was not bound to the state and proposal presented, or the injected read failed its bound. |
| `SPX-G572` | Supplied lifecycle bytes do not equal the independently recompiled lifecycle. |
| `SPX-G573` | Supplied bytes are not one exact canonical checkpoint generation: they do not reparse under the closed key set, their journal does not decode with advancing ranks, their chain link does not recompute, their program counter disagrees with their journal, or their canonical re-rendering differs. |

Module compilation diagnostics reach the caller unchanged: a `.spx` module
that does not verify fails with its own source diagnostics rather than an
agent-layer code.

Profile-specific rejection is intentionally collapsed into the stable
`runtime_v1_profile` invariant at this boundary. Runtime v1 diagnostics remain
unchanged for callers that use Runtime v1 directly.

## Executable gate

The `agent_runtime_v1` integration harness proves:

- deterministic definition and graph compilation;
- exact Runtime v1 profile byte and raw-digest preservation;
- execution of the projection through the unchanged `Agent<H>` kernel;
- stable rejection of noncanonical key order, incorrect stochastic/effect role,
  widened v1 effects, and cross-category identity collisions;
- visible model, context, proposal, capability, effect, limit, terminal, and
  evidence contracts while omitting concrete tokenizer/price content; and
- exact bundle replay plus graph tamper, definition cross-pair, profile tamper,
  and graph-capacity rejection;
- synchronized graph/profile changes for locality and quality requirements,
  capabilities, tool contracts, and limits; and
- stable-ID-only graph evolution with byte-identical Runtime v1 profile output.

Its `agent_proposal_schema_v1` module additionally proves:

- deterministic derivation of a record and a variant proposal grammar from one
  checked module's HIR, with the AgentDefinition's own Proposal identity;
- exact bundle replay and tamper rejection of the derived schema;
- a display rename of the type and its fields preserving both the schema bytes
  and the proposal-type revision, while a representation change, an added
  field, and a changed stable identity invalidate both and stale an existing
  proposal;
- decoder rejection of a noncanonical document, an unknown schema version,
  reordered fields, a record body against a variant grammar and the converse, a
  cross-agent proposal, a stale grammar digest, an extra field, a missing
  field, a wrong field identity, a wrong case, a mismatched representation, an
  oversized string, an oversized document, nine malformed exact integers, and
  four out-of-bound integers;
- exact preservation of `i64::MIN`, `i64::MAX`, `u64::MAX`, and 2^53 + 1
  through the decimal-string wire;
- explicit derivation rejection of an unresolved identity, a generic record, a
  class, `f64`, `Bytes`, `char`, a nested record field, and a field whose
  identity is automatic rather than persistent, plus the resolver's own prior
  rejection of a recursive type; and
- one offline scripted-provider run whose final message is decoded only
  through the derived grammar, whose evidence independently replays, whose
  tampered evidence does not, and whose proposal another agent's grammar
  refuses.

Its `agent_deployment_v1` module additionally proves:

- deterministic migration of the v1 fixture into a v2 definition plus one
  deployment whose binding reproduces the exact v1 document, AgentGraph digest
  and Runtime v1 profile bytes;
- exact bound-product replay and tamper rejection;
- provider/model substitution changing the deployment and bound identities
  while the semantic-definition identity is unchanged;
- five source-semantic changes staling the existing deployment;
- admitted narrowing of turns, granted capabilities and allowed tools, against
  nine rejected widenings and incompatibilities plus an unavailable required
  target feature and a below-minimum quality tier, each decided from the two
  documents alone;
- one semantic definition running two offline scripted deployments to distinct
  evidence, one of which independently replays; and
- rejection of every attempt to add a credential, token, or environment key to
  either closed document, and a source scan proving the binding path performs
  no environment, filesystem, process, or network access.

The fixture known answers are:

- AgentDefinition digest:
  `sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0`;
- AgentGraph digest:
  `sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61`;
- projected Runtime v1 profile raw SHA-256: the frozen Runtime v1 fixture value
  `sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a`.

The existing Agent Runtime v1 suite continues to own all Profile, Task, Action,
Trace, Evidence, routing, budget, cancellation, and injected-host known answers.

## Additive Proposal client bundle v1

`CompiledAgentProposalSchema::generate_clients` derives four pathless,
authority-free artifacts in fixed order: provider-neutral JSON Schema 2020-12,
TypeScript, Python, and Rust. Typed record fields remain in declaration order;
typed variant cases use exact stable IDs as discriminants. Integer encoders
emit bounded canonical decimal strings and the language clients enforce the
4096-byte UTF-8 text limit. The clients construct Proposal data only, never
`Authorized<T>`, a
publication token, or a capability.

The structured-output schema binds the exact Agent ID, Proposal Schema digest,
stable field IDs, and case discriminants using standard JSON Schema keywords.
Its decimal-string patterns do not replace the compiler decoder's full i64/u64
range checks. Its standard `maxLength` keyword is a character-count ceiling,
not the UTF-8 byte check; the language clients and compiler decoder remain the
byte-limit authority.

The canonical `semaprax.agent-proposal-client-bundle.v1` manifest binds the
frozen Proposal Schema v1 identity, schema digest, proposal-type revision, and
each artifact's kind, path, byte length, and domain-separated digest. One
source is limited to 1 MiB, their aggregate to 4 MiB, and the manifest to 64
KiB. `AgentProposalClientBundle::replay` and
`verify_agent_proposal_client_bundle` rederive all four artifacts and require
every byte to match. Malformed or over-bound input is `SPX-G560`; an exact
well-formed substitution is `SPX-G561`.

This additive bundle changes no Proposal Schema v1 byte. Its historical
`no_generated_consumer_clients_in_this_slice` nonclaim remains frozen as part
of that earlier document. Generation and replay perform no filesystem,
network, provider, tool, process, compilation, execution, packaging, or
publication operation.

## Nonclaims and next gates

This slice does not implement or claim:

- execution, packaging, or publication of the additive generated clients;
- proposal values beyond the closed monomorphic scalar record/variant subset;
- `string`, `char`, floating-point, borrowed, or generic stage values;
- a nominal Proposal carrier crossing a stage boundary, rather than its exact
  ordered scalar projection;
- compiled execution of `propose` or of a language-level `execute` body: the
  model is a scripted offline document and the effect is one explicitly
  injected read operation;
- iterative `AgentStep` `continue`/`complete`/`suspend`/`fail` execution, or
  any lifecycle longer than one acyclic pass;
- a Runtime v2 that consumes AgentGraph or the bound product directly;
- target-feature implementation, backend admission, or provider transport;
- typed mutation, testing, build, approval, or publication effects;
- semantic context construction;
- durable checkpointing of anything beyond the single registered read
  operation: no multi-effect journal, no concurrent or distributed run, and no
  compiled `execute` body;
- checkpoint authenticity, integrity, or tamper evidence independent of the
  caller's storage contract, and no signing or key material to provide it;
- external exactly-once execution, provider billing, or automatic retry of an
  uncertain operation;
- a CLI; or
- the signature-change reference vertical slice.

Agent Proposal Schema v1 closes the derived proposal grammar gate, the additive
client bundle closes deterministic source generation and exact replay,
AgentDefinition v2 with AgentDeployment v1 closes definition/deployment
separation, and Agent Lifecycle v1 closes the compiled deterministic stage
binding, the single acyclic execution, and the opaque one-use authorization
value with its executable rejection evidence. Agent Checkpoint v1 closes the
revision-bound durable checkpoint, the crash boundaries of the single external
operation, and the uncertainty reconciliation that replaces automatic retry. Provisioned compilation and
execution of the generated clients remain a separate gate. The Runtime v1
compatibility projection still carries its own authored action/tool schemas.
Durable checkpoint, resume and reconciliation, and Runtime v2's direct
consumption of AgentGraph, remain separate gates on top of this one.
