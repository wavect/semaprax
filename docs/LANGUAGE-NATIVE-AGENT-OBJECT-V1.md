# Language-Native Agent Object and Unified Harness v1

Audience: compiler contributors, Agent Runtime contributors, provider-adapter
authors, and semantic-workspace integrators.

Status: bounded phase-1 compiler slice implemented locally, extended by the
additive Agent Proposal Schema v1 grammar and decoder and by the additive
AgentDefinition v2 / AgentDeployment v1 separation; long-term language,
harness, effects, and durability goals remain proposed and unsupported.

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

## Nonclaims and next gates

This slice does not implement or claim:

- agent language syntax or parser/HIR admission for the agent object itself;
- generated Python, TypeScript, or Rust proposal consumers;
- proposal values beyond the closed monomorphic scalar record/variant subset;
- compiled execution of `initialize`, `observe`, `authorize`, or `reduce`;
- a Runtime v2 that consumes AgentGraph or the bound product directly;
- target-feature implementation, backend admission, or provider transport;
- typed mutation, testing, build, approval, or publication effects;
- semantic context construction;
- `Authorized<T>` minting or consumption;
- checkpoint, resume, exact replay, re-execution, or reconciliation;
- a CLI; or
- the signature-change reference vertical slice.

Agent Proposal Schema v1 closes the previously named next gate for the derived
proposal grammar, and AgentDefinition v2 with AgentDeployment v1 closes the
definition/deployment separation gate. The Runtime v1 compatibility projection
still carries its own authored action/tool schemas, and replacing those is
separate work. The next bounded gates are generated consumers for the proposal
grammar, and compiled execution of the deterministic stages with an opaque
one-use authorization value. Runtime v2 must not consume AgentGraph directly
until those authorization-token semantics have their own reviewed contract and
executable rejection evidence.
