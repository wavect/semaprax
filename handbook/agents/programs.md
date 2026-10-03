# Build an agent as a program

An agent needs more than a prompt. It needs a task, state, allowed operations,
and a rule for deciding what happens after each result. Semaprax gives those
parts explicit types and checked stages.

Start here after [Types](../language/types.md) and
[Contracts and effects](../language/contracts-effects.md). A coding assistant
editing `.spx` files is a separate workflow, covered in
[Driving Semaprax from an AI agent](../practices/agents.md).

## Design the task before the model call

Consider an agent that answers a question using one approved document. Write
down the inputs and decisions before choosing a provider:

| Role | Example responsibility |
| --- | --- |
| Task | The question and the permitted document identity. |
| State | What has already been read and how many turns remain. |
| Observation | The information presented for the next decision. |
| Proposal | A typed request, such as reading the permitted document or finishing. |
| Authorization | A deterministic decision about whether that proposal is allowed now. |
| Outcome | The result of the authorized operation. |
| Result | The final answer or the program's terminal result. |

These are design examples, not a ready-made wire schema. The compiler derives
the actual shapes from the selected checked declarations.

## Understand one turn

```text
initialize once
      ↓
observe → obtain proposal → decode → authorize → execute → reduce
   ↑                                                        │
   └────────────────────── Continue ─────────────────────────┘
                              or Complete / Suspend / Fail
```

The **reducer** turns the current state and operation outcome into the next
step. It is ordinary checked program logic, rather than another instruction
that the model is trusted to follow.

The iterative Step variant has `Continue`, `Complete`, `Suspend`, and `Fail`
cases. Their fields match the selected State and Result roles. Use the exact
role and case shapes from the lifecycle specification when assembling a source
Agent definition.

## Separate a proposal from permission

A decoded proposal tells you what was requested. Authorization decides whether
it is allowed for this state and this turn. The runtime checks authorization
again on every turn, then consumes the resulting grant at the operation boundary.

For the document example, checking that a proposal is well-formed does not
establish that it names the approved document. That second check belongs in the
authorization stage and host policy.

The host supplies the actual operation handler. A test can inject a fixed
response. A configured application host can read the permitted resource.
The same source roles make both cases easier to reason about.

## Choose the runtime route

There are two useful entry points to distinguish:

| Route | What you provide |
| --- | --- |
| CLI `agent inspect`, `agent run`, and `agent replay` | The definition and the versioned task, transcript, or evidence inputs required by the command. |
| Direct Runtime v2 and source-model integration | A retained Project, selected Agent/deployment, explicit invocation, host handlers, and the chosen model integration. |

The CLI's transcript route consumes recorded/scripted inputs. A live provider
is configured through the runtime/host integration. Do not put an API key into
a transcript and expect the transcript command to become a provider client.

Read the installed entry points without starting a provider:

```sh
semaprax help agent
semaprax agent skill
```

The skill bundle describes the Agent contract carried by that compiler. The
[source runtime API](https://github.com/wavect/semaprax/blob/main/src/agent_runtime_v2.rs)
exposes bindings for direct, live, linked, checkpoint, and migration workflows.

## Build a deterministic first test

Use the lifecycle's fixture-based tests as the starting pattern. Supply a fixed
task, a known sequence of proposals, and a handler with a predictable response.
Check the terminal case and returned data, then test one denied proposal.

Also test that authorization runs again after state changes. A grant for one
turn must not become permission for every later turn. Keep cancellation and
iteration limits in the test so a broken reducer cannot loop indefinitely.

The owning [iterative lifecycle specification](https://github.com/wavect/semaprax/blob/main/docs/AGENT-ITERATIVE-LIFECYCLE-V2.md)
links its focused Rust test selectors and exact carrier rules. Begin with the
default interpreter route. Native and Core Wasm stage selectors are explicit
host/runtime choices with their own capability and retained-source requirements.

## Add persistence after the basic loop works

A `Suspend` value describes a stopped lifecycle. Durable continuation needs the
checkpoint workflow and trusted store as well. Add those pieces after you can
explain each stage and reproduce the deterministic test.

**Next:** [Add budgets, recovery, and state migration](recovery.md).
References: [Typed effects v3](https://github.com/wavect/semaprax/blob/main/docs/AGENT-TYPED-EFFECTS-V3.md),
[Direct Runtime v2](https://github.com/wavect/semaprax/blob/main/docs/AGENT-RUNTIME-V2.md),
and [Source Model Operation](https://github.com/wavect/semaprax/blob/main/docs/SOURCE-MODEL-OPERATION-V1.md).
