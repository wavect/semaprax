# Ownership, strings, bytes

Ownership is the one genuinely new idea. The rule fits in a sentence:
**an `own` parameter consumes its argument; a `borrow` parameter only reads
it.** Using a consumed value again is a compile error (`SPX-O101`), never a
runtime crash.

## Own vs borrow

```semaprax
module app.resources;

@id("resources.token")
resource Token {
    @id("resources.token.drop")
    drop trivial;
}

@id("resources.inspect")
fn inspect(token: borrow Token) -> i64
{
    1
}

@id("resources.consume")
fn consume(token: own Token) -> i64
    ensures result == 1
{
    inspect(token)
}

@id("app.main")
fn main() -> i64
{
    0
}
```

- `borrow T` is the default shape for helpers: take a borrow, return a scalar.
- `own T` is for sinks: builders, destructors, transfers into another owner.
- Copy scalars (`i64`, `bool`, …) copy freely — ownership only constrains
  owned values like `string`, `Bytes`, boxes, and resources.
- A callee that needs the value twice should take `borrow`, or the caller
  passes a fresh value per `own` call.

Resources declare how they drop: `drop trivial;` or
`drop import "host.symbol";`. Modules declaring resources can't run under
single-file `run` (`SPX-B104`) — verify them with `check` and exercise them
through a project build.

## The string → str → bytes ladder

Text flows down a one-way ladder. Each step is a deliberate, named conversion:

```text
string            owned text: "hello", string_concat(a, b)
   | string_as_str(binding)   -- argument must be a let binding, not a literal
   v
str (borrowed)    read-only view: pass to borrow str params
   | str_as_bytes(view)
   v
Slice<u8>         borrowed bytes: byte_len, byte_get, byte_range, stdout_write
```

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

The three mistakes everyone makes once:

| Mistake | Code | Fix |
| --- | --- | --- |
| `+` on strings | `SPX-T250` | `string_concat(a, b)` (consumes both) |
| Passing a `string` where `borrow str` is expected | `SPX-T205` | Bind it, then `string_as_str(binding)` |
| `string_as_str("literal")` | `SPX-T266` | `let s = "literal";` first, then borrow the binding |

Other rungs: `array_as_slice(binding)` turns `[u8; N]` into a view;
`bytes_as_slice(binding)` does the same for owned `Bytes`;
`bytes_copy(view)` goes back up to an owned `Bytes`.

## Owned byte buffers

Build a bounded buffer as one write-once chain, then freeze it by binding:

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

`bytes_zeroed` needs a `usize` **literal** capacity. To fill a buffer in a
loop, allocate outside and re-assign the same `let mut` binding inside —
that same-owner replacement is the only re-opening the language admits:

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

Don't hold a borrowed view across the replacement, and don't call
`bytes_zeroed` inside the loop body. A literal index past capacity is
`SPX-T272`; a computed out-of-range index fails at runtime before writing
anything.

## Best practices

1. **Borrow down, own up.** Helpers take `borrow`; only sinks and builders
   take `own`.
2. **Convert at the boundary.** Turn owned values into views at function
   entries, work with views inside, build owned values at exits.
3. **Bind before borrowing.** `string_as_str`, `array_as_slice`, and
   `bytes_as_slice` all take plain `let` bindings — never literals or calls.

Exact rules: [RFC 0003](https://github.com/wavect/semaprax/blob/main/docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md).
