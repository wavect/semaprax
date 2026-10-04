# Finite retry command safety

This project is the checked source for the LAW-10 protocol example. The
`payment.protocol` session has a finite `Idle → Pending → Succeeded` path and
`Retry` and `Failed` alternatives. `payment.dispatch` is the pure transition
function; `payment.step` is the source caller whose route to it is checked.

From the repository root:

```sh
cargo run --locked -p semaprax -- check examples/law-packs/finite-retry/semaprax.toml
cargo test --locked -p semaprax --test project source_protocol_law::legal_retry_model_is_source_bound_and_finitely_checked
cargo test --locked -p semaprax --test project source_protocol_law::repeated_charge_has_minimal_source_replayed_counterexample
```

The first command checks the project source. The positive test explores the
finite protocol with a 64-state, depth-32, 128-transition bound, then replays
the report against the authenticated source revision. The negative test reads
[`mutants/repeated-charge.spx`](mutants/repeated-charge.spx), which adds a
second `charge` command after `Succeeded`; the model checker reports the
three-transition `charge, success, charge` counterexample and replays it from
the changed source. The law is not weakened to accept that mutation. The
project's source is the input to the positive test.

The law review selects `payment.protocol`, `payment.dispatch`, and
`payment.step` by stable identity, with `Succeeded` as the success state and
`charge` as the forbidden post-success command. The source protocol and law
evidence use their repository `checked-v1` profile; the session's source name
is `payment-command-v1`. The mutation's minimal failure is
`status=violated`, `trace_replay=concrete_source_replay`, with the third step
`Succeeded --charge--> Pending`. Removing that extra transition repairs the
source while leaving the selected rule and bounds unchanged. Changing the
source or bound makes old evidence fail replay and requires a fresh check.

The checked claim is pure command safety within the stated finite profile.
No provider call is made, so this example gives no external exactly-once or
payment-settlement guarantee. The separate [request identity v1 Project](identity/README.md) models one
active request 101 and a mismatched request 202. Its body-only identity-confusion
mutation fails exact source/protocol coverage, and restoring the equality
repairs the unchanged law. This finite case does not generalize to arbitrary
request keys or concurrent sessions. The checker executable and project
test require the repository's Rust toolchain; no hosted service is needed.
