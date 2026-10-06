# Loops and iterators

After this page you can repeat work three ways: `while` for a counter or other
scalar state, `for` to read a vector, and `for own` to drain an iterator.

| You want to | Use |
| --- | --- |
| Count or update scalar state | `while` |
| Visit each item of a vector and keep the vector | `for item in values` |
| Hand a vector to an iterator and consume it | `for own item in iterator` |

## Count with while

<!-- handbook-smoke: {"stdout":"6\n"} -->
```semaprax
module app.factorial;

@id("loops.factorial")
fn factorial(value: i64) -> i64
    requires value >= 0
{
    let mut remaining = value;
    let mut total = 1;
    while remaining > 1 {
        total = total * remaining;
        remaining = remaining - 1;
        remaining > 1
    }
    total
}

@id("app.main")
fn main() -> i64
{
    factorial(3)
}
```

The result is `3 * 2 * 1 = 6`. The rules:

- The condition is checked before every pass and must be a `bool`.
- The body ends with an expression. Its value is discarded, and the condition
  alone decides repetition. A body that ends after an assignment is `SPX-P203`.
- There is no `break` or `continue`.
- A body may use Copy scalars, calls that return scalars, and the
  `byte_get`/`Option<u8>` pattern. Building a record or variant, or calling a
  function that returns one, is `SPX-T252`. Loop over scalars, then build the
  value after the loop.
- `net_recv` returns an owned value and is not allowed in a body (`SPX-T270`).
  `bytes_zeroed` stays outside. The one buffer write a body may do is
  `buffer = bytes_set(buffer, index, value)`, see [Ownership](ownership.md).

## Visit a vector with for

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.traverse;

@id("app.main")
fn main() -> i64
{
    let values = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 20), 22);
    let mut total = 0;
    for item in values {
        total = total + item;
        0
    }
    total
}
```

`for item in values` visits the elements of a `Vec<T>` binding in index order.
`T` is one of the eight Copy scalars. The length is read once, the vector is
frozen inside the body, and the body's value is discarded. The vector must be an
immutable binding (`SPX-T284` for `let mut`) and a plain name, not a call. Do
not move or change it, or the item, inside the body. Build the vector first with
the [Vec calls](collections.md#build-and-read-a-vector).

## Drain an iterator with for own

`vec_into_iter<T>` moves a vector into an `Iter<T>`. `for own` consumes it:

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.consume;

@id("app.main")
fn main() -> i64
{
    let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 20), 22);
    let iterator = vec_into_iter<i64>(input);
    let mut total = 0;
    for own item in iterator {
        total = total + item;
        0
    }
    total
}
```

After the loop, `input` and `iterator` are both moved. Using either is
`SPX-O101`.

## Step by hand

`iter_next<T>` consumes an iterator and returns an `IterStep<T>`: `Done {}` or
`Yield { item, rest }`. Take it apart with `match own`. `item` is a Copy value
and `rest` owns the remaining iterator. Pass `rest` to the next step, or let it
go out of scope:

<!-- handbook-smoke: {"stdout":"0\n"} -->
```semaprax
module app.step;

@id("lazy.first_if_step")
fn first_if_step<T>(step: own IterStep<T>, keep: fn(T) -> bool) -> bool
{
    match own step { IterStep::Done {} => false, IterStep::Yield { item, rest } => keep(item), }
}

@id("app.main")
fn main() -> i64
{
    let input = vec_push<i64>(vec_with_capacity<i64>(1usize), 7);
    let found = first_if_step<i64>(iter_next<i64>(vec_into_iter<i64>(input)), fn(value: i64) -> bool { value > 0 });
    if found { 0 } else { 1 }
}
```

## Map, filter, and fold

There is no built-in `map`. You write these helpers once, as generic functions
over `for own`, and reuse them. Each call spells its type arguments:

<!-- handbook-smoke: {"stdout":"1\n"} -->
```semaprax
module app.pipeline;

@id("iterator.map")
fn map<T, U>(input: own Iter<T>, capacity: usize, transform: fn(T) -> U) -> Vec<U>
{
    let mut output = vec_with_capacity<U>(capacity);
    for own item in input {
        output = vec_push<U>(output, transform(item));
        0
    }
    output
}

@id("iterator.filter")
fn filter<T>(input: own Iter<T>, capacity: usize, keep: fn(T) -> bool) -> Vec<T>
{
    let mut output = vec_with_capacity<T>(capacity);
    for own item in input {
        if keep(item) { output = vec_push<T>(output, item); 0 } else { 0 }
    }
    output
}

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
    let input = vec_push<i64>(vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(3usize), -1), 2), 3);
    let mapped = map<i64, bool>(vec_into_iter<i64>(input), 3usize, fn(value: i64) -> bool { value > 0 });
    let filtered = filter<bool>(vec_into_iter<bool>(mapped), 3usize, fn(value: bool) -> bool { value });
    let count = fold<bool, usize>(vec_into_iter<bool>(filtered), 0usize, fn(count: usize, value: bool) -> usize { if value { count + 1usize } else { count } });
    if count == 2usize { 1 } else { 0 }
}
```

Elements are the eight Copy scalars and `Bytes`. Owned items, public iterator
signatures, and adapters that build another vector are outside the profile.
The lazy adapters, which do their work as the iterator is consumed, are in
`examples/lazy-iterator-adapters.spx`. Run it from a repository checkout:

```sh
semaprax run examples/lazy-iterator-adapters.spx
```

Exact rules: [While Loops v1](https://github.com/wavect/semaprax/blob/main/docs/WHILE-LOOPS-V1.md),
[Vec For Traversal v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md),
[Owning Iterators v1](https://github.com/wavect/semaprax/blob/main/docs/OWNING-ITERATORS-V1.md),
[Owning Iterator Loops v1](https://github.com/wavect/semaprax/blob/main/docs/OWNING-ITERATOR-LOOPS-V1.md),
[Generic Iterator Operations v1](https://github.com/wavect/semaprax/blob/main/docs/GENERIC-ITERATOR-OPERATIONS-V1.md),
[Lazy Iterator Adapters v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LAZY-ITERATOR-ADAPTERS-V1.md).
