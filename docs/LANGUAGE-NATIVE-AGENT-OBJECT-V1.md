# Language-Native Agent Object and Unified Harness v1

Audience: compiler contributors, Agent Runtime contributors, provider-adapter
authors, and semantic-workspace integrators.

Status: bounded phase-1 compiler slice implemented locally, extended by the
additive Agent Proposal Schema v1 grammar and decoder; long-term language,
harness, effects, durability, and deployment goals remain proposed and
unsupported.

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

No `.spx` agent syntax is admitted in this slice. The canonical definition is
an intermediate contract for reviewing identity, graph, and authority design
before syntax is frozen. Runtime v1 material is structured definition data; the
compiler, rather than the author, supplies its frozen schema and nonclaims.

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

The compatibility projection deliberately keeps deployment and v1 semantic
material together. A later additive `AgentDeployment` contract must split
concrete provider/model binding from the source-owned definition before
AgentGraph is consumed directly by Runtime v2.

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

## Digests

Digests are lowercase `sha256:` values over domain bytes followed by the exact
canonical document bytes:

```text
AgentDefinition:  "semaprax.agent-definition.digest.v1\0"
AgentGraph:       "semaprax.agent-graph.digest.v1\0"
Runtime profile:  "semaprax.agent-runtime.profile-digest.v1\0"
Proposal schema:  "semaprax.agent-proposal-schema.digest.v1\0"
Proposal type:    "semaprax.agent-proposal-type.revision.v1\0"
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
- typed mutation, testing, build, approval, or publication effects;
- a deployment document or model portability;
- semantic context construction;
- `Authorized<T>` minting or consumption;
- checkpoint, resume, exact replay, re-execution, or reconciliation;
- a CLI; or
- the signature-change reference vertical slice.

Agent Proposal Schema v1 closes the previously named next gate for the derived
proposal grammar; the Runtime v1 compatibility projection still carries its own
authored action/tool schemas, and replacing those is separate work. The next
bounded gates are generated consumers for the grammar, and definition/deployment
separation. Runtime v2 must not consume AgentGraph directly until that
separation and opaque authorization-token semantics have their own reviewed
contracts and executable rejection evidence.
