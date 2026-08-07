# Contributing

SEMAPRAX values coherent semantics over feature count. Before adding syntax, explain which semantic graph operation, verification rule, or systems use case it enables.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run -- check examples/meaning.spx
cargo run -- run examples/meaning.spx
```

Compiler changes should include a success case, a diagnostic case with a stable code, and an end-to-end case when output behavior changes. Keep generated behavior deterministic. Avoid adding ambient authority or build-time network access.

Design changes that affect syntax, the graph schema, transactions, effects, ownership, contracts, or ABI should start as an RFC in `docs/`.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
