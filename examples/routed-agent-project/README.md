# Routed agent project (MR-09/MR-10)

A source-declared agent (`fixture.agent` in `src/app.spx`) with two
host-approved concrete deployment profiles of the same semantic definition:

| Profile | Deployment | Concrete model |
| --- | --- | --- |
| `fast` | `deployments/fast.json` | `fake.local` / `fake-basic` |
| `strong` | `deployments/strong.json` | `other.local` / `other-basic` |

`routing.json` is the host-owned routing configuration: the approved
profiles and their logical metadata, an explicit operator pin and an
experimental decision provider. The router names a profile id only; the
deployment documents carry every grant and limit, and binding rejects any
profile that widens the definition's ceilings, capabilities or tools.

Runnable path (offline contract fixtures, no live inference):

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 routed_agent_example
```

The test approves both profiles with `ApprovedProfileSet::approve`, then runs
three tasks through `start_routed_task` and the real durable policy kernel:
rules mode (zero router calls), the operator pin, and the experimental
decision provider (a deterministic fixture `DecisionInvoker` standing in for
a learned router). It then runs a two-turn `RoutedSession` that re-routes at
the durable turn boundary. Provider adapters are offline fixtures; this
example is not live-provider, availability or billing evidence.

Contract: [Runtime model routing v1](../../docs/RUNTIME-MODEL-ROUTING-V1.md).
