# SEMAPRAX Example Consumer Project

This directory is an example Rust host consumer that uses generated owned-data artifacts.

## Build commands

From the repository root:

```sh
cargo run --locked -p semaprax -- run examples/owned-data-rust/owned_data.spx

cd examples/owned-data-rust
cargo check
cargo run
```

For additional setup and consumer usage details, see the project table and commands in [examples/README.md](../README.md).
