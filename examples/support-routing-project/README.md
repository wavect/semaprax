# Support routing project (MR-11)

Routes an incoming support request to one of two approved specialist agents
through the runtime finite-choice task `choice-select/v1`, then dispatches the
typed selection as a `RoutedSession` specialist turn through the durable
policy kernel.

| File | Owner | Content |
| --- | --- | --- |
| `src/app.spx` | source | The agent definition `fixture.agent` (same definition as `routed-agent-project`) |
| `deployments/billing.json`, `deployments/technical.json` | host | Two concrete deployment profiles of that one definition |
| `routing.json` | host | The approved profiles, the choice question (`support.route.v1`, typed input/output) and the explicitly attached decision provider |
| `specialists.json` | host | The configured specialist registry (`semaprax.runtime-specialist-registry.v1`): stable ids, approved profile, bounded description, types, effects, capabilities |
| `tickets.json` | fixture | Four incoming requests: billing, technical, unclear, and a prompt-injection attempt |

Flow (all in `tests/agent_runtime_v1/choice_examples.rs`):

1. `ApprovedProfileSet::approve` binds both deployments; `SpecialistRegistry::parse`
   reads the registry and `registry.options(&set)` builds the `agent` candidate
   set (a registry entry naming an unapproved profile is a configuration error).
2. `select_choice` screens the options (kind, types, privacy, budget, effects,
   capabilities) before inference and consults the attached adapter through
   `decision.evaluate` v3. The ticket text reaches the adapter only as the
   quoted, bounded excerpt.
3. The application matches `ChoiceOutcome` exhaustively:
   `Selected` goes through `authorize_specialist_choice` (recheck against the
   live inputs, registry and approved set) and then one specialist turn;
   `Abstained` and `Refused` go to an explicit human queue. There is no default
   action.

Runnable path (deterministic `FixtureChoiceInvoker`, no live inference):

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 choice_examples::support
cargo test --locked -p semaprax --test agent_runtime_v1 routing_e2e_lane
```

Observed: billing and technical tickets dispatch to the `billing` and
`technical` profiles; the unclear and injection tickets abstain (`native`) and
nothing runs. Negative cases: a fabricated specialist id, a command string, a
profile id and an unknown selection id answered by the adapter all abstain as
`rejected_choice`; a since-removed specialist fails the authorize recheck
(`SPX-HPJ026`/`SPX-HPJ024`); an adapter that did not negotiate v3 abstains as
`unsupported_adapter` with zero calls; no adapter abstains as `no_provider`.
`routing_e2e_lane` is the MR-14 end-to-end lane over this project (config,
negotiation, selection, authorized dispatch, report, resume) and also drives
an out-of-tree decision adapter through both runtime consumers.

The fixture is a contract fixture. A real provisioned decision adapter (Jev,
Laya, Mini Jev, Clef) attaches through the same `ConfiguredProvider` over the
host's own transport; none is live-tested on this host.

Contracts: [Runtime model routing v1](../../docs/RUNTIME-MODEL-ROUTING-V1.md),
`choice-select/v1` in [Harness decision v1](../../docs/HARNESS-DECISION-V1.md).
