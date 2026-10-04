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
refute their selected clauses. The gate restores exactly the two broken body fields, requires the repaired
source to equal the saved source bytes, and reproves all three transfer clauses
without altering law intent. A future version can add a Project manifest and
strict publication policy; this pack currently demonstrates direct checked
source and installed proof queries only.

## Finite retry v1

[`finite-retry/`](finite-retry/) contains a canonical, checked Project source
and a finite command-safety model. The positive test derives and replays
bounded evidence from this file; the negative test changes the source to send
a second `charge` command after success and replays the three-step witness.
Its [README](finite-retry/README.md) gives the exact local commands and trust
boundary. The [identity extension](finite-retry/identity/README.md) models one
active request 101 and one mismatched retry 202; an erased identity check fails
source/protocol coverage and its repair restores the original evidence. External
provider behavior remains outside both finite examples.

## Architecture v1

[`architecture/`](architecture/) contains a checked Project and an unchanged
`forbid_reaches` claim over stable declaration IDs. Its broken source adds one
call edge, making the read-only claim report a concrete three-declaration path
instead of `held`; restoring the body repairs the claim. The
[README](architecture/README.md) gives the exact CLI and owning test commands
and the static-graph trust boundary.

## Foreign boundary v1

[`foreign-boundary/`](foreign-boundary/) binds a saved canonical Project and
four reviewed assumptions to a real guarded indexed native SDK. Correct,
bad-return and repaired Rust bodies execute under the unchanged range law;
a fourth body returning zero passes that weak law and demonstrates why a
signature and a range guard do not prove addition. The
[README](foreign-boundary/README.md) gives the exact owning physical command,
and its [report walkthrough](foreign-boundary/REPORT.md) separates conditional
source evidence from observed calls and unproved foreign behavior.

## Collections v1

[`collection/`](collection/README.md) supplies immutable-list insertion sort,
compiler-fixed sortedness and exact multiplicity laws, and separately authored
proofs checked by pinned Lean. The translator emits the checked source's actual
branches and recursive calls. Empty-output and length-preserving duplicate
mutants refuse; separate kernel-checked `[1, 2]` counterexamples demonstrate why
sortedness and length alone are insufficient. Restoring the original body
replays the unchanged laws. Structural totality is mathematical; capacity,
allocation, depth and backend lowering remain separate runtime obligations.
See [the versioned profile](../../docs/COLLECTION-LAW-PACK-V1.md) for the exact
source grammar, report schema, axiom policy and replay boundary.
