# Budgets, checkpoints, and recovery

A long-running agent can stop between requesting an operation and receiving its
result. Plan for that interruption before giving it important external work.
The useful question is: what was authorized, what may have happened, and what
can safely happen next?

## Set limits before starting

Choose limits for the task rather than accepting an unbounded loop. Depending
on the selected runtime/model profile, these include turns, deterministic work,
operation calls, input/output bytes, model tokens, deadlines, and quoted cost.

A **reservation** sets aside capacity before work begins. An **observation**
records what is known afterwards. Keep unknown usage distinct from a measured
zero; a lost provider response does not establish that the provider did no work.

For an initial document-reading agent, choose a small turn limit and a small
operation budget. Test what the program returns when each limit is reached.

## Follow the journal boundary

A **journal** records the progress needed for recovery. The durable profiles
record intent before dispatch and retain settled observations afterwards.
This ordering helps a restart determine whether it may continue.

| Recorded state | Recovery question |
| --- | --- |
| Operation not started | Is a fresh authorization and reservation available? |
| Intent recorded, outcome unresolved | Could the external operation already have happened? |
| Outcome recorded | Can the retained result be replayed without repeating the operation? |
| Terminal result recorded | Can the terminal result be returned without new dispatch? |

An unresolved attempt needs the selected uncertainty/reconciliation behavior.
Blindly repeating it is especially risky for writes, messages, or payments.

## Keep checkpoints tied to their source

A checkpoint belongs to a specific execution and source/deployment context.
Recovery checks its bindings, limits, retained observations, and current
requirements before further work. A JSON file containing plausible state is not
a replacement for the selected checkpoint producer and trusted store.

Protect the store at the host boundary. A hash can identify bytes, but it does
not by itself stop someone who controls the storage from replacing or rolling
back the entire history.

## Understand the source-live command

The full toolchain exposes explicit source-live run, resume, migration, and
repair routes. Inspect their accepted arguments first:

```sh
semaprax-full help source-live
```

The configured route identifies the source project, checkpoint directory,
provider executable, and scratch directory. The host adapter owns process and
provider integration; source code owns the checked agent stages.

Use the configuration described by the
[source journal](https://github.com/wavect/semaprax/blob/main/docs/SOURCE-LIVE-JOURNAL-V2.md)
and [source migration](https://github.com/wavect/semaprax/blob/main/docs/SOURCE-LIVE-MIGRATION-V3.md)
contracts. Start with an injected handler or recorded input before involving
real provider credentials.

## Migrate state deliberately

When an Agent's State type changes, an old checkpoint may no longer fit the
new source. A migration explicitly binds the old and new revisions and checks
the transformation between their admitted state shapes.

Keep cumulative accounting across the migration. A new source revision should
not accidentally reset the budget already consumed by the task. Check that
recovery after migration retains the operation history and refuses a mismatched
predecessor.

The runtime API exposes migration and resumed-migration wrappers. Follow the
linked Project or source-live profile for your workflow instead of copying state
fields into a new checkpoint by hand.

## Treat money-moving operations as host operations

The economic-agent code separates payment intent, policy, approval, signing,
broadcast, and reconciliation. A model-generated payment proposal is input to
that workflow. The host supplies wallet/signing/transport implementations and
the authority to use them.

Keep a simulation or injected-handler exercise separate from an actual external
payment. Use reconciliation to resolve an uncertain external outcome before
attempting the operation again.

## Exercise recovery before deploying the host

Test interruption before dispatch, after intent is recorded, after a handler
returns, and after a terminal result is retained. Verify that recovery does not
repeat settled work. Also try a changed source revision, reduced budget,
cancelled task, and mismatched checkpoint.

The [runtime source map](../reference/source-map.md) points to the implementing
modules. The owning specifications provide the precise journal and test rules.

**Next:** [Review the runtime and host implementation entry points](../reference/source-map.md).
References: [Per-operation checkpoints](https://github.com/wavect/semaprax/blob/main/docs/AGENT-OPERATION-CHECKPOINT-V2.md),
[Model budget policy](https://github.com/wavect/semaprax/blob/main/docs/MODEL-BUDGET-POLICY-V1.md),
and [economic-agent implementation](https://github.com/wavect/semaprax/blob/main/src/economic_agent.rs).
