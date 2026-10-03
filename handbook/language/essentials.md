# Essentials

Learn the pieces that appear in almost every Semaprax program: values,
functions, bindings, branches, and loops. The complete examples below can be
saved as separate `.spx` files and run with `semaprax run` after formatting
and checking them.

## Functions return the last expression

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

`value` is the function's input. `-> i64` is its output type. The expression
`value * 2` produces the result. A caller writes `double(21)`.

The ID `basics.double` identifies the declaration for tools and imports.
The display name `double` is what you type at this call site. Giving the
function an explicit ID lets a later display rename keep the same identity.

## Choose the right type

A **scalar** holds one basic value, such as a number or a boolean. These are
Copy values: using one does not consume it.

| Type | Write a value as | Use it for |
| --- | --- | --- |
| `i64` | `42`, `-1` | Ordinary integer calculations. Unsuffixed integers use this type. |
| `i32` | `42i32` | Explicit 32-bit integer values. |
| `u8` | `255u8` | Individual bytes. |
| `usize` | `3usize` | Collection lengths and indexes. |
| `f64`, `f32` | `1.5`, `1.5f32` | Floating-point calculations. |
| `bool` | `true`, `false` | Conditions. |
| `char` | `'a'`, `'\n'` | One Unicode scalar value. |

Owned text uses `string`, written as `"hello"`. Its ownership and borrowed
views have their own rules; see [Ownership](ownership.md).

Operators do not silently mix numeric types. Write `index < 5usize` when
`index` is a `usize`. Writing `index < 5` compares different types and fails.
Likewise, an `i32` literal is `5i32`, even beside an `i32` annotation.

## Bind a value, then change it explicitly

`let count = 3;` creates an immutable binding. `let mut count = 3;` lets you
assign a new value to that binding. Function parameters remain immutable.

A statement such as `count = count + 1;` performs work and ends with `;`.
A block still needs a final expression to supply its value. Semaprax does not
use `+=`, and a second `let` with the same local name is not a replacement for
assignment.

## Choose a value with if

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.branch;

@id("app.main")
fn main() -> i64
{
    let score = 42;
    let accepted = if score >= 40 { score } else { 0 };
    accepted
}
```

Both branches produce the same type. `if` always has an `else`. To express a
second condition, nest another `if` inside the `else` block.

For booleans, use `&&`, `||`, and `!`. The right side of `&&` or `||` is only
evaluated when needed. This is useful when the first condition protects an
operation in the second.

## Repeat work with while

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
        0
    }
    total
}
```

The condition `next <= 3` is checked before each iteration. The loop adds
`1`, `2`, and `3`, then stops. The result is `6`.

The final `0` supplies the body's required expression, but the loop discards
that value. **The condition after `while` controls repetition.** Update the
state used by that condition so the loop can finish.

For vectors, the language also has `for item in values` and consuming
`for own item in iterator`. See [Loops](loops.md) for complete examples. A
Rust-style numeric range such as `0..n` is not this traversal syntax.

## Select a case with match

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

The first matching arm supplies the result. `|` combines patterns, `if` adds
a condition to an arm, and `_` matches anything left. Scalar matches need an
unguarded final catch-all. Matching variants is explained in [Matching](matching.md).

## Habits to learn early

| You might try | Write this instead |
| --- | --- |
| `return value;` | Put `value` at the end of the block. |
| `else if condition` | Put a nested `if` inside `else { ... }`. |
| `do_work();` as a standalone statement | Bind the result, for example `let ignored = do_work();`. |
| `fn work() -> ()` | Choose a supported result type; functions return a value. |
| `struct` or `enum` | Use `record` or `variant`. |
| String concatenation with `+` | Use `string_concat`. |

Fields, variant cases, and match arms use their required commas. Ordinary
function declarations do not end with a comma. Let `semaprax fmt` handle the
standard layout after the parser accepts the source.

**Next:** [Group values with records, variants, and classes](types.md).
Exact rules: [RFC 0001](https://github.com/wavect/semaprax/blob/main/docs/RFC-0001.md).
