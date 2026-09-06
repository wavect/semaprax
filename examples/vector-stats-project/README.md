# Vector stats project

The first example project built on [Owned Bounded Vec v1](../../docs/OWNED-BOUNDED-VEC-V1.md).
It accumulates a **variable** number of scalar readings into one owned
`Vec<i64>` and then filters them, with no allocation the compiler cannot bound
and no ambient authority.

```sh
semaprax check examples/vector-stats-project/semaprax.toml
semaprax test  examples/vector-stats-project/semaprax.toml
semaprax run   examples/vector-stats-project/semaprax.toml
```

## What it demonstrates

- **A loop-carried fill.** `vector-stats.alert-total` initializes one mutable
  vector outside a bounded `while`, and the body assigns that same binding
  exactly once from `vec_push<i64>(readings, ...)`. The number of pushes is the
  runtime argument `count`, not a fixed unrolled sequence: the entry reaches
  the same function with `reading_count(9)` = nine elements and
  `reading_count(13)` = zero, and the conformance module also drives it at 0, 3
  and 12.
- **A filtered walk.** A second bounded `while` reads the accumulated vector
  back through `vec_len<i64>` and `vec_get<i64>` and sums only the readings the
  predicate keeps, so the aggregate depends on both the accumulated length and
  the threshold. Over the same nine readings `alert_total(9, 0)` is `431`,
  `alert_total(9, 50)` is `310`, and `alert_total(9, 97)` is `0`.
- **Cross-module composition.** `vector_stats.readings` owns the scalar sample
  and predicate functions; `vector_stats.collect` imports both by stable `@id`
  and owns the vector; `vector_stats.app` imports the accumulator and supplies
  the element count and threshold; `vector_stats.tests` drives the accumulator
  and the helpers independently.

## Limits this example is shaped by

The vector never crosses a module boundary. `Vec<T>` is a compiler-owned value
type, and only the authenticated transparent `std.collections` aliases may
carry one through a signature, so an ordinary authored function keeps it local
and the module split is drawn around the scalar helpers instead.

The capacity is the literal `12usize` and `alert_total` carries
`requires count >= 0 && count <= 12`: there is no growth beyond the initial
capacity, and a push past it selects sticky `semaprax.vec.v1` code 1.

The two selected `web` exports are the entry module's own scalars, and that is
not a free choice. The public scalar adapter refuses a function that binds a
non-value local (`SPX-W115`), exactly as it refuses an owned byte buffer, and
the adapter closure is drawn per module: selecting a root from
`vector_stats.readings` would pull in `vector_stats.collect`, which imports
that module and holds the vector. Exporting from the entry module keeps the
adapter closure vector-free.
