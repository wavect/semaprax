# Collections: Vec, Box, arrays, bytes

After this page you can pick the right container and use it: `Vec` for lists
of scalars, `Box` for one boxed scalar, `[u8; N]` for a small fixed byte
block, and `Bytes` for a byte buffer you build.

| You have | Use |
| --- | --- |
| A list of numbers, bools, or chars | `Vec<T>` |
| One scalar that must live behind an owner | `Box<T>` |
| A few known bytes | `[u8; N]` |
| Bytes you fill in | `Bytes` |
| Text | `string`, see [Ownership](ownership.md) |

Every call spells its element type, such as `vec_push<i64>`. `T` is one of the
eight Copy scalars: `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, `bool`.
All capacities are bounded.

## Build and read a vector

<!-- handbook-smoke: {"stdout":"5\n"} -->
```semaprax
module app.stats;

@id("stats.sum")
fn sum(readings: borrow Vec<i64>) -> i64
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
    let mut building = vec_with_capacity<i64>(3usize);
    building = vec_push<i64>(building, 2);
    building = vec_push<i64>(building, 3);
    building = vec_push<i64>(building, 0);
    let readings = building;
    sum(readings)
}
```

Mutators take the vector and return the next one, so you always assign the
result back: `building = vec_push<i64>(building, 2);`. Build with `let mut`,
then move the finished vector into an immutable binding to traverse it.

| Call | Result |
| --- | --- |
| `vec_with_capacity<T>(n)` | An empty vector with room for `n` items. |
| `vec_push<T>(v, x)` | The vector with `x` added. |
| `vec_set<T>(v, i, x)` | The vector with item `i` replaced. |
| `vec_clear<T>(v)` | The emptied vector. |
| `vec_reserve_exact<T>(v, extra)` | The vector with room for `extra` more items. |
| `vec_len<T>(v)` | The length as a `usize`. Reads only. |
| `vec_get<T>(v, i)` | A copy of item `i`. Reads only. |
| `vec_into_iter<T>(v)` | An `Iter<T>` that consumes the vector, see [Loops](loops.md). |

Pushing past the capacity stops the program (`semaprax.vec.v1` status 1, "vec_push
beyond capacity"), and so does an index past the length (status 2, "vector
index out of bounds"). Reserve enough room first. A `for` loop avoids the
index bookkeeping: see [Loops](loops.md#visit-a-vector-with-for).

In a Project, `Vec` needs the `owned-data-api.v1` profile and
`std.collections = "^0.1.0"`. See [Profiles](../projects/profiles.md).

## Keep a scalar in a Box

<!-- handbook-smoke: {"stdout":"7\n"} -->
```semaprax
module app.boxed;

@id("app.main")
fn main() -> i64
{
    let boxed = box_new<i64>(7);
    let peek = box_get<i64>(boxed);
    box_into_inner<i64>(boxed)
}
```

`box_new<T>` makes a uniquely owned `Box<T>`. `box_get<T>` copies the value out
through a borrow. `box_into_inner<T>` consumes the box. These three names
select the compiler's box. A record you declare yourself as `record Box<T>` is
an ordinary inline record. The box holds one Copy scalar only, with no shared
ownership. See
[Owned Bounded Box v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-BOX-V1.md).

## Use a fixed byte array

`[97u8, 98u8]` has type `[u8; 2]`. An array literal holds bytes only. A list of
`i64` is a `Vec<i64>`, not `[1, 2, 3]` (`SPX-T262`). There is no `a[0]`
(`SPX-P106`). Borrow a view and read with `byte_get`, which returns an
`Option<u8>` so you handle the missing case:

<!-- handbook-smoke: {"stdout":"0\n"} -->
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

## Bytes, slices, and strings

| Type | Owns its data? | Get one from | Turn it into |
| --- | --- | --- | --- |
| `string` | Yes | A literal, `string_concat`, `string_from_*` | `string_as_str(binding)` gives `str` |
| `str` | No | `string_as_str`, `arg_utf8` | `str_as_bytes(view)` gives `Slice<u8>` |
| `Bytes` | Yes | `bytes_zeroed` + `bytes_set`, `bytes_copy`, `stdin_read` | `bytes_as_slice(binding)` gives `Slice<u8>` |
| `Slice<u8>` | No | `str_as_bytes`, `array_as_slice`, `bytes_as_slice` | `byte_len`, `byte_get`, `byte_range`, `stdout_write` |

A slice is a view. It borrows and never owns. Each conversion takes a named
`let` binding, not a literal or a call result.

## Work with text

| Call | Does |
| --- | --- |
| `string_concat(a, b)` | A new string. Consumes both. |
| `string_len(s)`, `string_len_chars(s)` | Length in bytes, in Unicode scalars. |
| `string_is_empty(s)` | `true` when the length is 0. |
| `string_starts_with(s, p)`, `string_contains(s, p)` | Search. |
| `string_from_char(c)`, `string_from_i64(n)`, `string_from_usize(n)` | Render a value as text. |
| `str_len_bytes(v)`, `str_is_empty(v)`, `str_starts_with(v, p)`, `str_contains(v, p)` | The same reads on a borrowed `str`. |

`==` compares string contents. These names are reserved: declaring your own
`string_len` is `SPX-S113`. The full signatures are in
[Built-in functions](../reference/builtins.md).

Exact rules: [Owned Bounded Vec v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-VEC-V1.md),
[v2](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-VEC-V2.md),
[String Operations v1](https://github.com/wavect/semaprax/blob/main/docs/STRING-OPS-V1.md),
[Portable Indexed Byte Data v1](https://github.com/wavect/semaprax/blob/main/docs/PORTABLE-INDEXED-BYTE-DATA-V1.md).
