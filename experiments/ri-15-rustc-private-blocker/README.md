# RI-15 `rustc_private` blocker fixture

This is an isolated, source-only reproduction for GitHub issue #373.  It
describes the smallest Rust-to-Semaprax-to-Rust generic callback shape that
would require a compiler-cooperative implementation.  It is not a Semaprax
program, does not invoke Semaprax, and does not claim an executable bridge.

`src/main.rs` deliberately imports both `rustc_driver` and `rustc_interface`.
Those are private rustc crates, so an ordinary stable distribution is not an
admitted implementation environment for the proposed fixed-point exchange.
The non-zero-sized `SemapraxState` makes the requested capability concrete:
a stateful Semaprax value would have to implement a safe Rust callback trait
inside a generic Rust consumer.  No layout, aliasing, borrowing, or cleanup
rule is assumed by this fixture.

The observed local compiler identity is pinned in `toolchain.lock`.  The
reproducer refuses a different compiler commit before it attempts any
compilation and writes its temporary output outside the repository.  It never
downloads a toolchain, invokes Cargo, or changes the default compiler.

Run only in a separately approved experimental environment:

```sh
RUSTC=/absolute/path/to/rustc ./reproduce.sh
```

On the recorded toolchain, the expected result is a non-zero exit that names
the unavailable `rustc_interface` crate.  A successful compile is also a
failure of this fixture: it would establish only that private crates are
present, and still would not prove a fixed-point implementation, ownership
preservation, or a supported product surface.

The decision and closure conditions are in
[`docs/decisions/0007-ri-15-cooperative-rustc-no-go.md`](../../docs/decisions/0007-ri-15-cooperative-rustc-no-go.md).
