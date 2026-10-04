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

## Exploratory M2 route comparison

After `prepare`, run `cargo run --locked --offline --manifest-path
examples/ri13-m2-record-iterator/Cargo.toml --bin measure` with a private
`CARGO_TARGET_DIR`. The binary emits raw CSV for two separate tasks:
`generic_record` decodes and re-encodes the same two owned JSON records;
`stateful_callback` runs the same two scalar transitions from state 10 and
checks final state 13. Each route has one warmup and five timed samples of 32
operations. Route order rotates per iteration.

`direct_rust` invokes Serde or an ordinary checked addition directly.
`handwritten_adapter` wraps those operations in explicit Rust adapter types.
`generated_semaprax` invokes the generated Serde mirror or checked
`SpxStatefulProxy` with environment teardown. All three route labels
describe this fixture, not interchangeable trust boundaries: the generated
callback also enforces its SEMAPRAX contract and lifecycle. The CSV counts
allocations made by Rust's global allocator during the measured body; it does
not count foreign allocator activity. `adapter_buffer_copied_bytes=0` means
the fixture's scalar callback bridge performs no explicit buffer copy; owned
JSON strings still allocate, and hidden library copies are not measured.
This comparison is exploratory, not a support or threshold result.
