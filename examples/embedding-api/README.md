# SEMAPRAX embedding API consumer

This standalone Rust program is an external host using only the public
`semaprax::embedding_api` facade. It checks caller supplied source, formats the
canonical projection, renders the semantic graph, preserves a successful-check
warning, rejects malformed input with `SPX-P101`, and refuses incompatible API
major versions.

Run it from this directory with:

```sh
cargo run --locked --offline
```

The dependency is a local path to the repository compiler because this API is
prepublication. The resulting run is checkout consumer evidence; it does not
claim that an installed or published crates.io package has been validated.
The consumer has no filesystem, process, network, CLI, or compiler-internal
access of its own.
