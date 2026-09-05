# Structured Tasks Runtime v1

Audience: maintainers, host integrators, and compiler contributors.

Status: locally evidenced Rust host runtime with invocation-owned HTTPS work;
language syntax and backend lowering remain open.

`semaprax::structured_tasks::task_scope` executes real user closures inside a
lexical scoped-thread lifetime. A scope admits at most 64 tasks, rejects empty,
NUL-bearing, and duplicate stable identities, starts workers in canonical
identity order, shares one cooperative cancellation token, and joins every
worker before returning. Closures may borrow non-`'static` values because the
scoped-thread type system prevents escape.

The first failed result in canonical report order is sticky. Semantic,
nonzero-physical, cancellation, and panic outcomes trigger cancellation;
siblings may observe it through `is_cancelled` or `cancellation_point`, but all
started work drains. Reports are canonical regardless of completion order.

`TaskScope::spawn_https_get` moves one explicit `NetworkProvider` into a
lexical task and returns its bytes through an `HttpsTaskOutput` slot. Callers
inspect that slot only after `task_scope` returns. The provider settles exactly
once before publication on success and HTTP failure, and also settles on
pre-start cancellation, provider panic, invalid deadline, or rejected task
registration. HTTP failures remain typed in the output slot and project their
exact nonzero `semaprax.http.v1` code into the task report.

HTTPS deadlines are caller-selected in `1ns..=30s`. Cancellation is checked
before entering transport. Once blocking TLS/HTTP work starts it drains, in
accord with the scoped-task model; if it completes after the deadline, its
response is discarded and the task reports `DeadlineExceeded`. This is a
bounded blocking host integration, not preemptive cancellation or an async
executor.

This is the real-execution foundation corresponding to [Deterministic Scoped
Task Model v1](SCOPED-TASKS-V1.md). It does not yet expose SEMAPRAX syntax,
HIR/Graph task nodes, `Sendable`/`Shareable` analysis, dependency scheduling,
schedule replay, async I/O, or native/Wasm task lowering.

Focused evidence:

```sh
cargo test --locked --lib structured_tasks::tests::
```
