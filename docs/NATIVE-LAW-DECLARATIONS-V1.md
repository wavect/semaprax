# Native Law Declarations v1

Status: bounded LAW-02 source profile. The executable gate is recorded with
LAW-02. This specification extends [Law Set v1](LAW-SET-V1.md) with a
canonical `.spx` projection; it does not widen LAW-01 selectors or evidence.

## Source form

A native law file has an ordinary module header and one or more declarations.
Every law has a persistent explicit `@id`, a persistent function subject ID,
one exact contract-clause kind, typed scalar binders, one scalar proposition,
and an evidence requirement.

```semaprax
module arithmetic.laws;

@id("arithmetic.add.monotonic")
law contract "arithmetic.add" ensures (left: i64, right: i64)
    left + right >= left
    evidence smt_proved;
```

The grammar is:

```text
law-declaration := "@id" "(" STRING ")"
                   "law" "contract" STRING ("requires" | "ensures")
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

The subject string is a declaration identity, never a display name. The
proposition must exactly select one existing `requires` or `ensures` clause of
that function. Its LAW-01 lowering preserves the subject ID, clause kind,
canonical proposition, and evidence requirement. Binders and source spans are
available to source diagnostics and query presentation; they do not invent a
new proof selector.

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

## Canonical projection and diagnostics

Canonical formatting emits the module header, one blank line before each law,
the explicit ID, typed binders in source order, normalized scalar proposition,
and exactly one final LF. Comments are trivia and never contribute to the
logical selector or LAW-01 digest.

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
separate evidence sources for the same LAW-01 obligation.

## Non-claims

Native law declarations do not execute functions, introduce dependent types,
authorize a solver or proof kernel, recursively discover source files, alter
ordinary function contract behavior, or turn incomplete proof work into an
accepted Project candidate.
