# RI-13 M2: one record and iterator callback source

`project/semaprax.toml` owns the authenticated M2 Project; `app.spx` and its checked `tests.spx` module are the single authored source for the generated Serde record
mirror and scalar callback. `prepare` authenticates and checks that Project before it renders both from
the same source revision. `consumer` parses two JSON records through the
generated mirror, maps their values through the generated `Fn` and `FnMut`
adapters under the standard Rust `Iterator`, and checks contract refusal and
teardown. No application-specific Rust trait or per-function adapter is used.

The scalar Project linker retains `app.spx` as the entry module. Its separate
test root therefore does not import that entry module: an imported module is a
provider in the test closure and providers cannot declare `main`. The generated
consumer is the route that exercises the selected record and callbacks.

From the repository root, with a private target under this checkout:

```sh
CARGO_TARGET_DIR=target/ri13-m2 cargo run --locked --offline --manifest-path examples/ri13-m2-record-iterator/Cargo.toml --bin prepare
CARGO_TARGET_DIR=target/ri13-m2 cargo run --locked --offline --manifest-path examples/ri13-m2-record-iterator/Cargo.toml --bin consumer
```

This is a local checked-source projection and Cargo consumer. It is not a
selected Project-published SDK or a `std::Iterator` Rust import declaration.
The generated Serde JSON path allocates owned strings; no zero-copy claim is
made. Ordinary `prepare_native_rust_callbacks` continues to refuse record
source; the opt-in combined route is deliberately limited to exactly one
selected record and the existing scalar snapshot/transition callback.
