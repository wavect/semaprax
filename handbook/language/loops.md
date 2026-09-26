# Loops and iterators

Three repetition forms, each with a bounded profile: `while` for scalar
state machines, `for … in` for borrowing a vector, `for own … in` for
consuming an iterator.

## while: the scalar workhorse

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

The profile, in full:

- The condition is checked before every iteration; the body's **last
  expression** is the continuation condition (a body ending in an assignment
  is `SPX-P203`).
- Bodies admit Copy-scalar operations, scalar-returning calls, and the exact
  `byte_get`/`Option<u8>` inspection profile. Constructing records/variants
  or calling aggregate-returning functions inside is `SPX-T252` — compute
  scalars in the loop, build aggregates after.
- The value of a `while` is discarded; bind results to `let mut` state.
- `bytes_zeroed` stays outside the loop; the same-owner `bytes_set`
  replacement is the one buffer write a body admits (see
  [Ownership](ownership.md)).
- Owned results like `net_recv` are not admitted in bodies (`SPX-T270`).

## for: borrow-traverse a vector

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

`for item in values` borrows each element of a simple immutable `Vec<T>`
binding (`T` a Copy scalar): length is snapshotted once, elements visit in
ascending index order, the body result is discarded, and `values` is frozen
inside. Keep the item immutable — no moving, mutating, or reassigning the
vector in the body. Computed iterables, owned elements, consuming traversal,
and `break`/`continue` are outside this form.

## for own: consume an iterator

`vec_into_iter<T>` moves a scalar vector into a non-Copy `Iter<T>`, and
`for own` drains it:

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

For manual stepping, `iter_next<T>` consumes the iterator and returns
`IterStep<T>` — match it with `match own`:

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

`Yield { item, rest }` gives a Copy `item` plus the owning `rest`: pass
`rest` to the next step or let scope cleanup settle it. Reusing a consumed
iterator is an ownership error. All eight Copy scalars are admitted; owned
items and lazy adapters beyond the bounded profile are separate work.

## Best practices

1. **Reach for `for` over `while`** when walking a vector — no index, no
   off-by-one, no bounds check to write.
2. **Consume with `for own`** when the vector is single-use input; it makes
   the transfer visible and frees the source for reuse discipline.
3. **Keep loop bodies scalar.** If a body wants to build a record, restructure:
   loop over scalars, construct once after.

Exact rules: [While Loops v1](https://github.com/wavect/semaprax/blob/main/docs/WHILE-LOOPS-V1.md),
[Bounded Vec For Traversal v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md),
[Owning Iterators v1](https://github.com/wavect/semaprax/blob/main/docs/OWNING-ITERATORS-V1.md),
[Owning Iterator Loops v1](https://github.com/wavect/semaprax/blob/main/docs/OWNING-ITERATOR-LOOPS-V1.md).
