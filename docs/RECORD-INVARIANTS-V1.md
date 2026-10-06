# Record Invariants v1, Variant Equality, and Case Or-Patterns

Audience: language users, tool authors, and compiler contributors.

Status: Partial. This tranche adds three source forms that shorten entity-style
programs: invariants on records, `==`/`!=` on payload-free variants, and
or-patterns over payload-free variant cases. Evidence lives in
`tests/language/record_invariants.rs`, `tests/language/payload_free_variants.rs`,
and `examples/record_rules.spx`.

## Record invariants

```text
record Team {
    name: string,
    seats: i64,
}
    requires string_len(name) >= 2
    requires seats >= 1
```

Zero or more `requires` clauses follow a record's closing brace, one per line,
indented four spaces in canonical form. Each clause is a `bool` expression over
the record's fields by bare name and may call effect-free functions and the
compiler-owned string functions. Source verification checks the clauses
exactly like the preconditions of a function whose parameters are the fields:

| Code | Meaning |
| --- | --- |
| `SPX-C101` | The clause is not `bool`. |
| `SPX-C102` | The clause calls a function with effects. |
| `SPX-T269`, `SPX-T270` | The clause names a host or command I/O operation. |
| `SPX-T202` | The clause names something that is not a field, function, or builtin; the help lists the fields. |
| `SPX-C103` | The record is generic; v1 admits invariants only on non-generic records. |

Semantics: every new value of the record satisfies its clauses. A record
literal, a `base with { ... }` update, and a field assignment `t.f = v;` each
check the value they produce, in clause order; the first false clause fails the
enclosing function with the ordinary contract-failure status of a failing
precondition (`semaprax.contract.v1`, code 1). Copying an existing value checks
nothing, because every value was checked when it was produced.

Lowering happens once, in HIR resolution. A record `Name` with invariants gets
the synthesized function `Name#invariant(fields...) -> bool` whose `requires`
are the clauses, and, for a Copy record, `Name#check(value: Name) -> Name`
whose precondition calls it. Literals and updates become `Name#check(...)`
calls, and `t.f = v;` assigns `{ let #value = v; let #record = Name { ...,
f: #value }; #value }`, whose literal is checked. Evaluation order is
unchanged: initializers still run in authored order and the assigned value runs
before any field is read. `#` cannot occur in a source identifier, so the
synthesized names never collide. Every backend that executes a program
therefore enforces the clauses through its ordinary call and contract path: the
reference interpreter, native C (`run --native` prints
`requires <clause> in <record id>#invariant` with the field values), and Core
Wasm (`spx_contract_fail`). The semantic graph carries the clauses as the
`requires_graph` of the `#invariant` function node, and `semaprax doc` lists
them as the record's `Invariants`.

A record that holds an owned `string` has no executable value layout on any
backend in this release, so it gets `Name#invariant` without `Name#check`: its
clauses verify, format, reach the graph, and become validation rules in
`semaprax webapp`, which enforces them in the browser and on the server.

Nonclaims: invariants on classes, variants, or generic records; invariants
that relate several records; static discharge of the checks.

## Equality on payload-free variants

`a == b` and `a != b` are admitted when both operands have one non-generic
variant type whose every case has no payload, including against a constructor
such as `Status::Done {}`. The result is `bool` and compares the case. Both
verifiers share the admission check; any other variant stays `SPX-T207`, whose
help names `match`. The HIR keeps the ordinary `binary` node over the nominal
operands; the interpreter compares the authenticated case, native C compares
`spx_tag`, and Wasm compares the `i32` tag at offset zero.

## Or-patterns over payload-free cases

```text
match status { Status::Todo {} | Status::Doing {} => true, Status::Done {} => false, }
```

In a plain value `match` over a variant, `|` may join case patterns of the
scrutinee's own variant whose cases carry no payload. Each alternative covers
its case for exhaustiveness. Rejected shapes keep stable diagnostics: a payload
alternative is `SPX-M105`, a duplicate or already-covered case `SPX-M102`, a
foreign case `SPX-M103`, a mixed literal/case or-pattern or a guard `SPX-T254`
(its help names the admitted forms), and an explicit `match own`/`match borrow`
`SPX-O117`. The HIR keeps one arm whose pattern is an `or_pattern` of
field-less `variant_pattern` alternatives (Graph v16, as for every refutable
node). The cleanup plan and its independent replay test one `VariantCase`
decision per alternative, all selecting the arm's single entry block; the
interpreter, native C, and Wasm test the case tag against each alternative.
