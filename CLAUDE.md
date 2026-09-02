# SEMAPRAX

Claude Code loads this file automatically. The repository's operating
invariants, read order, and change protocol live in [AGENTS.md](AGENTS.md) and
are imported below rather than restated, so the two cannot drift apart.

@AGENTS.md

## Before you finish

Run the gate profile for the change, not a bare `cargo test`:

```sh
scripts/quality.sh full      # or: quick | changed
```

[Quality gates](docs/QUALITY-GATES.md) explains profile selection. Clippy runs
with `-D warnings`, so an unused import fails the build.

## Known conditions

`cargo test --locked -p semaprax --lib` aborts in
`wasm::internal_strings::tests::nesting::nested_if_compile_on_default_stack`
with a stack overflow on a default-stack debug build. It reproduces on an
unmodified tree and is not caused by your change; skip it with
`-- --skip nested_if_compile_on_default_stack`.

A full `--workspace --all-targets` test build links several hundred integration
binaries and needs well over 10 GB. Build with `CARGO_INCREMENTAL=0` and
`CARGO_PROFILE_TEST_DEBUG=0` when disk is limited.
