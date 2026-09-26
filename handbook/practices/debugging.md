# Debugging diagnostics

Every Semaprax diagnostic has the same anatomy: a stable `SPX-…` code, a
message, a line:column location, and usually a `help` line with the fix.
**Match on the code, not the wording** — wording evolves, codes don't.

## The loop

1. Run `semaprax fmt <file>` first — a surprising number of "errors" are
   layout the formatter resolves.
2. Run `semaprax check <file>` (add `--json` for machine-readable `code`,
   `message`, `location`, `help`).
3. Fix the **first** diagnostic at its reported location. Later diagnostics
   are often knock-on effects.
4. Look up any code: `semaprax help diagnostic <SPX-code>`, or list them
   with `semaprax help diagnostic codes`. Codes are case-sensitive.

```sh
semaprax help diagnostic SPX-T208
semaprax explain SPX-T208 --json   # structured explanation, v0.6.0
```

## The top fixes

The mistakes every newcomer (human or agent) makes, condensed from the
compiler's own reference card:

| You wrote | Code | Fix |
| --- | --- | --- |
| `return 42;` | `SPX-P106` | Tail expression: `42` |
| `else if` | `SPX-P106` | `else { if … }` |
| `while` body ending in assignment | `SPX-P203` | End the body with the continuation condition |
| `for i in 0..n` | `SPX-P106` | `while` with a `let mut` counter |
| `f(x);` as a statement | `SPX-P106` | `let _ = f(x);` |
| `let t = (1, 2);` | `SPX-P106` | No tuples; declare a `record` |
| `i = i + 1` on immutable `i` | `SPX-U101` | `let mut i = …` |
| `let x = 1; let x = …` | `SPX-T209` | No shadowing; new name |
| `index + 1` with `index: usize` | `SPX-T208` | `index + 1usize` (no mixed-type ops) |
| `let a: i32 = 5` | `SPX-T232` | `5i32` — literals default to `i64` |
| `"a" + "b"` | `SPX-T250` | `string_concat("a", "b")` |
| `Some(1)` / `None` | `SPX-T203` | `Option<i64>::Some { value: 1 }` / `Option<i64>::None {}` |
| `Some(b) =>` in patterns | `SPX-P106` | `Option::Some { value: b } =>` |
| `f("abc")` for `borrow str` | `SPX-T205` | Bind, then `f(string_as_str(s))` |
| `string_as_str("lit")` | `SPX-T266` | Bind the literal first |
| `point.get()`, `s.len()` | `SPX-T203` | Only classes have methods: `get(point)`, `string_len(s)` |
| Second use after `own` move | `SPX-O101` | Callee takes `borrow`, or pass a fresh value |
| `fn main() -> bool` | `SPX-T104` | `main` returns `i64`; `0` = success |
| `fn f()` / `-> ()` | `SPX-P106` | Every function returns a value |
| Missing `permit` / `uses` | `SPX-E101` / `SPX-E102` | Declare the effect at module and function level |
| Last field/arm without `,` | `SPX-P106` | Trailing comma everywhere, including last |
| Non-canonical manifest | `SPX-J100` | `help` names the first differing line — match it |

When a fix isn't obvious, ask the compiler for a smaller question:
`semaprax help language <topic>` (`scalars`, `ownership`, …),
`semaprax help shapes <kind>` for minimal declaration examples, and
`semaprax help library <name>` for stdlib signatures. The full card is
[Agent quick reference](https://github.com/wavect/semaprax/blob/main/docs/AGENT-QUICK-REFERENCE.md),
also printed offline by `semaprax help language`.
