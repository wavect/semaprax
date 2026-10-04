# Finite request identity v1

This LAW-15 example adds an explicit identity check to the existing LAW-10
pure dispatcher profile. Read [`src/machine.spx`](src/machine.spx) first: the
`payment-request-identity-v1` protocol is the expected transition law. The
checked dispatcher must agree with it on every finite state/event pair.

## Initial law review

One session is bound to active request ID **101**. Its retry event carries 101;
the mismatched retry event carries **202**. `payment.request_id` defines this
closed event-to-identity mapping in checked source. This example has one active
request and two modeled identities, not an arbitrary queue or an unbounded key
space. Starting another independent session for a fresh request is outside its
claim. There is no network parser or payment provider in this Project.

The six states are `Idle`, `Pending`, `Succeeded`, `Retry`, `Failed`, and
`Rejected`, encoded in that order as 0–5. Events are encoded in protocol order:

| Event code | Event | Allowed transition |
| --- | --- | --- |
| 0 | `charge` for 101 | `Idle → Pending`, charge bit 1 |
| 1 | `success` for 101 | `Pending → Succeeded` |
| 2 | `failure` for 101 | `Pending → Retry` |
| 3 | `retry_a`, identity 101 | `Retry → Idle` |
| 4 | `retry_b`, identity 202 | `Retry → Rejected` |
| 5 | `abort` | `Retry → Failed` |
| 6 | `cancel` | `Idle → Failed` |
| 7 | `timeout` | `Pending → Failed` |

Every other state/event pair in this closed domain is disabled. `Succeeded`,
`Failed`, and `Rejected` are terminal. The existing selected rule also requires
no charge command after success. Its public `payment.step` caller forwards its
inputs unchanged to the dispatcher. Exact source coverage is part of admission:
labels alone cannot claim that an identity check was executed.

## Run the supported route

From the repository root with the repository Rust toolchain and locked Cargo
dependencies already provisioned:

```sh
cargo run --locked --offline -p semaprax -- check examples/law-packs/finite-retry/identity/semaprax.toml
CARGO_TARGET_DIR=target/law15-identity CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test --locked --offline -p semaprax --test project \
  source_protocol_law::request_identity:: -- --nocapture --test-threads=1
```

The owning selector passed 2/2 (zero failed or ignored) with this example.
The tests use these exact saved canonical sources and manifest through
the authenticated Project library route. They execute checked HIR for all 48
state/event pairs, then require closed model exploration within 64 states,
depth 32 and 128 transitions. No fairness assumption, solver, editor or hosted
service is required. `ModelChecked` is finite model evidence about this source
and these bounds, not an induction theorem or a native/Wasm lowering proof.

## Broken change, minimal failure, unchanged-law repair

[`mutants/identity-confusion.spx`](mutants/identity-confusion.spx) changes only
`request_id(event) == 101` to `request_id(event) >= 0`. The protocol, caller,
bounds and success/charge selections remain unchanged. Both modeled identities
now pass the body check.

The concrete source witness is `state=3` (`Retry`), `event=4` (`retry_b`, 202).
The broken checked body returns **0** (`Idle`, no charge) instead of **10**
(`Rejected`, no charge). Admission returns **`SPX-LP406`**, because source
execution differs from the expected protocol transition. This is a concrete
source/coverage mismatch, not a fabricated post-success counterexample trace.
The original report cannot replay against that changed body.

The owning test repairs the body by restoring the original equality, reruns
the finite checker, and replays the original evidence digest against the
restored source. It observes 10 for the mismatched identity and `ModelChecked`
again. The existing [repeated-charge example](../README.md) separately exercises
a concrete `charge, success, charge` safety counterexample.

## Versions, assumptions and limits

This pack uses the repository's `checked-v1` semantics and
`semaprax.source-protocol-safety.v1` report schema. Its source protocol version
is `payment-request-identity-v1`. Changing only that version to v2 preserves
the transition behavior but invalidates prior evidence with `SPX-LP407`.
Changing source, selected identities, transition coverage, bounds or Project
inputs requires a fresh check. The same source digest binds the identity map.

Trusted components are Project source authentication, checked-HIR interpreter,
finite table construction, breadth-first explorer and report replay. The report
states `fairness: none`, `authority: none` and
`finite_pure_dispatcher_safety_only_no_external_exactly_once`. A repeated command
before success remains an allowed retry; whether a provider deduplicates it,
charges money, settles, or responds is wholly outside this model. The pack
provides no external exactly-once, liveness, concurrency or payment authority.
