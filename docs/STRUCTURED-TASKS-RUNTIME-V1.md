# Structured Tasks Runtime v1

Status: locally evidenced Rust host runtime; language syntax and backend
lowering remain open.

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

This is the real-execution foundation corresponding to [Deterministic Scoped
Task Model v1](SCOPED-TASKS-V1.md). It does not yet expose SEMAPRAX syntax,
HIR/Graph task nodes, `Sendable`/`Shareable` analysis, dependency scheduling,
schedule replay, async I/O, or native/Wasm lowering.

Focused evidence:

```sh
cargo test --locked --lib structured_tasks::tests::
```
