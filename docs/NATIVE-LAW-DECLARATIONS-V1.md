# Native Law Declarations v1

Status: bounded LAW-02 source profile. The executable gate is recorded with
LAW-02. This specification extends [Law Set v1](LAW-SET-V1.md) with a
canonical `.spx` projection with an additive scalar relational selector.

## Source form

A native law file has an ordinary module header and one or more declarations.
Every law has a persistent explicit `@id`, typed scalar binders, one pure
proposition, and an evidence requirement. A contract law selects one precise
clause of a persistent function identity. A relational law stands alone.

```semaprax
module arithmetic.laws;

@id("arithmetic.add.monotonic")
law contract "arithmetic.add" ensures (left: i64, right: i64)
    left + right >= left
    evidence smt_proved;

@id("arithmetic.order.total")
law relational (left: i64, right: i64)
    left <= right || right < left
    evidence smt_proved;
```

The grammar is:

```text
law-declaration := "@id" "(" STRING ")"
                   "law" ("contract" STRING ("requires" | "ensures") | "relational")
                   "(" binder ("," binder)* ")"
                   proposition "evidence" evidence ";"
binder          := IDENT ":" scalar-type
scalar-type     := "i64" | "i32" | "u8" | "usize" | "bool" | "char" | "f32" | "f64"
evidence        := "runtime_guarded" | "compiler_proved" | "model_checked"
                 | "smt_proved" | "theorem_proved"
```

The binders are the complete variable scope for `proposition`. A proposition
uses only scalar literals, declared binder references, and the admitted unary
and binary operators. Calls, field projections, records, blocks, branches,
quantification, effects, and aliases are refused. This preserves the closed
scalar selector boundary of Law Set v1.

For a contract law, the subject string is a declaration identity, never a
display name. The proposition must exactly select one existing `requires` or
`ensures` clause of that function. An `ensures` law may bind `result` at the
subject function's exact scalar return type; a `requires` law may not. Other
binders must match typed subject parameters. A relational law has no function subject;
its typed binders and canonical proposition form a distinct LAW-01 selector.
Both preserve source spans for diagnostics. Calls remain unsupported in either
form until a separately reviewed resolved-call profile is available.

## Explicit Project selection

`LAWS.spx` is an optional filename convention only. A Project selects a law
module by listing its path in `[modules] law_sources` of
[`semaprax.manifest.v2`](PACKAGE-MANIFEST-V2.md). Every law source is also an
ordinary `sources` inventory path. The loader reads only selected paths through
the retained Project snapshot; it does not search the working tree for files
named `LAWS.spx`.

A selected law path is parsed as this native-law source form and
is retained as a `LawModule` whose `source_path` is the exact Project-relative
path. A missing selected path is a Project admission error. An unselected
`LAWS.spx` has no law effect. LAW-03 supplies an independently held baseline,
so removing a selected entry cannot reduce protected obligations.

## Complete executable example

[`examples/native-law-project`](../examples/native-law-project/semaprax.toml)
contains the complete manifest, executable app, tests, and explicitly selected
`src/LAWS.spx` shown by this profile. From the repository root, run:

```sh
cargo run --locked -p semaprax -- check examples/native-law-project
cargo run --locked -p semaprax -- query examples/native-law-project --kind law --json
cargo run --locked -p semaprax -- graph examples/native-law-project/semaprax.toml
```

The query and graph expose `native-law.add.right-nonnegative` and
`native-law.order.total` with their exact scalar propositions. Checking the
Project admits both declarations; it does not claim either law has proof
coverage. Use `LawSet::derive` and `derive_report` to inspect their open rows
under an independently selected policy.

## Canonical projection and diagnostics

Canonical formatting emits the module header, one blank line before each law,
the explicit ID, typed binders in source order, normalized scalar proposition,
and exactly one final LF. Comments are trivia and never contribute to the
logical selector or LAW-01 digest.

The Project graph v6 projection carries law modules, law IDs, propositions,
and contract-subject dependencies; Project query v2 exposes the same law IDs
and propositions.

`SPX-LW110` rejects malformed declarations, missing or duplicate explicit
identities, invalid clause or evidence words, non-scalar binder types,
undeclared proposition variables, and unsupported proposition forms.
`SPX-LW111` rejects a law declaration, binder list, or proposition that exceeds
the bounded source profile. LAW-01 continues to own `SPX-LW101` and
`SPX-LW102` for typed inventory normalization and its capacities.

## Proof attachment boundary

This form declares a law. It carries the required evidence class but contains
no proof implementation, theorem file, tactic language, solver command, or
filesystem path. A named theorem or a file named as the law is not evidence of
coverage. Existing SMT, model-checking, and external-kernel producers remain
separate evidence sources for the same LAW-01 obligation. An independent
relational law currently produces an explicit open coverage row: no verified
relational proof attachment is admitted by this bounded profile. A false
relation cannot become covered merely by naming a theorem or choosing
`smt_proved` as its required evidence class. Inline `proof law` declarations,
including a proof named for another law, are rejected by the native parser.

## Non-claims

Native law declarations do not execute functions, introduce dependent types,
authorize a solver or proof kernel, recursively discover source files, alter
ordinary function contract behavior, or turn incomplete proof work into an
accepted Project candidate.
