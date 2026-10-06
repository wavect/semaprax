# Ownership, strings, bytes

After this page you can pass text and bytes between functions without
ownership errors. The whole idea fits in one rule: **passing an owned value
moves it, and a `borrow` only reads it.** Using a moved value is a compile-time
error (`SPX-O101`), never a crash.

## Why did SPX-O101 fail?

A `string` or `Bytes` value has one owner. Passing it to a function, or to
`string_concat`, moves it. The second use below fails:

```text
let s = string_concat("ab", "cd");
size(s) + size(s)        // error[SPX-O101]: use of resource `s` after ownership was moved
```

Fix it in one of three ways:

| Fix | When to use it |
| --- | --- |
| Take a view: `let v = string_as_str(s);` and pass `v` to a `borrow str` parameter. | The callee only reads. This is the usual fix. |
| Build a second value for the second call. | Each call really needs its own copy. |
| Copy bytes with `bytes_copy(bytes_as_slice(data))`. | You need a second owned `Bytes`. |

Copy scalars (`i64`, `bool`, `char`, and the rest) never move. They copy freely.

## Borrow in helpers, own in sinks

<!-- handbook-smoke: {"stdout":"banana!0\n"} -->
```semaprax
module app.bytes;

permit { process.stdout.write }

@id("bytes.count_a")
fn count_a(text: borrow str) -> usize
{
    let view = str_as_bytes(text);
    let length = byte_len(view);
    let mut index = 0usize;
    let mut hits = 0usize;
    while index < length {
        hits = match byte_get(view, index) { Option::Some { value: byte } => if byte == 97u8 { hits + 1usize } else { hits }, Option::None {} => hits, };
        index = index + 1usize;
        index < length
    }
    hits
}

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let greeting = string_concat("banana", "!");
    let borrowed = string_as_str(greeting);
    let hits = count_a(borrowed);
    let written = stdout_write(str_as_bytes(borrowed));
    if hits == 3usize && written == 7usize && string_len(greeting) == 7 { 0 } else { 1 }
}
```

`run` prints `banana!` from `stdout_write`, then `0` from `main`.

- `borrow T` reads a value without taking it. Make it the default for helpers.
- `own T` takes the value. Use it for functions that finish with the value:
  builders, transfers, destructors.
- `own` is valid for `Bytes`, `Vec`, `Box`, iterators, and
  [resources](resources.md). A plain `string` parameter moves too. Writing
  `own string` is `SPX-O002`.

## Convert text to bytes in steps

Text goes down a one-way ladder. Each step is a named call:

```text
string            owned text: "hello", string_concat(a, b), string_from_i64(n)
   | string_as_str(binding)
   v
str (borrowed)    read-only view: pass it to `borrow str` parameters
   | str_as_bytes(view)
   v
Slice<u8>         borrowed bytes: byte_len, byte_get, byte_range, stdout_write
```

Every conversion takes a **named binding**, not a literal or a call result.

| You write | Error | Write this |
| --- | --- | --- |
| `string_as_str("hi")` | `SPX-T266` | `let s = "hi"; string_as_str(s)` |
| `str_as_bytes(string_as_str(s))` | `SPX-T266` | `let v = string_as_str(s); str_as_bytes(v)` |
| `str_as_bytes(text)` with a `string` | `SPX-T263` | Take the `str` view first. |
| `f("abc")` for a `borrow str` parameter | `SPX-T205` | `let s = "abc"; f(string_as_str(s))` |
| `"a" + "b"` | `SPX-T250` | `string_concat("a", "b")` |
| `string_concat("n=", 5)` | `SPX-T205` | `string_concat("n=", string_from_i64(5))` |

Going up: `bytes_copy(view)` makes an owned `Bytes`. `array_as_slice(array)` and
`bytes_as_slice(bytes)` give a `Slice<u8>`. The type table is in
[Collections](collections.md#bytes-slices-and-strings).

## Build a byte buffer

Allocate with a literal capacity and chain `bytes_set`. Binding the result
freezes it:

<!-- handbook-smoke: {"stdout":"0\n"} -->
```semaprax
module app.buffer;

@id("app.main")
fn main() -> i64
{
    let buffer = bytes_set(bytes_set(bytes_zeroed(2usize), 0usize, 65u8), 1usize, 66u8);
    let view = bytes_as_slice(buffer);
    if byte_len(view) == 2usize { 0 } else { 1 }
}
```

To fill a buffer in a loop, allocate it outside, then assign the result back to
the same `let mut` binding. This is the only way to re-open a buffer:

<!-- handbook-smoke: {"stdout":"0\n"} -->
```semaprax
module app.buffer_loop;

@id("app.main")
fn main() -> i64
{
    let mut buffer = bytes_zeroed(3usize);
    let mut index = 0usize;
    let mut value = 65u8;
    while index < 3usize {
        buffer = bytes_set(buffer, index, value);
        index = index + 1usize;
        value = value + 1u8;
        0
    }
    let view = bytes_as_slice(buffer);
    if byte_len(view) == 3usize { 0 } else { 1 }
}
```

Limits: `bytes_zeroed` takes a `usize` literal capacity and cannot sit inside a loop
(`SPX-T267`). A literal index past the capacity is `SPX-T272`. A computed index
past the capacity fails at run time before anything is written. Do not hold a
view across the reassignment (`SPX-T265`). You cannot re-open a named buffer any
other way (`SPX-T271`).

Exact rules: [RFC 0003](https://github.com/wavect/semaprax/blob/main/docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md),
[Owned String Borrowed View v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-STRING-BORROWED-VIEW-V1.md),
[Owned Bounded Byte Buffer v1](https://github.com/wavect/semaprax/blob/main/docs/OWNED-BOUNDED-BYTE-BUFFER-V1.md).
