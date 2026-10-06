# Tool choice project (MR-11)

Selects one of two granted, read-only tools of a bound deployment through the
runtime finite-choice task `choice-select/v1`, then executes the authorized
tool through the Runtime v1 agent of that deployment.

| File | Owner | Content |
| --- | --- | --- |
| `src/app.spx` | source | The agent definition with two read-only tool contracts, `kb.search` and `kb.status` (`effects: [read]`, `required_capabilities: [tool.read]`) |
| `deployments/tools.json` | host | The deployment granting `tool.read` and both tool ids |
| `choice.json` | host | The question (`kb.lookup.v1`), the allowed effects (`read`) and the argument/result schemas the application produces and consumes |

Flow (all in `tests/agent_runtime_v1/choice_examples.rs`):

1. `bind_agent_deployment` binds the definition and deployment;
   `granted_tool_options(bound.runtime_v1_profile())` offers exactly the tools
   the deployment grants, typed by `tool_schema_type` of their schemas.
2. `select_choice` screens and consults the attached adapter over the same
   contract the support example uses.
3. `Selected` goes through `authorize_tool_choice`: recheck against the live
   inputs and the live runtime profile, and validation of the arguments
   against the tool's closed argument schema. The resulting `AuthorizedTool`
   is the only thing the generation step may act on, and the Runtime v1
   boundary rechecks the tool grant and arguments again before invoking it.
   `Abstained`/`Refused` go to a human queue.

Runnable path (deterministic `FixtureChoiceInvoker`, no live inference):

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 choice_examples::tool
```

Observed: "search articles about a password reset" executes `kb.search`, "is
there an outage on the payment component" executes `kb.status`, "good
morning" abstains and runs nothing. Negative cases: fabricated tool ids and
command strings answered by the adapter abstain as `rejected_choice`; a write
tool and a tool needing an ungranted capability are screened out before
inference; smuggled, mistyped or missing arguments and a since-revoked grant
are refused by the authorize stage (`SPX-HPJ026`); a generation model that
emits a fabricated tool action is refused by Runtime v1 and no tool is
invoked; an adapter without v3 abstains as `unsupported_adapter`.

The agent definition raises `max_builder_bytes` to 4 MiB: Runtime v1 counts
both tool contracts into its builder budget, and the 1 MiB default of the
single-tool fixture is exhausted on the second turn.

Contracts: [Runtime model routing v1](../../docs/RUNTIME-MODEL-ROUTING-V1.md),
`choice-select/v1` in [Harness decision v1](../../docs/HARNESS-DECISION-V1.md),
[Agent Runtime v1](../../docs/AGENT-RUNTIME-V1.md).
