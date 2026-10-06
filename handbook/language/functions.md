# Functions, generics, function values

After this page you can write generic functions, pass a function to another
function, and capture a value in a closure.

## Write a generic function

<!-- handbook-smoke: {"stdout":"42\n"} -->
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

Type parameters go in angle brackets after the name. At a call, write the type
arguments: `identity<i64>(41)`. The compiler can fill them in when the
arguments decide them (`identity(41)` also works), but this is a private
profile and not a promise for public signatures. If it cannot decide, you get
an error that asks for them (`SPX-T225`). Always write them for `vec_*`,
`box_*`, and `iter_*` calls (`SPX-T281`).

- A generic function cannot call itself, directly or through other generic
  functions (`SPX-T226`).
- Generic records and variants spell their arguments when you build one:
  `Pair<i64> { first: 1, second: 2 }`. See [Types](types.md#make-a-type-generic).
- Generic functions stay inside a module. Public Project signatures take
  Copy scalars only unless the project profile says more
  ([Profiles](../projects/profiles.md)).

## Pass a function

A parameter of type `fn(A) -> B` takes a function. Pass a named function, or an
unnamed `fn` literal, which is a function without its name and `@id`:

<!-- handbook-smoke: {"stdout":"12\n"} -->
```semaprax
module app.callbacks;

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

@id("fn.twice")
fn twice(value: i64, step: fn(i64) -> i64) -> i64
{
    step(step(value))
}

@id("fn.inc")
fn inc(value: i64) -> i64
{
    value + 1
}

@id("app.main")
fn main() -> i64
{
    let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 5), 5);
    let total = fold<i64, i64>(vec_into_iter<i64>(input), 0, fn(acc: i64, value: i64) -> i64 { acc + value });
    twice(total, inc)
}
```

A function value has up to eight parameters. Its target must be a local,
effect-free, non-generic function, or a literal. A literal takes no trailing
comma. Bind one to a name to reuse it:
`let positive = fn(value: i64) -> bool { value > 0 };`

## Capture a value in a closure

A literal copies the Copy scalars it reads at the moment it is built. Later
changes to the original do not reach the copy:

<!-- handbook-smoke: {"stdout":"1\n"} -->
```semaprax
module app.snapshot;

@id("app.main")
fn main() -> i64
{
    let mut limit = 10;
    let within = fn(value: i64) -> bool { value < limit };
    limit = 0;
    if within(5) { 1 } else { 0 }
}
```

Closure bodies are ordinary scalar expressions and calls. A closure cannot
change what it captured, cannot capture an effect, and cannot sit inside a
generic collection position (`SPX-T288`). The Rust-style `|x| x + 1` is
`SPX-P201`.

## Move one owned value into a callback

`once fn` captures one owned `Bytes` value and carries it in the type
`FnOnce() -> i64`. Calling it consumes it, and so does passing it to an `own`
parameter:

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module affine.example;

@id("affine.consume")
fn consume(payload: own Bytes) -> i64
{
    42
}

@id("affine.make")
fn make() -> FnOnce() -> i64
{
    let payload = bytes_zeroed(4usize);
    once fn() -> i64 { consume(payload) }
}

@id("affine.run")
fn run(callback: own FnOnce() -> i64) -> i64
{
    callback()
}

@id("app.main")
fn main() -> i64
{
    let callback = make();
    run(callback)
}
```

The body is exactly one call that passes the captured buffer to a function
`fn target(payload: own Bytes) -> i64`. Reusing the buffer or the callback after
the move is `SPX-O101`. There are no other captures, parameters, or generics
(`SPX-T308`).

The older `own fn() -> i64 { target(payload) }` form is in the same family. In
0.9.0, `run` executes it on all three backends, but `check` reports `SPX-H006`.
Use `once fn`. Closures that borrow a `borrow str` parameter are a narrower
profile, see
[Synchronous Borrowed Text Closures v1](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-BORROWED-V1.md).

## Pick a name or a literal

Use a named function when the logic has a meaning, a contract, or a second
caller: it gets an `@id`, contracts, and tests. Keep a literal to one short
expression.

Exact rules: [Function Values v1](https://github.com/wavect/semaprax/blob/main/docs/FUNCTION-VALUES-V1.md),
[v2](https://github.com/wavect/semaprax/blob/main/docs/FUNCTION-VALUES-V2.md),
[Scalar Snapshot Closures](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-V1.md),
[Generic and Loop Closures](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-V2.md),
[Retained Affine Callback v1](https://github.com/wavect/semaprax/blob/main/docs/AFFINE-CALLBACK-V1.md),
[Owning-Capture Closures v1](https://github.com/wavect/semaprax/blob/main/docs/CLOSURES-OWNING-V1.md).
