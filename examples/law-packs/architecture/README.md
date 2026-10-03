# No path to a forbidden declaration

This checked Project demonstrates the `forbid_reaches` architecture claim.
The selected rule is `no-a-to-c`: declaration `archclaims.a` must not reach
`archclaims.c` through admitted static call edges. In the correct source,
`a → b` and `b` returns `1`, so the claim reports `status=held` and a null
path. The law is a read-only Project-revision-bound graph claim, not a
source-level theorem about every possible runtime effect.

From the repository root, with the repository Rust toolchain:

```sh
cargo run --locked -p semaprax -- check examples/law-packs/architecture/semaprax.toml
cargo run --locked -p semaprax -- project-assurance-manifest examples/law-packs/architecture/semaprax.toml --max-bytes 1048576 --max-obligations 256 --forbid-reaches no-a-to-c archclaims.a archclaims.c
cargo test --locked -p semaprax --test workspace architecture_claims::forbid_reaches_over_a_real_compiled_revision_goes_stale_loudly_not_silently
```

The owning test reads the correct
[`src/core.spx`](src/core.spx) and the broken
[`mutants/core-calls-c.spx`](mutants/core-calls-c.spx). The broken change makes
`b` call `c`; the same unchanged claim then reports `status=violated` with
minimal path `archclaims.a, archclaims.b, archclaims.c`. Restoring `b` to its
original body repairs the source without changing the claim. The changed
Project revision invalidates the earlier held result.

This pack uses the repository's Architecture Claims v1 `forbid_reaches`
operator and the checked Project v1 source profile. Static native imports and
dynamic call frontiers remain explicit limits of the graph claim. It does not
establish the behavior of a foreign implementation, exclude reflection
outside the admitted profile, or grant any publication or execution authority.
