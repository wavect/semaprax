# Functions, generics, function values

Named functions, generic functions, and functions-as-values — the three
levels of abstraction, each with explicit types everywhere.

## Generic functions

Type parameters go in angle brackets after the name. **Every call spells its
type arguments** — there is no inference at call sites:

```semaprax
module app.identity;

@id("generic.identity")
fn identity<T>(value: T) -> T
{
    value
}

@id("app.main")
fn main() -> i64
{
    identity<i64>(41) + 1
}
```

- `identity(4)` without arguments fails (`SPX-T225`); write
  `identity<i64>(4)`.
- Generic records and variants work the same way: declare `record Box<T>`,
  construct `Box<i64> { … }`.
- Private helpers may use one scoped generic `T` for iterator parameters and
  results (see [Loops](loops.md)).

## Function values as parameters

A parameter of type `fn(T) -> U` takes a function. Pass an anonymous `fn`
literal — same spelling as a named function, without the name and `@id`:

```semaprax
module example.fold;

@id("iterator.fold")
fn fold<T, A>(input: own Iter<T>, initial: A, combine: fn(A, T) -> A) -> A
{
    let mut accumulator = initial;
    for own item in input {
        accumulator = combine(accumulator, item);
        0
    }
    accumulator
}

@id("app.main")
fn main() -> i64
{
    let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 20), 22);
    let total = fold<i64, i64>(vec_into_iter<i64>(input), 0, fn(acc: i64, value: i64) -> i64 { acc + value });
    total
}
```

Rules for literals: no trailing comma in the parameter list, scalar-friendly
bodies, and at most 8 parameters in generic collection positions. Bind a
reused literal to a name instead of repeating it:

```semaprax
let positive = fn(value: i64) -> bool { value > 0 };
```

## Closures: snapshots and owning capture

A closure literal snapshots the Copy-scalar bindings it reads — later
mutations of the original don't affect the snapshot:

```semaprax
module app.snapshot;

@id("app.main")
fn main() -> i64
{
    let limit = 10;
    let within = fn(value: i64) -> bool { value < limit };
    if within(5) { 0 } else { 1 }
}
```

Constraints that keep closures predictable: bodies admit ordinary scalar
expressions and calls to known function values; closures can't nest inside
generic collection positions (`SPX-T288`); effectful captures are rejected.

The bounded owning-capture form transfers exactly one lexical owned `Bytes`
capture and takes zero explicit parameters:

```semaprax
own fn() -> i64 { 42 }
```

Owning closures are not admitted inside generic functions (`SPX-T291`).

## Best practices

1. **Prefer a named generic function** over a literal when the logic has a
   name, a contract, or more than one caller — named functions get `@id`,
   contracts, and testability.
2. **Keep literals tiny.** A literal longer than one expression usually wants
   to be a helper.
3. **Thread ownership explicitly.** `own Iter<T>` parameters consume; pass
   `rest` onward or let scope cleanup settle it — never reuse a consumed
   iterator.

Exact rules: [Function Values v1](https://github.com/wavect/semaprax/blob/main/docs/FUNCTION-VALUES-V1.md),
[Scalar Snapshot Closures v1](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-V1.md),
[Generic and Loop Closures v2](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-V2.md),
[Bounded Owning-Capture Closures v1](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-OWNING-V1.md).
