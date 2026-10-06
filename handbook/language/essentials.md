# Essentials

After this page you can write a complete Semaprax program: functions, values,
`if`, `while`, and `match`. Save each example as a `.spx` file and run it with
`semaprax run file.spx`.

## Write a program

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.basics;

@id("basics.double")
fn double(value: i64) -> i64
{
    value * 2
}

@id("app.main")
fn main() -> i64
{
    double(21)
}
```

`run` prints `42`, the value `main` returns. The rules:

- A file starts with one `module dotted.name;` line.
- Give every declaration an `@id("dotted.name")`. It is the declaration's
  permanent identity. Without it, `check` warns `SPX-S103`, and renaming the
  function changes its identity.
- The entry point is exactly `fn main() -> i64`.
- A function body is statements followed by one final expression. That
  expression is the result. There is no `return`.
- To use a function from another file, a Project imports it by `@id` with
  `use function @id("…") from module as name;`. See
  [Modules and imports](../projects/modules.md).
- Run `semaprax fmt file.spx` to apply the one canonical layout. It keeps `//`
  comments.

## Pick a type

| Type | Example | Use it for |
| --- | --- | --- |
| `i64` | `42`, `-1` | Integers. A plain integer literal is `i64`. |
| `i32` | `42i32` | 32-bit integers. |
| `u8` | `255u8` | One byte. |
| `usize` | `3usize` | Lengths and indexes. |
| `f64`, `f32` | `1.5`, `1.5f32` | Floating point. |
| `bool` | `true`, `false` | Conditions. |
| `char` | `'a'`, `'\n'`, `'\u{2603}'` | One Unicode scalar. |
| `string` | `"hello"` | Owned UTF-8 text. `==` compares contents. |

These eight scalar types (everything except `string`) are Copy: using a value
does not consume it. Text and bytes follow ownership rules, see
[Ownership](ownership.md).

Operators never mix types. If `n` is a `usize`, write `n < 5usize`, not
`n < 5` (`SPX-T208`). Write `5i32` when an `i32` is expected, because `5` is an
`i64` (`SPX-T232`). Integer arithmetic is checked: overflow stops the program
with a status such as `addition overflow`, never wraps.

<!-- handbook-smoke: {"stdout":"3\n"} -->
```semaprax
module app.scalars;

@id("app.main")
fn main() -> i64
{
    let small = 1i32 + 2i32;
    let ratio = 1.5 * 2.0;
    let letter = 'a';
    if small == 3i32 && ratio > 2.5 && letter == 'a' && !(1 > 2) { 3 } else { 0 }
}
```

Use `&&`, `||`, and `!` for booleans. `&&` and `||` run their right side only
when needed, always left to right.

## Bind a value

`let count = 3;` makes an immutable binding. `let mut count = 3;` lets you
assign again with `count = count + 1;`. Parameters are immutable. There is no
`+=`, and a second `let` with the same name is an error (`SPX-T209`).

To ignore a result, bind it to `_`: `let _ = work(1);`. A bare `work(1);` is a
syntax error.

## Choose a value with if

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.branch;

@id("app.main")
fn main() -> i64
{
    let score = 42;
    let accepted = if score >= 40 { score } else { 0 };
    if accepted > 100 { 1 } else { if accepted > 10 { accepted } else { 2 } }
}
```

`if` is an expression and always has an `else`. Both branches have the same
type. For a third case, nest an `if` inside the `else` block. There is no
`else if`.

## Repeat with while

<!-- handbook-smoke: {"stdout":"6\n"} -->
```semaprax
module app.counting;

@id("app.main")
fn main() -> i64
{
    let mut next = 1;
    let mut total = 0;
    while next <= 3 {
        total = total + next;
        next = next + 1;
        next <= 3
    }
    total
}
```

The condition after `while` is checked before every pass. The body must end
with an expression, and `while` throws its value away. Ending a body with an
assignment is `SPX-P203`. There is no `break` or `continue`: put the exit test
in the condition. See [Loops](loops.md) for `for` and the loop limits.

## Pick a case with match

<!-- handbook-smoke: {"stdout":"-9\n"} -->
```semaprax
module app.classification;

@id("app.main")
fn main() -> i64
{
    let value = -2;
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, _ => 1, }
}
```

The first matching arm wins. `|` joins alternatives, `if` adds a guard, and `_`
matches anything. A match on numbers or chars needs a last arm without a guard
(`SPX-T257`). Every arm ends with a comma, including the last. Matching
variants is in [Matching](matching.md).

## Mistakes to skip

| You write | Error | Write this |
| --- | --- | --- |
| `return x;` | `SPX-P106` | Put `x` last in the block. |
| `else if c { … }` | `SPX-P106` | `else { if c { … } else { … } }` |
| `i += 1;` | `SPX-P201` | `i = i + 1;` |
| `f(x);` alone | `SPX-P106` | `let _ = f(x);` |
| `for i in 0..n` | `SPX-P106` | `while` with a counter, or `for item in vector` |
| `break`, `continue` | `SPX-P106` | Test in the `while` condition. |
| `x as i64` | `SPX-P106` | No casts. Keep one type and suffix literals. |
| `c ? a : b` | `SPX-P106` | `if c { a } else { b }` |
| `"a" + "b"` | `SPX-T250` | `string_concat("a", "b")` |
| `struct`, `enum`, `pub`, `const` | `SPX-P104` | `record`, `variant`; no visibility keyword. |
| `fn main() -> bool` | `SPX-T104` | `main` returns `i64`. Use `0` for success. |
| tuples, `()`, `fn f()` | `SPX-P106` | Declare a `record`; every function returns a value. |

When `check` fails, read the first error: its `help:` line is usually the fix.
For a code, run `semaprax help diagnostic SPX-T208`.

**Next:** [Types](types.md). Exact rules: [RFC 0001](https://github.com/wavect/semaprax/blob/main/docs/RFC-0001.md).
