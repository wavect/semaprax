# Foreign-boundary law pack v1

This local example reviews a Rust boundary before publishing its returned value
as a Semaprax value. The saved Project forwards `interop.add` to the exact
selected Rust item `fixture_math::add`. The law requires a returned `i64` in
`[0, 50]`; it does **not** state that the function implements addition.

## Review the law and assumptions

Read [`assumptions.json`](assumptions.json), then
[`src/app.spx`](src/app.spx). Version 1 uses `checked-v1` semantics and declares
four assumptions about the foreign implementation: no effects, no callbacks,
no panics, and no shared state. These are explicit conditions accepted by this
example's host policy. Neither a Rust signature nor index admission proves them.
The Rust fixtures use saturating arithmetic so ordinary signed overflow does
not contradict the no-panic declaration.

The adapter checks the inclusive return range after the Rust call and before
successful semantic publication. It cannot undo an effect, callback, panic, or
shared-state access inside the foreign call. The trusted base includes index
admission, source-to-caller analysis, guard generation, Rust/C compilers and
linker, and the executing process. No foreign-body theorem is supplied.

## Run the example

From the repository root on macOS, with the repository's locked dependencies
already provisioned and absolute stable Rustc/Clang paths available:

```sh
RUSTC="$(command -v rustc)" CLANG="$(command -v clang)" \
SEMAPRAX_ARCHIVER=/usr/bin/libtool \
CARGO_TARGET_DIR=target/law15-foreign CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test --offline --locked -p semaprax-native-rust-interop --lib \
  public_sdk::indexed_tests::indexed_project::guarded_indexed_project_sdk_checks_physical_return_before_semantic_publication \
  -- --exact --nocapture --test-threads=1
```

This owning harness reads these saved source, manifest, assumption and Rust
files. It constructs an exact prepared API index, authenticates the Project,
builds its guarded native SDK through the library API, and compiles and runs a
Rust consumer of that SDK. It requires no solver, network, or hosted service.
This directory is an executable example for that API route; `assumptions.json`
is fixture input, not a newly introduced compiler CLI configuration format.
Other platforms require their configured tools from the native interop guide.

The harness asserts canonical source round-trip, exact report expectations,
selected policy replay, changed-law and stale-artifact refusal, and actual
native outcomes. It deletes its temporary packages after success.

## Break and repair without changing the law

All four consumers call the same checked Semaprax export with `(20, 22)`.

| Rust implementation | Observed native outcome | Meaning |
| --- | --- | --- |
| [`correct.rs`](rust/correct.rs) | `Ok(42)` | This observed return satisfies the range guard. |
| [`bad-return.rs`](rust/bad-return.rs) | `host.math.v1`, `Import`, `40909`, nonretryable | The physical Rust return is 1042; the adapter refuses to publish it as success. |
| [`repaired.rs`](rust/repaired.rs) | `Ok(42)` | Fresh code and artifact evidence pass the unchanged range law. |
| [`weak-law.rs`](rust/weak-law.rs) | `Ok(0)` | An incorrect addition body satisfies this weak range law. |

The last control is intentional: improve a law when it omits required behavior.
Do not interpret this range certificate as an addition theorem. The original
consumer also refuses `(1000, 22)` with the same status. The law, assumptions,
and Semaprax source stay unchanged across the broken and repaired Rust bodies.

See [`REPORT.md`](REPORT.md) for the report boundary and drift controls.
