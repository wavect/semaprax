# Collections: Vec, Box, arrays, bytes

Four aggregate shapes, each with one ownership story. All type arguments are
explicit; all capacities are bounded.

## Vec: owned growable vectors of Copy scalars

```semaprax
module app.stats;

@id("stats.sum")
fn sum(readings: own Vec<i64>) -> i64
{
    let length = vec_len<i64>(readings);
    let mut position = 0usize;
    let mut total = 0;
    while position < length {
        total = total + vec_get<i64>(readings, position);
        position = position + 1usize;
        position < length
    }
    total
}

@id("app.main")
fn main() -> i64
{
    let readings = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 20), 22);
    sum(readings)
}
```

The operation set (all with explicit `<T>`, `T` an admitted Copy scalar):

| Operation | Shape |
| --- | --- |
| `vec_with_capacity<T>(n)` | Fresh vector with capacity `n` |
| `vec_push<T>(v, x)` | Returns the vector with `x` appended — **thread the owner** |
| `vec_len<T>(v)` | `usize` length (borrows) |
| `vec_get<T>(v, i)` | Copy element at `i` (borrows) |
| `vec_clear<T>(v)` | Returns the emptied vector |
| `vec_into_iter<T>(v)` | Moves the vector into a consuming `Iter<T>` |

Mutators transfer and return the owner: always write
`let next = vec_push<T>(current, x);` — there is no in-place mutation. A
project using vectors needs profile `owned-data-api.v1` and
`std.collections = "^0.1.0"`; prefer the authenticated `std.collections`
aliases there. Traverse with `for`/`for own` (see [Loops](loops.md)).

## Box: one owned scalar behind indirection

```semaprax
module app.boxed;

@id("app.main")
fn main() -> i64
{
    box_into_inner<i64>(box_new<i64>(41)) + 1
}
```

- `box_new<T>(value)` allocates a uniquely owned `Box<T>` (explicit admitted
  Copy scalar only).
- `box_get<T>(b)` reads through a borrow; `box_into_inner<T>(b)` consumes
  the box and returns the value.
- These three names select the compiler-owned allocation. An authored
  `record Box<T>` without them is still an inline record — no indirection,
  no heap. The bounded profile has no owned payload, region, arena, or
  shared-ownership surface.

## Arrays: fixed inline bytes

`[u8; N]` is a fixed-size inline value: `[97u8, 98u8]` has type `[u8; 2]`.
There is no indexing syntax — borrow a view and use byte operations:

```semaprax
module app.lookup;

@id("app.main")
fn main() -> i64
{
    let sample = [97u8, 98u8];
    let view = array_as_slice(sample);
    match byte_get(view, 0usize) { Option::Some { value: b } => if b == 97u8 { 0 } else { 1 }, Option::None {} => 2, }
}
```

`a[0]` doesn't exist (`SPX-P106`): `byte_get(array_as_slice(a), 0usize)`
returns `Option<u8>` instead, forcing you to handle the missing case.

## Bytes vs slices vs strings

| Type | Owned? | From | To |
| --- | --- | --- | --- |
| `string` | Yes | Literals, `string_concat`, `string_from_*` | `string_as_str(binding)` → `str` |
| `str` (borrowed) | No | `string_as_str`, `arg_utf8` | `str_as_bytes` → `Slice<u8>` |
| `Bytes` | Yes | `bytes_zeroed`+`bytes_set`, `bytes_copy`, `stdin_read` | `bytes_as_slice(binding)` → `Slice<u8>` |
| `Slice<u8>` (borrowed) | No | `str_as_bytes`, `array_as_slice`, `bytes_as_slice` | `byte_len`, `byte_get`, `byte_range`, `*_write` |

Slices are views: they borrow, never own. Every conversion takes a plain
`let` binding — never a literal or call result. See [Ownership](ownership.md)
for the full ladder and buffer rules.

## Best practices

1. **Vec for lists, arrays for small fixed blobs, Bytes for built buffers.**
   Don't grow a `Bytes` element-by-element when a `Vec` threads ownership
   more clearly.
2. **Borrow to inspect, own to transform.** `vec_len`/`vec_get`/`byte_get`
   borrow; `vec_push`/`bytes_set` consume and return.
3. **Prefer `for` traversal over `vec_get` loops** — same result, no index
   bookkeeping.

Exact rules: [Owned Bounded Vec v1/v2](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-VEC-V1.md),
[Owned Bounded Box v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-BOX-V1.md),
[Portable Indexed Byte Data v1](https://github.com/wavect/semaprax/blob/main/docs/PORTABLE-INDEXED-BYTE-DATA-V1.md).
