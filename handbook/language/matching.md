# Matching

After this page you can pick a result with `match` on numbers, bytes, chars,
and variants. Arms are tried in order and the first match wins.

## Match numbers, bytes, and chars

<!-- handbook-smoke: {"stdout":"-9\n"} -->
```semaprax
module app.sign;

@id("refutable.sign_class")
fn sign_class(value: i64) -> i64
{
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, n => 1, }
}

@id("app.main")
fn main() -> i64
{
    sign_class(-2)
}
```

| Pattern | Example | Meaning |
| --- | --- | --- |
| Literal | `0 => …` | Equal to the literal. |
| Alternatives | `-1 \| -2 => …` | Any of them. |
| Binding | `n => …` | Anything, named `n`. |
| Guard | `n if n < 0 => …` | The binding, plus a condition. |
| Wildcard | `_ => …` | Anything, unnamed. |

- The last arm must be `_` or a binding **without** a guard. Otherwise you get
  `SPX-T257`.
- Order matters. Put specific arms first. In the example, `-2` is caught by the
  alternatives before the guard sees it.
- Chars and bytes work the same way, with their own literals:

<!-- handbook-smoke: {"stdout":"13\n"} -->
```semaprax
module app.route;

@id("refutable.digit_name")
fn digit_name(digit: u8) -> i64
{
    match digit { 0u8 => 10, 9u8 => 90, k if k > 4u8 => 2, _ => 1, }
}

@id("refutable.route")
fn route(code: char) -> i64
{
    match code { 'a' => 1, 'b' | 'c' => 2, _ => 3, }
}

@id("app.main")
fn main() -> i64
{
    digit_name(4u8) + route('c') + digit_name(0u8)
}
```

A range such as `0..=5` is not a pattern. Use a guard.

## Match a variant

Name the case and bind each payload field as `field: name`. Building a generic
variant spells its type arguments. Matching one does not:

<!-- native-checked: {"stdout":"4\n"} -->
```semaprax
module app.pick;

@id("data.first_positive")
fn first_positive(left: i64, right: i64) -> Option<i64>
{
    if left > 0 { Option<i64>::Some { value: left } } else { if right > 0 { Option<i64>::Some { value: right } } else { Option<i64>::None {} } }
}

@id("app.main")
fn main() -> i64
{
    match first_positive(0, 4) { Option::Some { value: v } => v, Option::None {} => 0, }
}
```

- A case without data is matched as `Shape::Dot {}`, never bare `Dot`.
- A `match` over a variant must cover every case. Add a case and the compiler
  lists each `match` to update.
- `Some(v)` is not a pattern. Write `Option::Some { value: v }`.
- Arms produce numbers, bools, and calls, not new records or variants
  (`SPX-T258`). Match out the scalars first, then build the value with `if`.
- A missing comma between arms is a syntax error. The last arm needs a comma.
- `if let` does not exist. Use `match`.

Matching a call result in `main` needs `semaprax run file.spx --native`. The
default interpreter reports `SPX-F102`.

## Match an owned value

`match own` moves the value into the arms. Use it for `IterStep` from
`iter_next`:

```text
match own step { IterStep::Done {} => false, IterStep::Yield { item, rest } => keep(item), }
```

`Yield` gives a Copy `item` and the owning `rest`. The full example is in
[Loops](loops.md#step-by-hand).

## Keep arms short

Match where a value arrives, then work with plain scalars. If an arm needs its
own `match`, move it into a named function with its own `@id`.

Exact rules: [Refutable Match v1](https://github.com/wavect/semaprax/blob/main/docs/REFUTABLE-MATCH-V1.md),
[RFC 0002](https://github.com/wavect/semaprax/blob/main/docs/RFC-0002-ALGEBRAIC-DATA.md).
