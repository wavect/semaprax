# Types: records, variants, classes

Choose a type based on the question your data answers. A **record** groups
fields that belong together. A **variant** represents one of several possible
cases. A **class** groups fields with methods that operate on them.

Declare these types at module level, then construct their values inside
functions. The examples below are complete standalone modules. When moving
helpers into a project, use the matching [profile](../projects/profiles.md)
for their parameter and result types.

| Your data | A useful starting point |
| --- | --- |
| A point with `x` and `y` | A record. |
| A shape that is a dot or a box | A variant. |
| A value that may be absent | `Option<T>`. |
| An operation that succeeds or returns an error | `Result<T, E>`. |
| A counter with associated operations | A class. |

## Records

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

- Construction names **every** field, in any order — a missing field is
  `SPX-T213`.
- `record with { field: value }` is immutable update: it builds a new value.
- Field mutation (`origin.x = …`) needs a `let mut` binding.
- Give every field its own `@id`, just like the record itself.

The record example returns `12`: mutation changes `x` to `2`, and the update
builds a value with `y = 10`. A field's name is its local spelling; its `@id`
is the persistent identity used by semantic tools.

## Variants, Option, Result

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

Payload-less cases are declared `Dot,` and written `Shape::Dot {}` everywhere
else. The spelling rules for generics are strict — get them wrong and the
compiler tells you exactly which side to fix:

| Position | Spelling | Example |
| --- | --- | --- |
| Construct a generic variant | **with** type arguments | `Option<i64>::Some { value: 1 }` |
| Match a generic variant | **without** type arguments | `Option::Some { value: v } => …` |
| Call a generic function | **with** type arguments | `identity<i64>(4)` |

`Some(v)` / `None` bare constructors don't exist. `Option` and `Result` are
ordinary generic variants, so the same table covers them:

```semaprax
module app.checked;

@id("data.checked_div")
fn checked_div(left: i64, right: i64) -> Result<i64, i64>
{
    if right == 0 { Result<i64, i64>::Err { error: 1 } } else { Result<i64, i64>::Ok { value: left / right } }
}

@id("app.main")
fn main() -> i64
{
    match checked_div(8, 2) { Result::Ok { value: v } => v, Result::Err { error: code } => code, }
}
```

Match arms can't construct nominal aggregates (`SPX-T258`): bind scalars out
of the match first, or build the record/variant with `if` instead.

`Result<i64, i64>` uses its first type for success and its second for the error.
Here, error code `1` means the divisor was zero. The example demonstrates that
one error case; integer arithmetic still has checked overflow behavior.

## Classes

Classes are records with methods. Methods take `self` explicitly and are
called with dot syntax — the **only** dot-callable values in the language
(records have no methods, so `point.get()` fails):

```semaprax
module app.counter;

@id("data.counter")
class Counter {
    @id("data.counter.value")
    value: i64,

    @id("data.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter
{
        Counter { value: self.value + amount }
    }
}

@id("app.main")
fn main() -> i64
{
    let counter = Counter { value: 1 };
    counter.bumped(1).value
}
```

`class Dog : Animal` inherits; `super.method()` dispatches to the parent.

## Best practices

1. **Prefer records + free functions** for data; reach for classes only when
   methods genuinely belong to the value.
2. **Return `Option`/`Result`, don't invent sentinel values.** Callers must
   handle every case — the exhaustiveness check is the point.
3. **Choose the boundary before moving the helper.** The default scalar
   Project route has scalar public boundaries. Owned-data routes and selected
   private helper profiles admit more shapes. Follow [Profiles](../projects/profiles.md)
   rather than assuming a standalone helper fits every export target.

Exact data-model rules: [RFC 0002](https://github.com/wavect/semaprax/blob/main/docs/RFC-0002-ALGEBRAIC-DATA.md).
