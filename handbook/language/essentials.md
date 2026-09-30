# Essentials

The core language on one page: files, scalars, bindings, and control flow.

## File anatomy

```semaprax
module app.flow;

permit { clock.read }

@id("flow.digit_sum")
fn digit_sum(value: i64) -> i64
    requires value >= 0
    ensures result >= 0
{
    let mut remaining = value;
    let mut total = 0;
    while remaining > 0 {
        total = total + remaining % 10;
        remaining = remaining / 10;
        remaining > 0
    }
    total
}

@id("app.main")
fn main() -> i64
{
    digit_sum(98765)
}
```

- One `module dotted.name;` per file, always first.
- Optional `permit { … }` lists the effects this file may use.
- Every declaration gets an `@id("dotted.stable.name")`. Without one the
  compiler warns (`SPX-S103`) — and renames silently change identity.
- A function body is zero or more statements plus one **tail expression**.
  That expression is the return value. There is no `return`.

## Scalars

| Type | Literals | Notes |
| --- | --- | --- |
| `i64` | `42`, `-1` | Default integer; checked overflow |
| `i32` | `42i32` | Suffix required, no implicit widening |
| `u8` | `255u8` | Byte value |
| `usize` | `3usize` | Lengths and indices; compare only with `usize` |
| `f64`, `f32` | `1.5`, `1.5f32` | Floats |
| `bool` | `true`, `false` | `&&`, `||`, `!` (lazy, left to right) |
| `char` | `'a'`, `'\n'` | Single scalar value |
| `string` | `"text"` | Owned UTF-8; content equality with `==` |

**Operators never mix types.** If `n` is `usize`, `n < 5` fails — write
`n < 5usize`. Integer literals are `i64` unless suffixed, so
`let a: i32 = 5` fails; write `5i32`.

## Bindings and mutation

- `let x = …;` is immutable. `let mut x = …;` allows `x = x + 1;`.
- Assignment is a statement, never an expression. No `+=`, no shadowing
  (`let x = 1; let x = x + 1;` fails — pick a new name).
- Parameters are always immutable.

## Control flow

**`if` is an expression and always has `else`.** Nest `if` inside `else`
instead of `else if`. Every branch yields a value — a branch that only
assigns still needs a trailing expression:

```semaprax
module app.branch;

@id("app.main")
fn main() -> i64
{
    let mut x = 0;
    let y = if x == 0 { x = 1; x } else { x };
    y
}
```

**`while` repeats while its tail expression is `true`.** The condition is
checked before every iteration, and the body's final expression is the
continuation condition — a body ending in an assignment fails (`SPX-P203`).
While bodies admit Copy-scalar operations; building records or variants
inside one fails (`SPX-T252`) — compute scalars in the loop, construct after.

**`match` needs a final catch-all** (`_` or a binding, no guard):

```semaprax
module app.flow;

@id("flow.classify")
fn classify(value: i64) -> i64
{
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, _ => 1, }
}

@id("app.main")
fn main() -> i64
{
    classify(-2)
}
```

## What doesn't exist (and what to write instead)

| Instead of… | Write… |
| --- | --- |
| `return x;` | `x` as the tail expression |
| `else if` | `else { if … }` |
| `for i in 0..n` | `while` with a `let mut` counter |
| `f(x);` as a statement | `let _ = f(x);` or make it the tail |
| tuples, `struct`, `enum` | `record` / `variant` (see [Types](types.md)) |
| `fn f()` / `-> ()` | Every function returns a value; `main` returns `i64` (`0` = success) |

Every declaration, field, and match arm ends with `,` — including the last
one. When in doubt, run `fmt` and read the diagnostic: the fix is usually in
the `help` line. Full rulebook: [RFC 0001](https://github.com/wavect/semaprax/blob/main/docs/RFC-0001.md).
