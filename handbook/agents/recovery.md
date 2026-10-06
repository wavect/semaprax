# Budgets, checkpoints, and recovery

You will set limits for an agent, and learn what happens when it stops between
asking for an operation and seeing the result. Decide this before an agent does
important external work. The question is always: what was authorized, what may
have happened, and what can safely happen next?

> **Status.** Local and fixture-based evidence. The recovery routes
> (`semaprax-full source-live`) are private tooling, not the public CLI.

## What limits does a run have?

| Limit | Behavior |
| --- | --- |
| Turns, provider attempts, tool calls | Runtime v1 caps: 16, 32, 32. A profile may lower them. |
| Deadline | Five minutes at most. Elapsed time equal to the limit counts as expired. |
| Cost, tokens, calls, context | Checked before each model call by the budget policy. |
| Cancellation | Cooperative. It does not interrupt a call already in flight. |

Since v0.9.0 the runtime checks the live clock and the remaining allowance
before each model call, including routing calls. Each call also gets its
remaining deadline as a positive budget. A new turn that has no time or
allowance left stops before any provider call.

When no model is both eligible and affordable, the run ends `policy_rejected`
with `SPX-G206`. It still returns its Trace, Evidence, and accounting receipt
for work already done. That terminal is replayable: replaying returns the same
result and calls no provider.

## How is cost recorded?

A **reservation** sets money aside before a call. An **observation** records
what the provider reported afterward.

| Case | Recorded as |
| --- | --- |
| Success with an explicit usage report | `observed` |
| Explicit all-zero report | `observed` zero |
| Missing report, failed attempt, or response rejected after dispatch | `unknown` |

A lost response does not prove the provider did no work, so unknown stays
unknown. The accounting receipt reports reserved, observed, and unknown totals.
It cannot bill or settle.

## What does the journal record?

The journal writes intent before dispatch and keeps the result after.

| State on restart | What recovery does |
| --- | --- |
| Not started | Needs fresh authorization and a fresh reservation. |
| `effect_intent` recorded, no result | Marks the effect `uncertain`. It makes no model call and no callback call. |
| Result recorded | Replays the result without repeating the operation. |
| Terminal recorded | Returns it without new work. |

Since v0.9.0 a routed session journals `effect_intent` before the host callback
that may run tool effects. Only model bytes recovered before any durable intent
allow exactly one callback. An uncertain dispatch or effect halts the session
for reconciliation. It is never retried with another model.

Do not repeat an uncertain write, message, or payment. Reconcile it first.

## What is a checkpoint?

A checkpoint belongs to one execution and one source or deployment. Recovery
checks its bindings, limits, and retained results before doing more work. A JSON
file with plausible state does not replace the checkpoint producer and the
trusted store. A hash identifies bytes but does not stop someone who controls
the storage from rolling the whole history back. Protect the store at the host.

## Resume after an interruption

`RoutedSession::resume` restores the journal. Completed turns replay with no
route, model, or effect calls. An in-flight turn reuses its recorded route with
zero router calls. A revoked deployment is refused and never rebound.

For source agents, the private `semaprax-full` tool has `source-live run`,
`resume`, `migrate`, and `repair`:

```sh
semaprax-full help source-live
```

It takes a project config, a checkpoint directory, a provider executable, and
an empty scratch directory. Start with an injected handler or recorded input,
not live credentials. See [Source live CLI](https://github.com/wavect/semaprax/blob/main/docs/SOURCE-LIVE-CLI-V1.md),
[Source journal](https://github.com/wavect/semaprax/blob/main/docs/SOURCE-LIVE-JOURNAL-V2.md),
and [Source migration](https://github.com/wavect/semaprax/blob/main/docs/SOURCE-LIVE-MIGRATION-V3.md).

## Change the agent's state type

When the State type changes, an old checkpoint may not fit. A migration binds
the old and new revisions and checks the transformation. Spending carries
across it: a new revision does not reset the budget already used. A mismatched
predecessor is refused.

## Delegate to a child agent

A child session runs under a grant: an allowance reserved from the parent,
a depth limit, one profile, and the parent's deadline. A child never widens the
deadline or overspends. Settling a child never refunds. Delegation is bounded
and runs on the same host.

## Money-moving operations

A model's payment proposal is input to a workflow of intent, policy, approval,
signing, broadcast, and reconciliation. The host supplies the wallet and the
authority. Keep simulations apart from real payments. See
[Economic agent](https://github.com/wavect/semaprax/blob/main/docs/ECONOMIC-AGENT-V1.md).

## Test recovery

Interrupt a run at each point and check that settled work is never repeated:

1. Before dispatch.
2. After intent is recorded.
3. After the handler returns.
4. After the terminal result.

Then try a changed source revision, a lowered budget, a cancelled task, and a
mismatched checkpoint.

**Next:** [Source map](../reference/source-map.md).

References: [Agent Runtime v1](https://github.com/wavect/semaprax/blob/main/docs/AGENT-RUNTIME-V1.md),
[Runtime model routing v1](https://github.com/wavect/semaprax/blob/main/docs/RUNTIME-MODEL-ROUTING-V1.md),
[Per-operation checkpoints](https://github.com/wavect/semaprax/blob/main/docs/AGENT-OPERATION-CHECKPOINT-V2.md),
[Model budget policy](https://github.com/wavect/semaprax/blob/main/docs/MODEL-BUDGET-POLICY-V1.md).
