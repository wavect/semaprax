# Types: records, variants, Option, Result

After this page you can model your data with records (fields), variants (one of
several cases), `Option` (maybe absent), and `Result` (value or error). Classes
have [their own page](classes.md).

| Your data | Use |
| --- | --- |
| A point with `x` and `y` | `record` |
| A shape that is a dot or a box | `variant` |
| A value that may be missing | `Option<T>` |
| A call that can fail | `Result<T, E>` |
| A value with methods | [`class`](classes.md) |

The examples on this page build records and variants. The default interpreter
behind `semaprax run` does not admit those (`SPX-F102`), so run them with
`semaprax run file.spx --native`, which compiles generated C11. `check` and
`fmt` need no flag.

## Group fields with a record

<!-- native-checked: {"stdout":"12\n"} -->
```semaprax
module app.data;

@id("data.point")
record Point {
    @id("data.point.x")
    x: i64,
    @id("data.point.y")
    y: i64,
}

@id("app.main")
fn main() -> i64
{
    let mut origin = Point { x: 1, y: 2 };
    origin.x = origin.x + 1;
    let moved = origin with { y: 10 };
    moved.x + moved.y
}
```

- A record literal names every field, in any order. A missing field is
  `SPX-T213`.
- `origin.x = …` changes a field and needs `let mut`.
- `origin with { y: 10 }` builds a new record and leaves `origin` unchanged.
- Give each field its own `@id`.
- Records have no methods: `point.get()` is `SPX-T203`. Call `get(point)`.

## Pick one case with a variant

<!-- native-checked: {"stdout":"42\n"} -->
```semaprax
module app.shapes;

@id("data.shape")
variant Shape {
    @id("data.shape.dot")
    Dot,
    @id("data.shape.box")
    Box {
        @id("data.shape.box.width")
        width: i64,
        @id("data.shape.box.height")
        height: i64,
    },
}

@id("data.area")
fn area(shape: Shape) -> i64
{
    match shape { Shape::Dot {} => 0, Shape::Box { width: w, height: h } => w * h, }
}

@id("app.main")
fn main() -> i64
{
    area(Shape::Box { width: 6, height: 7 })
}
```

A case without data is declared `Dot,` and written `Shape::Dot {}` everywhere
else. A `match` must cover every case, so adding a case makes the compiler
point at each `match` you must update. See [Matching](matching.md).

## Handle missing values and errors

`Option<T>` has `Some { value }` and `None {}`. `Result<T, E>` has
`Ok { value }` and `Err { error }`. Both are ordinary generic variants, so one
spelling rule covers them:

| Where | Spelling | Example |
| --- | --- | --- |
| Build a generic variant | with type arguments | `Option<i64>::Some { value: 1 }` |
| Match a generic variant | without them | `Option::Some { value: v } => …` |
| Call a generic function | with type arguments | `identity<i64>(4)` |

`Some(1)` and bare `None` do not exist (`SPX-T203`, `SPX-T202`). Leaving off
the type arguments when you build one is `SPX-T221`.

<!-- native-checked: {"stdout":"20\n"} -->
```semaprax
module app.checked;

@id("data.checked_div")
fn checked_div(left: i64, right: i64) -> Result<i64, i64>
{
    if right == 0 { Result<i64, i64>::Err { error: 1 } } else { Result<i64, i64>::Ok { value: left / right } }
}

@id("data.half_of_quotient")
fn half_of_quotient(left: i64, right: i64) -> Result<i64, i64>
{
    let quotient = checked_div(left, right)?;
    checked_div(quotient, 2)
}

@id("app.main")
fn main() -> i64
{
    match half_of_quotient(80, 2) { Result::Ok { value: v } => v, Result::Err { error: code } => code, }
}
```

`expr?` returns early with the error when `expr` is an `Err`. It works only in
a function that itself returns a `Result` (`SPX-T218` elsewhere), so `main`
uses `match`.

An arm cannot build a record or variant (`SPX-T258`). Pull scalars out of the
`match`, then build the value with `if`.

## Make a type generic

<!-- native-checked: {"stdout":"3\n"} -->
```semaprax
module app.pair;

@id("data.pair")
record Pair<T> {
    @id("data.pair.first")
    first: T,
    @id("data.pair.second")
    second: T,
}

@id("data.swap")
fn swap(pair: Pair<i64>) -> Pair<i64>
{
    Pair<i64> { first: pair.second, second: pair.first }
}

@id("app.main")
fn main() -> i64
{
    let swapped = swap(Pair<i64> { first: 1, second: 2 });
    swapped.first + swapped.second
}
```

Write the type arguments when you construct a generic value. Generic
functions are in [Functions](functions.md).

## Choose the boundary before you move a helper

Standalone files accept any of these types in any function. A Project export
is stricter: the default profile allows only Copy scalars in public
signatures (`SPX-G174`). Records, variants, `Option`, and `Result` can still
appear inside a function. Pick a [profile](../projects/profiles.md) before you
move a helper to a project.

Return `Option` or `Result` instead of a sentinel value such as `-1`, so
callers must handle each case.

Exact rules: [RFC 0002](https://github.com/wavect/semaprax/blob/main/docs/RFC-0002-ALGEBRAIC-DATA.md).
