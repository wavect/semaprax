# Language-Native Agent Object and Unified Harness v1

Audience: compiler contributors, Agent Runtime contributors, provider-adapter
authors, and semantic-workspace integrators.

Status: bounded phase-1 compiler slice implemented locally; long-term language,
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
canonical UTF-8 JSON with one terminal LF and a maximum size of 262,144 bytes.
Its ordered fields are:

1. `schema`;
2. `definition_digest`;
3. `agent_id`;
4. `types`;
5. `operations`;
6. `relationships`;
7. `runtime_v1_profile_digest`; and
8. `nonclaims`.

The graph repeats the admitted stable type and operation nodes and derives the
following fixed relationship sequence:

```text
initialize RETURNS State
observe BORROWS State
observe RETURNS Observation
propose BORROWS Observation
propose RETURNS Proposal
authorize BORROWS State
authorize BORROWS Proposal
execute CONSUMES Proposal
execute RETURNS Outcome
reduce CONSUMES State
reduce USES Proposal
reduce USES Outcome
reduce RETURNS Result
```

The current graph does not assert that the proposal is authorized. Opaque,
single-use `Authorized<T>` semantics belong to a later runtime-consuming graph
version and must not be inferred from this relationship list.

## Digests

Digests are lowercase `sha256:` values over domain bytes followed by the exact
canonical document bytes:

```text
AgentDefinition: "semaprax.agent-definition.digest.v1\0"
AgentGraph:      "semaprax.agent-graph.digest.v1\0"
Runtime profile: "semaprax.agent-runtime.profile-digest.v1\0"
```

The graph binds both the exact definition and exact v1 profile. Reordering,
renaming, changing an operation kind, or changing even escaped profile bytes
therefore changes the corresponding identity.

## Public Rust surface

The additive API is:

```rust
let compiled = semaprax::agent_definition::compile_agent_definition(source)?;
let definition = compiled.definition();
let graph = compiled.graph();
let profile = compiled.runtime_v1_profile();
```

`AgentDefinition`, `AgentGraph`, and `CompiledAgentDefinition` expose only
immutable canonical source, identities, digests, and the compatibility
projection. They expose no constructor that can bypass compiler admission and
no provider, tool, filesystem, process, network, approval, or publication
authority.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-G501` | The definition is not canonical closed AgentDefinition v1 JSON. |
| `SPX-G502` | A semantic identity, role, bound, agent/profile join, or embedded Profile v1 invariant failed. |

Profile-specific rejection is intentionally collapsed into the stable
`runtime_v1_profile` invariant at this boundary. Runtime v1 diagnostics remain
unchanged for callers that use Runtime v1 directly.

## Executable gate

The `agent_runtime_v1` integration harness proves:

- deterministic definition and graph compilation;
- exact Runtime v1 profile byte and raw-digest preservation;
- execution of the projection through the unchanged `Agent<H>` kernel;
- stable rejection of noncanonical key order, incorrect stochastic/effect role,
  and mismatched definition/profile identity; and
- omission of concrete tokenizer/profile content from AgentGraph.

The fixture known answers are:

- AgentDefinition digest:
  `sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0`;
- AgentGraph digest:
  `sha256:04f1aa2c674a4b65b78504007e87686c3163aa9ef7cf46b2e845d3448d24024f`;
- projected Runtime v1 profile raw SHA-256: the frozen Runtime v1 fixture value
  `sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a`.

The existing Agent Runtime v1 suite continues to own all Profile, Task, Action,
Trace, Evidence, routing, budget, cancellation, and injected-host known answers.

## Nonclaims and next gates

This slice does not implement or claim:

- agent language syntax or parser/HIR admission;
- generated proposal types or provider response grammars;
- compiled execution of `initialize`, `observe`, `authorize`, or `reduce`;
- typed mutation, testing, build, approval, or publication effects;
- a deployment document or model portability;
- semantic context construction;
- `Authorized<T>` minting or consumption;
- checkpoint, resume, exact replay, re-execution, or reconciliation;
- a CLI; or
- the signature-change reference vertical slice.

The next bounded gate should replace the compatibility projection's manually
authored action/tool schemas with a closed proposal grammar derived from stable
Semaprax record and variant identities. Runtime v2 must not consume AgentGraph
directly until definition/deployment separation and opaque authorization-token
semantics have their own reviewed contracts and executable rejection evidence.
