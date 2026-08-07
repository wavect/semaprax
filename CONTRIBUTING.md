# Contributing

SEMAPRAX values coherent semantics over feature count. Before adding syntax, explain which semantic graph operation, verification rule, or systems use case it enables.

## Development

```sh
scripts/quality.sh
```

On hosts without a POSIX shell, run the baseline commands in [the quality-gate specification](docs/QUALITY-GATES.md). Read [AGENTS.md](AGENTS.md) for repository invariants and the semantic change protocol; it applies equally to human and automated contributors.

Compiler changes should include a success case, a diagnostic case with a stable code, and native/Wasm equivalence when output behavior changes. Keep generated behavior deterministic. Avoid adding ambient authority or build-time network access.

Design changes that affect syntax, the graph schema, transactions, effects, ownership, contracts, or ABI should start as an RFC in `docs/`.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
