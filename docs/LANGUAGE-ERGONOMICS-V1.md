# Language Ergonomics v1

Audience: language users, agent authors, and compiler contributors.

Status: Partial. Statement `if` is parse-level sugar and runs wherever a
value `if` runs: the reference interpreter, native C11, and Core Wasm.

## Objective

Agents writing SEMAPRAX lose edit-check turns on habits the language rejects
even though their meaning is clear. v1 admits the most frequent of them with
exact, documented meaning and no change to any existing program's AST, graph,
cleanup plan, or backend output.

## Statement `if`

In statement position, a block may contain

```text
if <condition> { <statements> }
if <condition> { <statements> } else { <statements> }
if <condition> { <statements> } else if <condition> { <statements> } else { <statements> }
```

with or without a trailing `;`. Branch blocks may end without a final value.
This is admitted everywhere a statement is: function bodies, nested blocks,
`if` and `match` branches, and `while` and `for` bodies.

```text
while i < limit {
    if i % 2 == 0 { evens = evens + 1; }
    if line_ok { good = good + 1; } else { bad = bad + 1; }
    i = i + 1;
    i < limit
}
```

### Meaning

A statement `if` is sugar for the discarded value `if` agents previously
spelled by hand. The parser lowers

```text
if c { x = x + 1; }
```

to

```text
let _if1 = if c { x = x + 1; 0 } else { 0 };
```

- A branch that ends without a value yields the `i64` value `0`; a missing
  `else` is `else { 0 }`; `else if` is `else { if … }` exactly as in a value
  `if`.
- A branch that does end with a value keeps an integer literal value as is,
  and discards any other value with its own `let _if<n> = <value>;` before the
  `0`, so branches never disagree on the discarded type.
- A block admits no shadowing, so each discard takes the next name
  `_if1`, `_if2`, … that no identifier in the file already spells. The binding
  is an ordinary immutable `i64` local nothing reads.

Because the result is an ordinary value `if`, the condition must be `bool`
(`SPX-T210`), evaluation order, effects, ownership joins, cleanup plans, both
source verifiers, the semantic graph, and every backend are exactly those of
the value form. There is no new AST node, graph schema, or diagnostic.

### Canonical form

`fmt` writes the lowered spelling, as it writes `else if` as `else { if … }`:
`let _if1 = if c { x = x + 1; 0 } else { 0 };`. The sugared source and its
canonical text have the same graph revision and the same backend output. The
canonical text parses back to the same program, because the `_if<n>` names it
contains are then ordinary identifiers that later discards skip.

### Value `if` is unchanged

A chain in which every branch ends with a value and a final `else` exists is a
value `if`:

- followed by `}`, it is the block's final expression;
- followed by an operator, it starts the final expression
  (`if c { 1 } else { 2 } * 3`);
- followed by `;`, it is the expression statement it always was (`SPX-P106`);
- followed by another statement, it is discarded like any statement `if`.

### Diagnostics

A statement `if` yields no value, so a block that needs a value still needs a
final expression after it. When a statement `if` is the last item of such a
block, the diagnostic is the one the value grammar always produced for that
text: `SPX-P104` (``expected `else` ``) for `if c { 42 }`, and `SPX-P203`
with a help naming both fixes for `if c { x = 1; }`. Expression statements
inside a branch (`f(x);`) stay `SPX-P106`.

## Evidence

`tests/language/statement_if.rs` parses a sugared corpus (straight-line code,
`while` bodies, nested statement `if`s, `else if` chains, empty `else`, a
trailing `;`, a discarded value `if`, and a source that already names
`_if1`), compares it with its pinned canonical text by graph revision, graph
JSON, and Core Wasm bytes, and runs it on the reference interpreter, native C
through `semaprax run --native`, and Core Wasm in Node. A `for` body corpus
runs on the interpreter and native C. Diagnostic regressions pin the refusals
above.
