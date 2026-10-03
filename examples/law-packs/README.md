# Law packs under active admission

These checked source examples keep law statements beside their implementations.
Each pack names the proof profile it uses, a broken body, and the exact gate
that rejects the broken body. Proof evidence applies only to that selected
source and profile; it does not prove target lowering or external effects.

## Money and state v1

[`money-state.spx`](money-state.spx) defines two immutable account balances.
`law07.transfer` requires nonnegative balances and an amount within the debit
balance, then states three independent clauses: exact debit, exact credit, and
conservation. `law15.attempt-transfer` admits amounts up to 1000 and returns
a flat decision record with both balances and a status code. Its three result
clauses require code 0 and the updated balances when funds suffice, or code 1
and the unchanged balances when they do not. The 0–1000 input and amount
bounds keep every intermediate result in checked `i64` range; the proof query includes overflow
side obligations. The source uses no external payment or database operation.

The installed Z3 proof profile is the finite structured-law scalarization of
checked records and closed variants. The trusted base includes source-to-VC
translation and the pinned solver. A successful query proves the selected
source clauses, not native or Wasm execution equivalence. The stable `@id`
values identify the declarations and field paths; changing their shape or any
source body requires fresh proof.

From the repository root, with `z3` explicitly installed:

```sh
cargo run --locked --offline -p semaprax -- check examples/law-packs/money-state.spx
SEMAPRAX_SMT_Z3_PATH="$(command -v z3)" cargo test --locked --offline -p semaprax --lib assurance_manifest::structured_law::tests::real_z3 -- --ignored --nocapture
```

The owning test uses the same source file. Its body-only no-op mutation leaves
the law clauses unchanged: conservation still proves, while both exact amount
clauses refute. Separate debit, credit, overflow and wrong-variant mutations
refute their selected clauses. Restoring the original body repairs the example
without altering law intent. A future version can add a Project manifest and
strict publication policy; this pack currently demonstrates direct checked
source and installed proof queries only.

## Finite retry v1

[`finite-retry/`](finite-retry/) contains a canonical, checked Project source
and a finite command-safety model. The positive test derives and replays
bounded evidence from this file; the negative test changes the source to send
a second `charge` command after success and replays the three-step witness.
Its [README](finite-retry/README.md) gives the exact local commands and trust
boundary. Request identity and external provider behavior remain outside this
version of the pack.

## Architecture v1

[`architecture/`](architecture/) contains a checked Project and an unchanged
`forbid_reaches` claim over stable declaration IDs. Its broken source adds one
call edge, making the read-only claim report a concrete three-declaration path
instead of `held`; restoring the body repairs the claim. The
[README](architecture/README.md) gives the exact CLI and owning test commands
and the static-graph trust boundary.

The collection and Rust-boundary packs are separate LAW-15 work.
A collection pack requires LAW-08's source-authenticated structural induction
and cannot be represented by bounded tests alone.
