# Language Ergonomics v1

Audience: language users, agent authors, and compiler contributors.

Status: Partial. Statement `if` is parse-level sugar and runs wherever a
value `if` runs: the reference interpreter, native C11, and Core Wasm.
Conversions v1 (numeric conversions and `string_from_str`) runs on the
reference interpreter and native C11 (`run`, `run --native`,
`build --target native`); every Core Wasm lane refuses it with one stable
diagnostic (`SPX-W116`).

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

## Conversions v1

There are no casts: `x as f64` stays `SPX-P106`, and its help names the
functions below. They are compiler-owned and reserved like the `string_*`
names (declaring one is `SPX-S113`); each call resolves to an ordinary
monomorphic call with the stable identity below, so no prelude byte, graph
schema version, or earlier program's projection changes.

| Function | Stable identity | Signature |
| --- | --- | --- |
| `f64_from_i64` | `core.num.f64_from_i64` | `(value: i64) -> f64` |
| `i64_from_f64` | `core.num.i64_from_f64` | `(value: f64) -> i64` |
| `usize_from_i64` | `core.num.usize_from_i64` | `(value: i64) -> usize` |
| `i64_from_usize` | `core.num.i64_from_usize` | `(value: usize) -> i64` |
| `string_from_str` | `core.string.from_str` | `(s: borrow str) -> string` |

### Numeric semantics

- `f64_from_i64` returns the `f64` nearest to `value`, ties to even. It is
  exact for every magnitude up to 2^53 and never fails. C's `(double)`, the
  interpreter's Rust `as f64`, and IEEE-754 round-to-nearest agree on it.
- `i64_from_f64` truncates toward zero (`-2.75` gives `-2`). NaN fails with
  `semaprax.convert.v1` code 2; a value outside `[-2^63, 2^63)` fails with
  code 1. `9223372036854775807.0` is `2^63` as an `f64`, so it fails.
- `usize_from_i64` fails with code 1 for a negative value.
- `i64_from_usize` fails with code 1 above `9223372036854775807`.
- Every other value converts exactly.

A failure is the checked status of a failing operation, exactly like a
checked arithmetic overflow: it propagates out of the function, runs the
enclosing cleanup, and `semaprax run` prints one stderr line such as
`semaprax.convert.v1/1 (conversion out of range)` and exits 1. The status
class is `adapter` and the status is not retryable.

The conversions take and return Copy scalars, so they are admitted wherever a
scalar call is: in expressions, contracts, `while` conditions, and loop
bodies.

```text
let average = f64_from_i64(total) / f64_from_i64(count);
let rounded = i64_from_f64(average * 10.0);
let mut index = 0usize;
while index < usize_from_i64(limit) {
    sum = sum + i64_from_usize(index);
    index = index + 1usize;
    0
}
```

### `string_from_str`

`string_from_str(s)` copies a borrowed `str` view into a new owned `string`,
so text that arrives as a view (a command-line argument from `arg_utf8`, or a
`borrow str` parameter) can be compared, stored, and passed to every
`string_*` operation. Owned strings compare by content with `==` and `!=`:

```text
let raw = arg_utf8(0usize);
let flag = string_from_str(raw);
let top = if flag == "--top" { 1 } else { 0 };
```

The copy is an ordinary owned temporary: it is released on every exit, and
the allocation-counting native harness observes no leak. The borrowed view
is not consumed.

### Backends

The reference interpreter and native C11 implement the family; native C
inlines each numeric conversion and its range checks without a runtime
helper. Every Core Wasm lane refuses the family up front with
`SPX-W116`: "Conversions v1 operation `f64_from_i64` is not lowered to Core
Wasm; run it on the reference interpreter or native C11".

## Evidence

`tests/language/statement_if.rs` parses a sugared corpus (straight-line code,
`while` bodies, nested statement `if`s, `else if` chains, empty `else`, a
trailing `;`, a discarded value `if`, and a source that already names
`_if1`), compares it with its pinned canonical text by graph revision, graph
JSON, and Core Wasm bytes, and runs it on the reference interpreter, native C
through `semaprax run --native`, and Core Wasm in Node. A `for` body corpus
runs on the interpreter and native C. Diagnostic regressions pin the refusals
above.
