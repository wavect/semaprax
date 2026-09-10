# Agent typed effects v3

Status: **HOSTED GREEN** for the bounded v0.4.0 implementation.

Audience: compiler contributors and runtime integrators.

The [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md) supersedes the former
local-only evidence status without changing the registry or host boundary.

`agent_lifecycle::iterative::effects::compile_typed_effects` accepts one checked
module, an immutable BoundAgentDeployment, one Step type identity, one Proposal
selector field identity and an ordered operation registry. It compiles the exact
bound deployment's Definition v1 projection through iterative lifecycle v2.
The original one-read v2 API and its document remain unchanged.

The registry contains at most 64 operations. Every operation must name an exact
source-owned tool ID present in the deployment's allowed subset. Its effect ID
must be the tool's sole declared effect. Argument and result keys must match
every required deployed schema field exactly, in order. These are source-owned
schema identities, not nominal HIR type identities. Each argument additionally
binds an exact checked Proposal field identity and scalar representation.
The selector must be a checked usize Proposal field; its zero-based value
selects a registry row on the current authorized turn.

The admitted host scalars are bool, i32, i64, u8 and usize. Integer schema values
must also fit the deployed signed integer representation. There are at most
eight argument fields and eight nonempty result fields. The host receives an
immutable request with the exact operation/effect IDs, projected scalar values
and the current consumed authorization request. Only an explicitly injected
TypedEffectHandler dispatches the operation; no provider, process, filesystem
or network implementation is created by compilation or execution.

The registry document commits the complete deployment digest, selector,
ordered field mappings, scalar representations and exact lifecycle document.
The resulting digest scopes every fresh per-turn authorization. Substituting
an operation, projection, result schema or deployment changes that scope.

The existing reducer still consumes Outcome{Bytes,i64}. A successful typed host
result is encoded as the exact `semaprax.agent-effect-fields.v1` ordered fields
document in Outcome Bytes; status remains zero. This is an explicit typed host
boundary and byte transport, not arbitrary typed nominal reducer signatures.
Bytes, nested records and variants are not admitted host result types.

Caller call, argument-byte, result-byte and aggregate-byte ceilings intersect
the exact deployment limits. Iterations intersect deployed max_turns and the
v2 hard ceiling. Every field also obeys its deployed max_bytes. Byte work counts
canonical typed field encoding, including schema and field identities; this
conservatively includes framing beyond raw scalar bytes. A dispatched call
stays charged on host failure, malformed result or oversized result. Host None
contains no result bytes to charge.

Result size is measured before field/type validation or JSON allocation.
Measurement computes the exact canonical encoded size up to the hard 65536-byte
transport bound. Larger values charge the explicit overflow sentinel 65537 and
fail closed. Depth above 32 or work above 4096 value nodes also charges that
sentinel and fails closed. Thus rejected arbitrary host carriers do not trigger
unbounded encoding allocations. Result counters retain this bounded attempted
work even when the result is rejected. No failed result reaches reduce and no
later effect executes after that failure.

TypedEffectRun exposes immutable counters, failure reason, underlying iterative
evidence and an additive v3 evidence digest. The evidence commits registry,
underlying invocation/stage evidence, effective effect ceilings and attempted
call/byte counters. Execution remains retained-interpreter and injected-handler
scoped. [Direct Runtime v2](AGENT-RUNTIME-V2.md),
[per-operation checkpoints](AGENT-OPERATION-CHECKPOINT-V2.md), and
[durable migration](AGENT-STATE-MIGRATION-V3.md) are implemented joined
extensions with hosted-green release evidence, not missing prerequisites for
this bounded registry. Provider transport and a public general Agent ABI remain
separate functionality.

Focused gate: `cargo test --locked -p semaprax --all-features --lib
agent_lifecycle::iterative::effects::tests` (the original five-case corpus).
Cases alternate two operations over three turns, retain fresh authorization
bindings, reject typed result drift and effect/projection substitution, enforce
call and deployment turn ceilings, and charge oversized failed host work before
encoding.
