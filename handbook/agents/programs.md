# Build an agent as a program

You will learn how a Semaprax `agent` declaration splits one model-driven task
into checked stages, how a proposal differs from permission, and how to pick a
model route. This chapter is for people building an agent. To let a coding
assistant edit `.spx` files, read [Agent workflow](../practices/agents.md).

> **Status.** Source agents, the runtime, and model routing are partial and
> beta. The CLI replays recorded transcripts. A live model needs a Rust host
> that you write and that owns the provider credentials. Routing evidence is
> local and fixture-based: no live provider, hosted, or billing claim.

## What is a turn?

Each turn runs these stages. Your code owns every stage except the model call.

```text
initialize once
     |
observe -> propose (model) -> decode -> authorize -> execute (effect) -> reduce
   ^                                                                       |
   +------------------------------- Continue -----------------------------+
                          or Complete / Suspend / Fail
```

| Stage | Job |
| --- | --- |
| `observe` | Build what the model sees from the current state. |
| `propose` | The model returns a typed proposal. This is data, not permission. |
| `authorize` | Deterministic code decides whether the proposal is allowed now. |
| `execute` | The host runs the one allowed effect. |
| `reduce` | Turn state and outcome into `Continue`, `Complete`, `Suspend`, or `Fail`. |

Authorization runs again on every turn. A grant for one turn never carries over.

## Declare an agent

An `agent` declaration names six types and the stage functions, each with an
`@id`:

```text
@id("example.agent")
agent Example {
    types { type task; type state; type observation;
            type proposal; type outcome; type result; }   // each with an @id
    operations {
        fn initialize;  fn observe;  model fn propose;
        fn authorize;   effect fn execute;  fn reduce;    // each with an @id
    }
    runtime_v1 { canonical_json "..."; }
}
```

Only `propose` is `model fn`. Only `execute` is `effect fn`. Missing or
duplicate IDs report `SPX-P124`. The full rules are in
[Language-native Agent syntax](https://github.com/wavect/semaprax/blob/main/docs/LANGUAGE-NATIVE-AGENT-SYNTAX-V1.md).

Check and inspect the committed examples:

```sh
semaprax check examples/everyday-agent-v2-project
semaprax check examples/routed-agent-project
semaprax agent skill        # the agent contract this compiler carries
```

## Run a recorded transcript

The CLI never calls a model. It replays recorded input:

| Command | Does |
| --- | --- |
| `semaprax agent inspect <definition.json> [--profile]` | Prints the AgentGraph. |
| `semaprax agent run <definition.json> <task.json> <transcript.json> [--evidence\|--trace]` | Runs the scripted turns. |
| `semaprax agent replay <definition.json> <task.json> <transcript.json> <evidence.json>` | Re-checks recorded evidence. |

Do not put an API key in a transcript. A live provider connects through the
host integration, not these commands.

## Give the model only what it needs

The runtime enforces your declared limits on every run:

- It calls only tools the profile allows. Each tool has a closed argument
  schema and a read effect.
- One run is single-threaded and bounded: at most 16 turns, 32 provider
  attempts, 32 tool calls, and five minutes. A profile may lower these, never
  raise them.
- The runtime never retries a tool or an uncertain provider call.
- Cancellation is cooperative, not forced.

See [Budgets, checkpoints, and recovery](recovery.md) for limits and what
happens when one is reached.

## Route to a model

Routing picks one approved model profile per task, and can change it between
turns. A router names a profile ID only. It cannot add a tool, capability, or
limit beyond what the deployment grants.

| You want | Use |
| --- | --- |
| One model per task | `route_new_invocation` over an approved profile set. |
| A different model each turn | `RoutedSession`, which re-routes only at a durable turn boundary. |
| Pick one granted tool or specialist agent | `choice-select/v1`, then an authorize-stage recheck (`SPX-HPJ024`, `SPX-HPJ026`). |
| See why a route was chosen | `route.explain` in harness reports; `semaprax-full harness status --routing`. |

Rules mode makes zero router calls. An operator pin that fails screening is
refused, not replaced. A revoked deployment is refused on resume.

Runnable examples, offline with fixture models:

- `examples/routed-agent-project`: two approved profiles, a pin, and a re-route.
- `examples/support-routing-project`: route a support request to one of two agents.
- `examples/tool-choice-project`: select one of two granted read-only tools.

Run them with `cargo test --locked -p semaprax --test agent_runtime_v1 routed_agent_example`
(or `choice_examples`). The `harness` command is part of the private
`semaprax-full` toolchain, not the public CLI.

## Test it before a model is involved

1. Supply a fixed task and a scripted sequence of proposals.
2. Check the terminal case and its data.
3. Add one denied proposal and confirm `authorize` refuses it.
4. Set a low turn limit and confirm a broken reducer cannot loop.

The interpreter is the default route. Native C11 and Core Wasm stage routes
are explicit host choices with their own requirements.

**Next:** [Budgets, checkpoints, and recovery](recovery.md).

References: [Agent Runtime v1](https://github.com/wavect/semaprax/blob/main/docs/AGENT-RUNTIME-V1.md),
[Runtime model routing v1](https://github.com/wavect/semaprax/blob/main/docs/RUNTIME-MODEL-ROUTING-V1.md),
[Iterative lifecycle v2](https://github.com/wavect/semaprax/blob/main/docs/AGENT-ITERATIVE-LIFECYCLE-V2.md),
[Typed effects v3](https://github.com/wavect/semaprax/blob/main/docs/AGENT-TYPED-EFFECTS-V3.md).
