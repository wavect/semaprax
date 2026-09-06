# Agent Proposal to Runtime v1 Compatibility v1

Audience: maintainers, runtime integrators, and compiler contributors.

Status: additive AGENT-04 final-message bridge with locally passed focused
evidence. This is not a Runtime v1 schema or API revision.

## Boundary

Agent Proposal Schema v1 derives a typed closed grammar and decoder from the
verified Proposal role. Agent Runtime v1 accepts its frozen `final` action,
whose message is an ordinary string. This adapter removes only the fixture's
manual wrapping step between those two existing contracts.

`compile_agent_proposal_runtime_v1_compatibility` takes an already compiled
Proposal Schema and the compiled canonical AgentDefinition from which that
schema claims derivation. It requires their exact definition, agent, and
Proposal-role identities to agree, reparses the frozen Runtime v1 profile, and
returns an immutable `AgentProposalRuntimeV1Compatibility`. The product retains
the Proposal decoder and the exact Runtime provider-response bound. It has no
serialized adapter document, schema, digest, or caller-mintable replacement.

`AgentProposalRuntimeV1Compatibility::decode_and_render` first decodes one
untrusted canonical `semaprax.agent-proposal.v1` document through Proposal
Schema v1. Only after that succeeds does it return
`AgentRuntimeV1ActionBytes` containing an exact canonical frozen
`semaprax.agent-runtime-action.v1` final action.

## Exact mapping

Every record or variant already admitted by Proposal Schema v1 is accepted;
the adapter adds no second shape vocabulary. It does not inspect or reinterpret
the decoded case or fields. The exact admitted Proposal document, including
its terminal LF, becomes the value of Runtime v1 `final.message`. Runtime's
canonical JSON string escaping therefore preserves the complete Proposal bytes
inside the final action rather than translating them into action fields.

The result is always `kind: "final"`. A Proposal case or field whose display
name or stable identity contains words such as `tool`, `arguments`, or `final`
remains Proposal data. It cannot select a Runtime tool, name an argument, or
change the action kind. The adapter does not replace Runtime v1's action parser
or tool schemas, and providers still speak the unchanged Runtime v1 action wire
when executing through that runtime.

## Bounds and failure ownership

Rendering measures the complete escaped final action, not only the unescaped
Proposal document, against the exact profile's
`max_provider_response_bytes`. The adapter returns no partial or truncated
action and performs no schema repair, field sorting, fallback, or best-effort
conversion.

`SPX-G578` owns only an incompatible compiled definition/schema/profile pair or
a complete rendered final action that exceeds the Runtime profile bound.
Proposal Schema v1 remains the sole owner of untrusted Proposal failures:

- `SPX-G550` rejects a noncanonical closed Proposal document; and
- `SPX-G551` rejects a Proposal identity, case, field, representation, or
  exact integer bound.

The adapter does not translate those diagnostics into `SPX-G578`. Runtime v1
retains its existing diagnostics for callers that submit Runtime action bytes
directly.

## Compatibility and authority

This is a generated final-message wrapper, not a new execution protocol. It
changes no AgentDefinition v1, AgentGraph v1, Proposal Schema v1, Runtime
Profile v1, Task v1, Action v1, Trace v1, or Evidence v1 byte, schema, digest
domain, known answer, or public constructor. Existing Runtime v1 direct callers
remain unchanged.

A decoded Proposal remains data. Producing a final action does not establish
model quality, semantic correctness, authorization, or successful task
completion. Compilation and rendering invoke no provider or tool and grant no
host, capability, filesystem, process, network, environment, credential,
approval, signing, payment, publication, or persistence authority.

## Focused evidence

The existing `agent_runtime_v1` integration harness owns the bounded gate:

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 agent_proposal_runtime_v1_compatibility::
```

That selector covers:

- deterministic compilation for one admitted Proposal record and one admitted
  Copy-scalar variant;
- byte-exact final rendering in which the complete canonical Proposal,
  including its LF, is preserved as the escaped `message` string;
- Proposal `SPX-G550` and `SPX-G551` preservation for malformed, stale,
  cross-agent, wrong-case, wrong-field, and out-of-range inputs;
- `SPX-G578` for cross-definition/schema/profile pairing and exact
  complete-action bound overflow;
- escape-expansion accounting at the exact profile limit and one byte beyond;
- equality with the frozen Runtime v1 canonical final grammar, followed by one
  offline scripted Runtime v1 run and independent evidence replay;
- hostile Proposal names that resemble Runtime `tool`, `arguments`, and
  `final` members but still produce only a final message;
- no host reachability during compilation or rendering; and
- unchanged frozen AgentDefinition, AgentGraph, Runtime Profile, Task, Action,
  Trace, and Evidence known answers.

Documentation link checks and the module-size gate remain cumulative. This
focused selector is local deterministic evidence only, not hosted,
cross-platform, live-provider, or production support.

## Nonclaims

This slice does not claim:

- a new Runtime schema, API, wire, digest, or Runtime v2;
- direct provider Proposal input or direct Runtime consumption of AgentGraph or
  nominal Proposal values;
- semantic Proposal-case or field translation into Runtime actions;
- generation, replacement, or widening of Runtime tool actions, argument
  schemas, authorization, capability checks, or effect execution;
- Proposal values beyond the existing closed Proposal Schema v1 subset;
- provider transport, credentials, prompt-injection resistance, model quality,
  live execution, retry, billing, persistence, or exactly-once behavior;
- generated-client compilation, packaging, publication, public ABI, Project or
  ProgramRoot retention, CLI, editor, or hosted support; or
- lifecycle authorization, iterative AgentStep execution, or completion of the
  broader Agent-native semantic-program row.
