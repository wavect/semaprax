# Agent quick reference

Status: public beta reference card. Every `semaprax` code block on this
page is a complete module that `tests/documentation.rs` checks against the
compiler: blocks without an `expect` marker must verify without diagnostics and
already be canonical; blocks with one must produce exactly that diagnostic code.

Audience: coding agents and their operators writing SEMAPRAX programs with a
bounded context window.

Use this card when writing a `.spx` file. It shows accepted syntax, common
diagnostics, and the shortest useful check loop. For explanations, read the
[language tour](LANGUAGE-TOUR.md). [RFC 0001](RFC-0001.md) defines the rules;
the [completion matrix](COMPLETION-MATRIX.md) says what is implemented.

The installed compiler prints this page verbatim with `semaprax help language`,
so it is available without the source checkout.

## Spend tokens on source, not on dumps

1. Write the file, then run `semaprax fmt <file> && semaprax run <file>` as
   one command: `run` verifies first and reports the same diagnostics as
   `check`. Fix the first diagnostic at its reported line and column; its
   `help:` line is usually the fix. Plain output is about a third smaller than
   `--json`, which is for tools. Tests should match the stable `SPX-…` code,
   not message wording.
2. Read a small `.spx` file directly. Use `semaprax graph <file>` only when a
   tool needs the whole expression tree or cleanup plan. On the calculator
   example, the graph is about 40 times larger than source.
3. For one declaration, ask `semaprax context <file> <stable-id> --depth 1
   --filters contracts,ownership --max-bytes 4096`. Check `truncation` before
   relying on the result. [Agent Context v2](AGENT-CONTEXT-V2.md) defines it.
4. In a Project, first locate a declaration with `semaprax query
   <project-dir> --id <stable-id>`; use `--calls <stable-id>` for callers. Then
   request a bounded neighborhood with `semaprax context <project-dir>
   <stable-id> --direction both --depth 1 --max-bytes 2048 --max-nodes 16`.
   Use `--json` only when you need exact revision and relationship fields.
5. Ask for narrow help: `semaprax help <command>`, `semaprax help language
   topics`, `semaprax help language <topic>`, `semaprax help diagnostic
   <SPX-code>`, or `semaprax help shapes <kind|stable-id|path#stable-id>`.
   `semaprax help all` and the full language card are for broad questions.

`fmt` and single-file `patch` preserve `//` comments. Workspace transactions
do not promise that, so put durable intent in stable `@id` names, contracts,
and tests.

## A complete file

```semaprax
module app.hello;

@id("app.main")
fn main() -> i64
{
    42
}
```

- One `module dotted.name;` per file, first.
- Give every declaration an `@id("dotted.stable.name")`. Without it the
  compiler warns `SPX-S103`; a function rename then changes its identity.
- The entry point is exactly `fn main() -> i64`. There is no other signature.
- A function body has zero or more statements (`let`, assignment, `if`,
  `while`, `for`, `unsafe`) and one final expression. That expression supplies the block's
  value. User code has no `return`, expression statement, tuple, or unit
  value.
- Source blocks, delimiters, unary chains, and expression trees may nest at
  most 128 levels; `SPX-P207` asks you to extract a named helper.
- Canonical layout puts the function body's `{` on its own line and each
  statement on its own line; `if`, `match`, and record literals stay on one
  line. Let `fmt` do it.

For exact unsigned integers beyond `i64`, depend on `std.int.decimal = "^0.1.0"`
with the `owned-data-api.v1` Project profile. Its source-authored `add`,
`subtract`, `divide`, and `compare` operate on canonical decimal strings;
`digits` admits untrusted ASCII input and `canonicalize` strips leading zeros.
Subtract requires a nonnegative result; divide requires a nonzero divisor.
Malformed inputs fail checked contracts. This partial alloc package lists
interpreter and native C11 targets; it claims no public BigInt or Core Wasm ABI.
See [the catalog](STANDARD-LIBRARY-CATALOG.md#stdintdecimal) for exact stable IDs.

## Scalars and literals

- `i64`: `42`, `-1`; default integer, checked overflow.
- `i32`: `42i32`; suffix required, no implicit widening.
- `u8`: `255u8`; byte value.
- `usize`: `3usize`; lengths and indices.
- `f64`, `f32`: `1.5`, `1.5f32`.
- `bool`: `true`, `false`; `&&`, `||`, `!`.
- `char`: `'a'`, `'\n'`, `'\u{2603}'`.
- `string`: `"text"`; owned UTF-8; content equality and UTF-8 byte ordering.
- `str`: no literal; borrowed by `borrow str` or `string_as_str(binding)`.
- `[u8; N]`: `[97u8, 98u8]`; fixed; `array_as_slice(binding)` gives
  `Slice<u8>`.
- `Bytes`, `Slice<u8>`: no literal; owned bytes and borrowed byte view.

`n: usize` needs `n < 5usize` (`SPX-T208`). Join strings
with `string_concat`. No `as`; use `f64_from_i64`, `i64_from_f64` (truncates),
`usize_from_i64` or `i64_from_usize`.

## Control flow, mutation, contracts, effects

Exact integer widening uses `i64_from_u8`, `i64_from_i32`, or `usize_from_u8`.
Scalar conversions fail out of range or on NaN with `semaprax.convert.v1`.
Exact integer widening and checked integer conversions run on the interpreter,
native, and Core Wasm; float conversions and `string_from_str` retain
`SPX-W116`. `i64_from_u8(byte)` is allocation-free and infallible. Integer `%`
supports i64, i32, u8, and usize: zero divisors fail; signed MIN % -1 fails
with remainder overflow.

```semaprax
module app.convert;

@id("app.main")
fn main() -> i64
{
    let total = 10;
    let count = 4usize;
    let average = f64_from_i64(total) / f64_from_i64(i64_from_usize(count));
    i64_from_f64(average * 10.0)
}
```

`semaprax run convert.spx` prints `25`.

```semaprax
module app.flow;

permit { clock.read }

@id("flow.digit_sum")
fn digit_sum(value: i64) -> i64
    requires value >= 0
    ensures result >= 0
{
    let mut remaining = value;
    let mut total = 0;
    while remaining > 0 {
        total = total + remaining % 10;
        remaining = remaining / 10;
        remaining > 0
    }
    total
}

@id("flow.classify")
fn classify(value: i64) -> i64
{
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, _ => 1, }
}

@id("flow.tick")
fn tick(value: i64) -> i64
    uses { clock.read }
{
    value + 1
}

@id("app.main")
fn main() -> i64
    uses { clock.read }
{
    let mut acc = digit_sum(98765);
    acc = acc + classify(-2);
    if acc > 0 { tick(acc) } else { 0 - acc }
}
```

- As a value, `if` always has `else`; `else if` chains are fine (`fmt`
  writes them as `else { if … }`). As a statement, `if c { x = x + 1; }`
  needs no `else` and no branch value, in loop bodies too; `fmt` keeps that
  statement spelling and any explicit `else`. The block still ends with its
  own final expression.
- A `while` condition must be `bool` and is checked before every iteration.
  Its body still needs a final expression, but that value is discarded; the
  condition controls repetition. While bodies admit
  Copy-scalar operations, user calls with declared read-only input effects
  taking Copy scalars or flat Copy variants, borrowed byte slices or named `str` views, or consumed
  strings and returning a scalar, flat Copy variant or string, matches over Copy
  scalars or variants with only Copy scalar payloads,
  and string literals and `string_*` calls (each iteration releases its own
  strings). Match arms may yield strings or flat Copy variants. Concrete variants with only Copy
  scalar payloads may be constructed there, including direct match scrutinees;
  record/non-Copy variant construction, other aggregate-returning calls, and
  surrounding ownership changes are `SPX-T252`. Conditions settle temporary
  Strings before each Boolean decision ([condition lifetime](STRING-CONDITION-LIFETIMES-V1.md)). Copy variant guards admit ordinary checked bool calls/blocks and
  case/wildcard/or patterns, require exhaustive unguarded fallback, and cannot
  consume an outer owner ([guard profile](GENERAL-LOOP-MATCH-V1.md)).
- Bindings are immutable unless `let mut`. Assignment is a statement:
  `x = x + 1;` or `point.x = 5;`. Parameters are immutable. A `let mut`
  string can be replaced by a same-typed RHS; the completed RHS becomes its
  new owner after the old owner is released ([replacement](STRING-REPLACEMENT-V1.md)).
- Contracts are `requires`/`ensures` lines between the signature and the body;
  `result` names the return value. They are checked at run time.
- Effects: the module lists `permit { … }`, and every function that performs
  or calls into an effect declares `uses { … }`. Missing `permit` is
  `SPX-E101`; a missing `uses` is `SPX-E102`.
- `match` on scalars needs a final catch-all arm (`_` or a binding) without a
  guard, else `SPX-T257`.
- Match arms cannot yield nominal aggregates. `SPX-T258` means to use `if` to
  construct the record/variant, or extract scalars first. Arms and `if`
  branches may yield `string`: `match level { 0 => "low", _ => "high", }`,
  `let text = if ok { a } else { b };`, or an `if` passed straight to
  `string_concat`. Only the selected branch runs; the other's text is never
  built.

## Records, variants, classes

```semaprax
module app.data;

@id("data.point")
record Point {
    @id("data.point.x")
    x: i64,
    @id("data.point.y")
    y: i64,
}

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

@id("data.area")
fn area(shape: Shape) -> i64
{
    match shape { Shape::Dot {} => 0, Shape::Box { width: w, height: h } => w * h, }
}

@id("data.first_positive")
fn first_positive(left: i64, right: i64) -> Option<i64>
{
    if left > 0 { Option<i64>::Some { value: left } } else { if right > 0 { Option<i64>::Some { value: right } } else { Option<i64>::None {} } }
}

@id("data.checked_div")
fn checked_div(left: i64, right: i64) -> Result<i64, i64>
{
    if right == 0 { Result<i64, i64>::Err { error: 1 } } else { Result<i64, i64>::Ok { value: left / right } }
}

@id("app.main")
fn main() -> i64
{
    let mut origin = Point { x: 1, y: 2 };
    origin.x = origin.x + 1;
    let moved = origin with { y: 10 };
    let shape = Shape::Box { width: moved.x, height: moved.y };
    let counter = Counter { value: 1 };
    let picked = match first_positive(0, 4) { Option::Some { value: v } => v, Option::None {} => 0, };
    let divided = match checked_div(8, 2) { Result::Ok { value: v } => v, Result::Err { error: code } => code, };
    area(shape) + counter.bumped(1).value + picked + divided
}
```

- A declaration's last field or case may omit its `,`; `fmt` writes it.
- Give every field and case its own `@id`. Cases without payload are
  written `Name,` in the declaration and `Type::Name {}` everywhere else;
  the `{}` may be omitted (`Type::Name`) and `fmt` writes it back.
- Constructing a generic variant spells the type arguments:
  `Option<i64>::Some { value: v }`. Matching one does not:
  `Option::Some { value: v } => …`. Neither side accepts `Some(v)`. Generic
  functions are called with explicit type arguments: `identity<i64>(4)`.
- `record … with { field: value }` is immutable update. Record construction
  must name every field (`SPX-T213`).
- Monomorphic acyclic records may own String/Bytes and Copy scalar fields,
  including nested records. Pass with `own` or `borrow`; results own their
  leaves. Generic, resource/view/class and invariant-bearing String records
  stay outside the executable profile (`SPX-T309`);
  [String records](OWNED-STRING-RECORDS-V1.md) owns the exact shapes.
- Classes hold fields and `fn name(self: Class, …)` methods, called as
  `value.method(args)`. `class Dog : Animal` inherits; `super.method()`
  dispatches to the parent. Records have no methods.

```semaprax
module app.rules;

@id("rules.status")
variant Status {
    @id("rules.status.todo")
    Todo,
    @id("rules.status.doing")
    Doing,
    @id("rules.status.done")
    Done,
}

@id("rules.seats")
record Seats {
    @id("rules.seats.limit")
    limit: i64,
    @id("rules.seats.used")
    used: i64,
}
    requires limit >= 1
    requires used <= limit

@id("rules.open")
fn open(status: Status) -> bool
{
    match status { Status::Todo {} | Status::Doing {} => true, Status::Done {} => false, }
}

@id("app.main")
fn main() -> i64
{
    let mut seats = Seats { limit: 3, used: 1 };
    seats.used = 2;
    let status = Status::Doing {};
    if (open(status) && status != Status::Done {}) { seats.used } else { 0 }
}
```

- `==` and `!=` compare two values of one non-generic variant whose cases all
  carry no payload: `status == Status::Done {}`. A variant with a payload or
  type arguments stays `SPX-T207`; test its case with `match`. A condition or
  contract ending in `Type::Case {}` is written in parentheses, as above.
- `A {} | B {} => …` joins payload-free cases of the scrutinee's variant in
  one arm of a plain `match`; each alternative counts for exhaustiveness. No
  guard and no payload case in such an arm (`SPX-T254`, `SPX-M105`).
- `requires` lines after a record's `}` are invariants over its fields by bare
  name. Every literal, `with` update, and field assignment re-checks them; a
  false one is the same contract failure as a function `requires`. Each must
  be `bool` and effect-free (`SPX-C101`, `SPX-C102`); generic records take
  none (`SPX-C103`). Executable owned records refuse invariants (`SPX-C104`);
  direct string fields retain the graph/webapp-only exception.

## Ownership and resources

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

- An `own T` parameter consumes its argument, so using it again is `SPX-O101`.
  A `borrow T` parameter reads it. Resources declare `drop trivial;` or
  `drop import "host.symbol";`.
- Single-file `semaprax run` rejects modules declaring resources with
  `SPX-B104`. Verify them with `check`, exercise them through a
  project's native or Wasm build (or the explicit `run <file> --native` lane),
  and keep interpreter-run examples free of `resource` declarations.

## Session protocols

```semaprax
module app.checkout;

@id("checkout.session")
session protocol "checkout-v1" {
    states { Idle, Open, Committed, Failed }
    initial Idle;
    terminal Committed cleanup {}
    terminal Failed cleanup {}
    on Idle begin: send BeginRequest via "checkout.begin" -> Open;
    on Idle abort: fail Unit -> Failed;
    on Open commit: send CommitRequest via "checkout.commit" -> choice { committed: Committed, refused: Failed };
    on Open lost: fail Unit -> Failed;
}

@id("checkout.begin")
fn begin() -> i64
{
    1
}

@id("checkout.commit")
fn commit() -> i64
{
    2
}

@id("app.main")
fn main() -> i64
{
    0
}
```

- A `session protocol` declares a named state machine: `states`, `initial`,
  zero or more `terminal S cleanup { op, ... }` entries (a terminal state's
  ordered cleanup inventory), and `on <state> <label>: <kind> <Payload>
  [requires capability cap.name] [consumes resource] [via "<function-id>"]
  -> <state> | choice { label: state, ... };` transitions. `kind` is one of
  `send`, `receive`, `call`, `return`, `cancel`, `timeout`, `fail`.
- It is checked and erased: `SPX-K1xx` verifies the declared graph (unknown
  state, duplicate label, a one-branch or duplicate-labeled choice, a
  terminal with an outgoing transition, a non-terminal dead end, a
  non-terminal state with no `cancel`/`timeout`/`fail` escape, a terminal
  missing its cleanup entry) and its bounded reachability, then the
  declaration lowers to nothing on either backend and grants no authority.
  Every projected fact carries `"authority":"none"`.
- `via` binds a transition to an ordinary monomorphic function of the same
  module by its `@id`, checked against that function's own retained HIR
  (`SPX-K104`). `requires capability` is ordering metadata, not a grant: with
  a `via`, the named capability must already be one of that function's own
  declared `uses { ... }` effects (`SPX-K105`); the ordinary effect checks
  stay authoritative regardless of what the protocol declares. Without a
  `via`, the capability is realized outside checked source and every
  projection labels it `"capability_binding":"unattributed"`.
- Declared protocols project into the per-source graph (`semaprax.graph.v48`,
  selected only for a declaring program), the Workspace and Package Semantic
  Graphs (`.v2`, selected only for a declaring workspace or package),
  `context --filters session_protocol`, Architecture Claims
  (`protocol_realizers_bound`), Assurance Manifest v1, `semaprax doc`, and
  `semaprax query --kind session_protocol`. See [Session/protocol types
  v1](SESSION-PROTOCOL-TYPES-V1.md) for the full model, including what a
  declaration explicitly does not claim.

## Strings and bytes

`text: string` already transfers ownership. Do not write `own string`
(`SPX-O002`). A helper that only reads text takes `text: borrow str`; pass
`string_as_str(text)` before any consuming call.

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

- Literals and `string_concat` results are owned. `string_as_str(binding)`
  borrows a plain `let` binding; literals and calls are `SPX-T266`. Pass the
  view to `borrow str` parameters or `str_as_bytes`.
- Byte functions take `borrow Slice<u8>`. Get one from `str_as_bytes(view)`,
  `array_as_slice(array_binding)`, or `bytes_as_slice(bytes_binding)`.
- Build a bounded byte buffer in one write-once expression: `bytes_zeroed`
  requires a literal `usize` capacity; each `bytes_set` takes the prior link,
  a `usize` index expression, and a byte. Binding freezes it for borrowed reads.
  Re-opening a named binding is `SPX-T271`; a literal index >= capacity is
  `SPX-T272`. A computed out-of-bounds index fails before writing with
  `semaprax.byte-buffer.v1` code 1.
  [Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) owns the rule.
- One form re-opens a frozen buffer: the same-owner replacement
  `buffer = bytes_set(buffer, index, value)`, where the assignment target and
  the `buffer` operand are the same `let mut` binding. That is also the only
  `bytes_set` a bounded `while` admits. Allocate before the loop:
  `bytes_zeroed` inside is `SPX-T267`. A
  borrowed view may not be live across the replacement (`SPX-T265`).

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

- `stdout_write(slice)` needs both `permit { process.stdout.write }` and
  `uses { process.stdout.write }` and returns the `usize` byte count.
- Single-file `run` evaluates `app.main`, or `fn main` with another `@id`,
  in the bounded interpreter. Options: `--json`, `--max-steps`, `--max-bytes`;
  `--native` selects generated C11. Exact `process.stdout.write` authority
  selects the bounded stdout transcript interpreter: the example prints
  `banana!0`. Permitting `process.args.read`, `fs.read`, or
  `process.stderr.write` selects command-line behavior (below). `stdin_read`
  needs native project profile `useful-data-command.v1`.
- `net_connect`, `net_send`, `net_recv`, `net_stream_stdout`, `net_wait`, and
  `net_close` are the effect-gated TCP client operations of
  [Bounded Language Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md); they
  need `permit`/`uses` of `network.connect`, `network.read`, and
  `network.write`, execute only through an injected provider, and `net_recv`
  (an owned result) is not admitted inside `while` bodies (`SPX-T270`).
- `file_read` and `file_write_new` use explicit `fs.read` / `fs.write` effects
  and an injected [Filesystem I/O v1](FILESYSTEM-IO-V1.md) provider. Paths are
  bounded relative byte prefixes; writes create new files and never overwrite.
  [Filesystem I/O v2](FILESYSTEM-IO-V2.md) adds stat, sorted immediate-name
  listing, directory creation, removal, and atomic replacement. Only stat/list
  accept an empty path for the injected root. `std.fs` composes typed Path,
  FileInfo, and Reader/Writer values through these boundaries.
- Interpreter and native calls have a 256-frame recursion bound; exceeding it
  reports runtime capacity, not a language status or process signal. Raw
  WebAssembly uses its engine's stack-limit trap.

## Compiler-owned functions

| Function | Signature |
| --- | --- |
| `string_len`, `string_len_chars` | `(s: string) -> i64` bytes / scalars |
| `string_is_empty` | `(s: string) -> bool` |
| `string_concat` | `(a: string, b: string) -> string` consumes both |
| `string_starts_with`, `string_contains` | `(s: string, other: string) -> bool` |
| `string_from_char` | `(c: char) -> string` |
| `string_from_i64` | `(value: i64) -> string` canonical decimal |
| `string_from_usize` | `(value: usize) -> string` canonical decimal |
| `string_slice` | `(s: string, start: i64, end: i64) -> string` UTF-8 byte boundaries |
| `string_find` | `(s: string, needle: string, from: i64) -> i64` first byte offset >= `from`, or `-1` |
| `string_to_i64` | `(s: string) -> Option<i64>` optional `-`, then digits only |
| `string_trim` | `(s: string) -> string` trims ASCII whitespace |
| `string_byte_at` | `(s: string, index: i64) -> i64` byte `0..=255` |
| `file_read_text` | `(path: borrow str) -> string` whole UTF-8 file, at most 64 KiB; needs `fs.read` |
| `string_compare` | `(a: string, b: string) -> i64` `-1`/`0`/`1` in bytewise order |
| `map_*`, `set_*` | Owned `Map<K,V>` / `Set<K>`; see String-keyed maps |
| `string_as_str` | `(binding: string) -> borrow str` |
| `str_len_bytes` | `(s: borrow str) -> i64` |
| `str_is_empty` | `(s: borrow str) -> bool` |
| `str_starts_with`, `str_contains` | `(s: borrow str, other: borrow str) -> bool` |
| `str_as_bytes` | `(s: borrow str) -> Slice<u8>` |
| `byte_len` | `(v: borrow Slice<u8>) -> usize` |
| `byte_get` | `(v: borrow Slice<u8>, i: usize) -> Option<u8>` |
| `str_byte_at` | `(s: borrow str, i: usize) -> Option<u8>` |
| `byte_range` | `(v: borrow Slice<u8>, start: usize, end: usize) -> Slice<u8>` |
| `bytes_copy` | `(v: borrow Slice<u8>) -> Bytes` |
| `bytes_zeroed` | `(count: usize) -> Bytes` literal capacity |
| `bytes_set` | `(b: own Bytes, i: usize, v: u8) -> Bytes` write-once chain |
| `bytes_as_slice` | `(b: borrow Bytes) -> Slice<u8>` |
| `array_as_slice` | `(a: borrow [u8; N]) -> Slice<u8>` |
| `stdout_write`, `stderr_write` | `(v: borrow Slice<u8>) -> usize` |
| `args_len` | `() -> usize` |
| `arg_utf8` | `(i: usize) -> borrow str` |
| `stdin_read` | `() -> own Bytes` |
| `file_read` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` |
| `file_write_new` | `(path: borrow Slice<u8>, length: usize, data: borrow Slice<u8>, data_length: usize) -> usize` |
| `file_stat`, `file_create_dir`, `file_remove` | `(path: borrow Slice<u8>, length: usize) -> usize` |
| `file_list` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` |
| `file_write_atomic` | `(path: borrow Slice<u8>, length: usize, data: borrow Slice<u8>, data_length: usize) -> usize` |
| `box_new<T>` | `(value: T) -> Box<T>` for an explicit admitted Copy scalar |
| `box_get<T>` | `(value: borrow Box<T>) -> T` Copy read |
| `box_into_inner<T>` | `(value: own Box<T>) -> T` consumes |

Redefining reserved `string_len` is `SPX-S113`.

Boxes allocate uniquely; authored `record Box<T>` stays inline. Owned payloads,
public generic ABI, regions, arenas and shared ownership remain outside
[Box v1](OWNED-BOUNDED-BOX-V1.md).

To print an integer, render it, borrow the string, and write its bytes:

```semaprax
module app.print_count;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let count = 42usize;
    let text = string_from_usize(count);
    let view = string_as_str(text);
    let written = stdout_write(str_as_bytes(view));
    if written == 2usize { 0 } else { 1 }
}
```

`semaprax run count.spx` prints `42`; signed values use `string_from_i64`.

[Whole `let mut` String replacement](STRING-REPLACEMENT-V1.md) accepts
literals, owning names, blocks, helpers and branches; old owners settle after
RHS success. Owning names move; active views prevent replacement.

Conditions inspect named owners via `string_len`, `string_is_empty`,
`string_starts_with` and `string_contains`
([predicates](BORROWED-STRING-PREDICATE-CONDITIONS-V1.md)). They also admit
computed String temporaries. An append loop:

```semaprax
module app.join;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let mut out = string_from_i64(0);
    let mut i = 1;
    while i < 5 {
        out = string_concat(out, ",");
        out = string_concat(out, string_from_i64(i));
        i = i + 1;
        0
    }
    let view = string_as_str(out);
    let written = stdout_write(str_as_bytes(view));
    if written == 9usize { 0 } else { 1 }
}
```

`semaprax run join.spx` prints `0,1,2,3,4`. `while string_len(text) < 4`
reads its available owner's byte length without allocation. Condition
literals, blocks and String helpers settle before either Boolean outcome.
Consuming an enclosing owner is `SPX-T252`.

## Command-line programs

Permitting `fs.read`, `process.args.read`, or `process.stderr.write`
(optionally `process.stdout.write`) selects CLI behavior. `semaprax run
lines.spx -- data.txt` (also `--native` or a native build) passes `data.txt` to
`arg_utf8`. `main` returns the exit status (`0..=255`); stdout/stderr appear
after return. `file_read_text` reads below the current directory. Checked
failures (missing file, out-of-range slice) print one stderr line and exit 1.
Rules: [Text Toolkit v1](TEXT-TOOLKIT-V1.md).

For a CLI that imports source libraries, select table-manifest
`[package] profile = "source-command.v1"`, empty web exports, the exact
main stable ID in `[command]`, `input = "argv-utf8+file-text.v1"`, the
explicit sorted capability subset, and `[targets] matrix = ["native64"]`.
Then `semaprax build <project> --target native --output <fresh-path>` links
ordinary dependencies such as `std.int.decimal`. This route is native-only and
refuses interpreter/Web/npm/Wasm execution. See [Project v26](PROJECT-MANIFEST-V26.md)
and the [decimal CLI example](../examples/source-command-project/semaprax.toml).

Build with `semaprax build lines.spx --target native --output lines`. Omit
`--profile`: `text-toolkit-v1` and `internal-strings-v1` are Wasm/web export
profiles; native text operations come from source. Destinations must be fresh.
On `SPX-I307`, choose a new `--output` or, after confirming ownership, remove
the prior build artifact. The compiler never overwrites it.

Streaming uses Project v23 input
`argv-utf8+stdin-stream.v1` and profile `language-command-io.stream.v1`.
Native reuses a 4096-byte buffer; `stdin_read()` remains a snapshot. Open
prefills; zero bytes means EOF and short positive reads are chunks. Open/Next
require `process.stdin.read`; Eof/Chunk inspect named readers purely. Open runs
once per reachable path, never in loops. Readers have no constructor or
generic, aggregate, or public ABI escape. End a borrowed chunk's scope before
Next (`SPX-T265`); exact acyclic `own StdinReader -> StdinReader` helpers may
renew the owner.
[Streaming contract](BOUNDED-STDIN-STREAM-V1.md): gates and settlement.

Exit codes: `semaprax help language specifications`.

On the pure single-file interpreter route, `run` tries
`semaprax.interpret.v1`, then retries refusals with the internal String profile
for owned `string` parameters/results otherwise refused by `SPX-F102`. If both
refuse, report the ordinary diagnostic. Retry JSON `schema` is
`semaprax.interpret.internal-strings.v1`. Permit-selected command and stdout
runners skip this fallback.
See [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md) for
the separate profile contract.

```semaprax
module app.lines;

permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("app.usage")
fn usage() -> i64
    uses { process.stderr.write }
{
    let message = "usage: lines <file>\n";
    let view = string_as_str(message);
    let written = stderr_write(str_as_bytes(view));
    2
}

@id("app.report")
fn report() -> i64
    uses { fs.read, process.args.read, process.stdout.write }
{
    let path = arg_utf8(0usize);
    let text = file_read_text(path);
    let size = string_len(text);
    let mut start = 0;
    let mut lines = 0;
    let mut sum = 0;
    while start < size {
        let found = string_find(text, "\n", start);
        let end = if found < 0 { size } else { found };
        let line = string_trim(string_slice(text, start, end));
        sum = sum + match string_to_i64(line) { Option::Some { value: n } => n, Option::None {} => 0, };
        lines = lines + 1;
        start = end + 1;
        0
    }
    let mut out = "lines: ";
    out = string_concat(out, string_from_i64(lines));
    out = string_concat(out, "\nsum: ");
    out = string_concat(out, string_from_i64(sum));
    out = string_concat(out, "\n");
    let view = string_as_str(out);
    let written = stdout_write(str_as_bytes(view));
    0
}

@id("app.main")
fn main() -> i64
    uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    if args_len() == 1usize { report() } else { usage() }
}
```

For lines `4`, ` 5 `, `x`, `10`, running `lines.spx -- nums.txt` prints
`lines: 4` and `sum: 19` (exit 0); no argument prints usage to stderr (exit 2).
Bind `arg_utf8(i)` before passing it on. To compare an argument, copy it into a
`string` (`let raw = arg_utf8(1usize); let flag = string_from_str(raw);`), then
compare `flag == "--top"`. Match `string_to_i64` directly; in loops use exactly
`Option::Some { value }` and `Option::None {}`. Functions admit multiple
independent `if`s, `&&`/`||` operands, and `match`es. Offsets are bytes;
`string_byte_at(s, i) == 32` tests a space without allocation.

## String-keyed maps

`Map<string, i64>` counts or sums by text key. Create it with `let mut counts
= map_new(capacity);` and update it with `counts
= map_add(counts, key, delta);` (insert, or add to the value) and `counts =
map_set(counts, key, value);`, in straight-line code, loop bodies, and `if`
branches. Entries stay in ascending bytewise key order; visit them by index.

| Function | Signature |
| --- | --- |
| `map_new` | `(capacity: usize) -> Map<string, i64>` at most 65,536 entries |
| `map_add`, `map_set` | `(m: own Map<string, i64>, key: string, n: i64) -> Map<string, i64>` add to / replace |
| `map_get_or` | `(m: Map<string, i64>, key: string, default: i64) -> i64` |
| `map_has` | `(m: Map<string, i64>, key: string) -> bool` |
| `map_len` | `(m: Map<string, i64>) -> usize` |
| `map_key_at`, `map_value_at` | `(m: Map<string, i64>, i: usize) -> string` / `i64` in key order |

```semaprax
module app.tally;

@id("app.main")
fn main() -> i64
{
    let text = "b a c a";
    let size = string_len(text);
    let mut counts = map_new(16usize);
    let mut start = 0;
    while start < size {
        let found = string_find(text, " ", start);
        let end = if found < 0 { size } else { found };
        counts = map_add(counts, string_slice(text, start, end), 1);
        start = end + 1;
        0
    }
    let mut best = 0usize;
    let mut index = 0usize;
    while index < map_len(counts) {
        best = if map_value_at(counts, index) > map_value_at(counts, best) { index } else { best };
        index = index + 1usize;
        0
    }
    let top = map_key_at(counts, best);
    if string_compare(top, "a") == 0 && map_len(counts) == 3usize { map_get_or(counts, "a", 0) } else { -1 }
}
```

- Maps and sets move through explicit `own` parameters, results and bounded
  monomorphic record fields; `borrow` parameters permit reads. Collections
  are never Copy. Invariant-bearing or generic records stay outside this profile.
- `Map<K,V>` keys are `string`, `i64`, `bool`; values are String or any Copy
  scalar. Generic calls require `<K,V>`: `map_new<i64,string>(8usize)`,
  `map_set<i64,string>(map,1,"one")`, `map_remove<i64,string>(map,1)`.
  Generic `map_get_or`, `map_has`, `map_len`, `map_key_at`, `map_value_at`
  take the same type arguments. `map_add<K,i64>` adds with checked overflow.
  Calls without type arguments keep the `Map<string,i64>` API, including
  `map_remove(map,key)`.
- `Set<K>` uses `set_new<K>(capacity)`, `set_insert<K>(set,key)`,
  `set_remove<K>(set,key)`, `set_has<K>(set,key)`, `set_len<K>(set)`,
  `set_key_at<K>(set,index)`. Mutations consume and return the collection;
  reads borrow it. Missing removal and repeated set insertion succeed.
- Key order is unsigned UTF-8 bytes, signed `i64`, or `false` before `true`.
  String keys/values are copied on insertion; String read results are fresh
  owners. Other typed maps/sets use `semaprax.map.v2` with the same codes.
  See [String Collections v2](STRING-COLLECTIONS-V2.md).
- Keys are borrowed; the map copies a key when it inserts it. `while index <
  map_len(counts)` is a valid loop condition.
- A new key beyond the capacity, an index at or past `map_len`, a capacity
  above 65,536, and an overflowing `map_add` fail with `semaprax.map.v1`
  codes 1-4.
- Maps iterate in key order. Rank by repeated scans, so
  "ties by key" is "ties by index". String `<`, `<=`, `>`, `>=` compare unsigned UTF-8 bytes.
  [String Collections v1](STRING-COLLECTIONS-V1.md) owns the rules.

## Lists and iterators

`vec_sort<T>(values)` consumes and returns a Copy-scalar vector in ascending
order; use `values = vec_sort<T>(values)` for a mutable binding. Capacity and
length stay unchanged. Floats use IEEE total order, including signed zeros and
NaN encodings. Owned payloads such as Bytes are rejected.
[Copy Scalar Sort v1](COPY-SCALAR-SORT-V1.md) owns the operation.

A list of Copy scalars is a `Vec<T>`. Every `vec_*` call spells its element
type. Build with `let mut`, then move the finished vector into an immutable
binding before traversing it:

```semaprax
module app.vec;

@id("app.main")
fn main() -> i64
{
    let mut building = vec_with_capacity<i64>(4usize);
    building = vec_push<i64>(building, 10);
    building = vec_push<i64>(building, 32);
    let values = building;
    let mut total = 0;
    for item in values {
        total = total + item;
        0
    }
    if vec_len<i64>(values) == 2usize { total + vec_get<i64>(values, 0usize) - 10 } else { -1 }
}
```

`vec_set<T>(v, index, value)` and `vec_clear<T>(v)` return the next vector, as
`vec_push` does. A push beyond capacity fails at run time, so reserve enough
capacity up front or use `vec_reserve_exact<T>(v, additional)`.

`for item in values { body }` borrow-traverses a simple immutable
`Vec<T>` binding, where `T` is one of the eight Copy scalars. It snapshots the
length once, visits elements in ascending index order, and freezes `values`.
The body result is discarded. Keep the item immutable; do not move, mutate, or
reassign the vector inside the body. Computed iterable expressions, owned elements,
consuming traversal, and `break`/`continue` remain outside this `for` form. See
[Owned Bounded Vec For Traversal v1](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md).

### Consuming scalar iterators

`vec_into_iter<T>(values)` moves a scalar vector into a non-Copy `Iter<T>`.
`iter_next<T>(iterator)` consumes it and returns `IterStep<T>`: use
`match own` with `IterStep::Done {}` and `IterStep::Yield { item, rest }`.
The item is Copy; `rest` owns the remaining iterator. Pass `rest` to the next
step or let scope cleanup settle it. Reusing a consumed iterator is an
ownership error. Private helpers may consume and return the same iterator
or step type. Explicit `IterStep<T>::Done {}` also works without a vector.
All eight Copy scalars are admitted; owned items, lazy adapters, and public
iterator signatures remain separate work. Consuming `for own` is specified by
the separately implemented [Owning Iterator Loops v1](OWNING-ITERATOR-LOOPS-V1.md),
which keeps this iterator protocol's boundaries intact. See
[Owning Iterators v1](OWNING-ITERATORS-V1.md) and the separate
[closure profile](CLOSURES-V2.md). Private helpers may use one scoped generic
`T` for iterator parameters and results, reconstruct steps, and invoke scalar
callbacks under [Generic Iterator Helpers v1](GENERIC-ITERATORS-V1.md).

## Habits from other languages: diagnostic examples

Each block below shows a common first attempt. Its marker names the expected
diagnostic; the following text gives the fix. Parser and source-verifier
diagnostics repeat that fix in `help`; other messages name the accepted form.
Read the diagnostic first, then return here if needed.

<!-- expect: SPX-P106 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    return 42;
}
```

No `return`. Make the value the block's tail expression: `42`.

<!-- expect: SPX-P203 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let mut i = 0;
    while i < 3 {
        i = i + 1;
    }
    i
}
```

A `while` body must end with the continuation condition: add `i < 3` as the
body's last line.

<!-- expect: SPX-P203 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let mut x = 0;
    if x == 0 { x = 1; }
}
```

Statement `if` may omit `else` and branch values. The enclosing function
still needs its result: add `x` after the `if` as the block's tail expression.

<!-- expect: SPX-P106 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let sample = [1u8, 2u8];
    let view = array_as_slice(sample);
    match byte_get(view, 0usize) { Some(b) => 1, None => 0, }
}
```

Patterns spell the variant and its fields:
`Option::Some { value: b } => 1, Option::None {} => 0,`.

<!-- expect: SPX-T232 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let a: i32 = 5;
    0
}
```

Integer literals are `i64` unless suffixed: `let a: i32 = 5i32;`.

<!-- expect: SPX-U101 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let i = 0;
    i = i + 1;
    i
}
```

Declare mutable bindings with `let mut i = 0;`.

<!-- expect: SPX-T263 -->
```semaprax
module app.habit;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let text = "hi";
    let written = stdout_write(str_as_bytes(text));
    0
}
```

`str_as_bytes` takes a `str` view, not an owned `string`, and `string_as_str`
takes a binding, not a literal (`SPX-T266`):
`let view = string_as_str(text); stdout_write(str_as_bytes(view))`.

## Habits from other languages: diagnostic index

Other first-attempt diagnostics and their fixes:

| You wrote|Code|Fix|
| ---|---|---|
| native ownership/cleanup lowering refuses after successful check|`SPX-B104`|Preserve exact message + minimal reproducer: backend refusal, not source ownership error. Named String condition read with String scalar-match region parent is not canonical is a compiler regression; retain cleanup checks|
| for i in 0..n { … }|`SPX-P106`|Use while, a let mut counter, and a discarded tail|
| while body ending after assignment|`SPX-P203`|Add a discarded scalar tail such as 0|
| f(x); as a statement|`SPX-P106`|Discard it with let _ = f(x); or make it the tail|
| let t = (1, 2);|`SPX-P106`|No tuples; declare a `record`|
| Option::Some { value: 1 }|`SPX-T221`|Option<i64>::Some { value: 1 }|
| `index + 1` when `index: usize`|`SPX-T208`|Integer literals default to `i64`; write `index + 1usize`|
| let a: i32 = 5|`SPX-T232`|Suffix the literal: let a: i32 = 5i32|
| 9223372036854775808 or -(9223372036854775808)|`SPX-P003`|One literal: -9223372036854775808 or -2147483648i32. Whitespace after sign is trivia; parentheses separate it. Negating minimum or dividing by -1 overflows|
| "a" + "b"|`SPX-T250`|string_concat("a", "b")|
| f("abc") or f(owned) for borrow str|`SPX-T205`|Bind then borrow: let s = "abc"; f(string_as_str(s))|
| i64_from_f64(3) or usize_from_i64(1.5)|`SPX-T205`|Match types: i64_from_f64(3.0) or usize_from_i64(1)|
| f64_from_i64(1, 2)|`SPX-T204`|Pass one argument: f64_from_i64(1)|
| Unsupported Map<f64,i64> / Set<f64>|`SPX-T274`|Keys: string, i64, bool; values: String or Copy scalars|
| implicit collection ownership at helper boundary|`SPX-O001`|Use explicit own/borrow parameters; owned results move|
| Unsupported String/collection record profile|`SPX-T309`|Use explicit IDs, monomorphic acyclic records, own/borrow parameters; no invariants|
| point.get() on a record|`SPX-T203`|Records have no methods: call get(point) or use a class|
| let x = 1; let x = x + 1;|`SPX-T209`|No shadowing; pick a new name|
| assignment to an immutable binding|`SPX-U101`|Declare it with let mut before assigning|
| fn main() -> bool|`SPX-T104`|main returns i64; CLI exit status 0 means success|
| a second consume(b) after own|`SPX-O101`|Take borrow in the callee or pass a fresh value|
| struct, enum, pub, const|`SPX-P104`|Use record, variant, omit visibility, or return values from functions|
| match x { 0 => 0, _ => 1 }|`SPX-P106`|End every arm with ,, even the last; a declaration's last field/case may omit it|
| x += 1;|`SPX-P201`|x = x + 1;|
| c ? a : b|`SPX-P106`|if c { a } else { b }|
| break, continue|`SPX-P106`|Put the exit test in while condition|
| x as i64|`SPX-P106`|Use named conversions (Scalars and literals; range checked), or one integer type with suffixed literals|
| Rust/JavaScript closure|`SPX-P201`|Use fn(x: i64) -> i64 { x + 1 }|
| use std::io;|`SPX-G170`|Compiler-owned functions need no import; projects import one declaration: use function @id("…") from module as name;|
| rebuilding into an existing output path|`SPX-I307`|Choose fresh --output, or remove existing output only after confirming it is your prior build artifact; it never overwrites automatically|
| f()? in main|`SPX-T218`|Only Result functions propagate; match in main|
| [1, 2, 3]|`SPX-T262`|Arrays hold bytes ([1u8, 2u8]); use Vec<i64>|
| fn f() or -> ()|`SPX-P106`, `SPX-P105`|Spell the result type; unit is unsupported|
| a[0]|`SPX-P106`|Use byte_get(array_as_slice(a), 0usize) (Option<u8>)|
| Some(1), None|`SPX-T203`, `SPX-T202`|Option<i64>::Some { value: 1 }, Option<i64>::None {}|
| s.len() on a string|`SPX-T203`|Call string_len(s); see Compiler-owned functions for text operations. Only classes have methods|
| str_as_bytes(text) or str_as_bytes(string_as_str(text))|`SPX-T263`, `SPX-T266`|Bind view first: let view = string_as_str(text); str_as_bytes(view)|
| string_as_str("literal")|`SPX-T266`|Bind the literal before passing it to string_as_str|
| shape == Shape::Box { width: 1 } or option == Option<i64>::None {}|`SPX-T207`|Only payload-free, non-generic variants compare with ==; test others with match shape { Shape::Dot {} => true, _ => false, }|
| an or-pattern alternative with a payload, such as Shape::Box { width: w }|`SPX-M105`|Or-pattern alternatives are payload-free cases; give a payload case its own arm|
| String, int, or unsupported Vec inference/element types|`SPX-T001`/`SPX-T281`|Use string/scalars; spell Copy element and wrapper/vec_*<T> types; projects may use authenticated std.collections aliases|

## Web applications

`semaprax webapp app.spx -o out` turns one module into a full-stack web app:
REST API, persistence, browser and server validation, computed fields, and a
UI (dashboard, searchable/sortable/filterable paginated lists, detail pages,
forms). No `main` or `@id` is needed. This example is checked by the tests:

```spx webapp
module shop;

variant Tier {
    Free,
    Pro,
}

record Customer {
    name: string,
    tier: Tier,
    seats: i64,
}
    requires string_len(name) >= 2 && string_len(name) <= 80
    requires seats >= 1

record Order {
    customer_id: i64,
    total: f64,
    paid: bool,
}

fn customer_large(tier: Tier, seats: i64) -> bool
{
    seats >= 100 || tier == Tier::Pro {}
}

fn order_status(paid: bool) -> string
{
    if paid { "paid" } else { "due" }
}
```

- Each `record` is an entity at `/api/<snake_name>`. Field types: `string`,
  `i64`, `f64`, `bool`, `char`, payload-free variant. The server assigns `id`.
- `customer_id: i64` references `Customer`: select input, missing target
  rejected, deleting a referenced row is 409.
- Each `requires` line after a record is a validation rule (a record
  invariant). Any `fn <entity>_<name>(…)` is a computed field `<name>`.
  `<entity>` is snake_case (`time_entry`) or lowercase (`timeentry`).
  Parameters are that entity's fields, same name and type.
- Bodies: `let`, `if`/`else`, `match` (`A {} | B {}` joins payload-free
  cases), `==` on enums, arithmetic, comparisons, `&&` `||` `!`, `string_len` (bytes),
  `string_len_chars`, `string_is_empty`, `string_contains`,
  `string_starts_with`, `string_concat`, `string_from_i64`, and helper
  functions. No `i64` to `f64` cast: use a recursive helper. Errors:
  `SPX-WA102` type, `SPX-WA103` expression, `SPX-WA105` uncalled function.
- Unique key: `fn customer_key(email: string) -> string { email }` (any
  scalar; join fields with `string_concat`).
- Workflow on variant field `state`: `fn order_state_step(from: State, to:
  State) -> bool`. Rows start in the first case; updates must pass the step.
- Rollups: a computed field may take `count_<child>`, `count_<child>_<bool
  field>` (both `i64`), or `sum_<child>_<number field>` over the child rows
  that reference this row: `fn customer_spent(sum_order_total: f64) -> f64`.
- Accounts: `fn member_account(email: string, active: bool) -> bool {
  active }` makes `Member` the sign-in entity (login field first; the server
  keeps a write-only `password`). First run: start `node out/server.mjs
  --setup` in the background; until some account has a password every request
  is allowed, so POST the first account with a `"password"` of at least 8 bytes, then sign
  in. The self-test needs no setup.
- Permissions: `fn <entity>_can_read` / `_can_write(…) -> bool` take row
  fields plus `me: i64` and `my_<account field>`. Unprefixed `can_read` /
  `can_write` (or `can_write_<name>`) are defaults; one taking row fields such
  as `member_id: i64` covers every entity with those fields, the most
  specific default winning. Creation controls hide New on a definite account
  restriction; unknown row-dependent permissions keep the validated form
  available. The server checks the actual new row. Audit history and CSV
  export are automatic.
- Run `semaprax fmt app.spx && semaprax webapp app.spx -o out && node
  out/server.mjs --self-test`. It reports checks and cleanup;
  `semaprax webapp app.spx --api` lists the API.
- Cross-row rules: `<entity>_constraint[_name](fields, other_<entity>_<field>)
  -> bool` checks every distinct row pair; incoming changes also recheck it.
- Migration: `<entity>_migrate_<field>(old_<field>: type) -> type` (no
  parameters for a default); restart with `--migrate` after reviewing changes.
  The server validates the whole migrated state and saves the previous bytes.
- API: `GET`/`POST /api/<entity>`, `GET`/`PUT`/`DELETE /api/<entity>/<id>`,
  `GET /api/<entity>/<id>/history`, `?format=csv`, `GET /api/audit`; with
  accounts `POST /api/session {"login", "password"}` and `DELETE
  /api/session`. Before **every mutation**, GET `/api/session/csrf`, retain
  its cookie and send its JSON `token` as `X-CSRF-Token` (refresh after sign-in).
  Sign-in is rate limited. `node out/server.mjs [--port N] [--data DIR]
  [--setup] [--migrate]` serves until killed; start it in the background.

## Projects

`str_byte_at(text, 0usize)` reads borrowed UTF-8 bytes without an owned copy;
match its `Option<u8>` and use `i64_from_u8(byte)` to widen `Some` exactly.
`std.bytes.get_or` is also available in `useful-data.v1`:
`semaprax help library std.bytes.get_or`.

A project puts `semaprax.toml` beside `src/`. Use the table layout below;
the frozen one-line-per-key `semaprax.project.v1` layout remains admitted:

```toml
schema = "semaprax.manifest.v1"

[package]
name = "calculator"
version = "0.1.0"

[modules]
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["calculator.tests"]

[exports]
web = ["calculator.add"]

[dependencies]
std.num = "^0.1.0"
```

Canonical manifests use the shown table order, one blank line between tables,
one-line arrays, and no comments. Otherwise `SPX-J100` names the first differing
line in `help` (the frozen layout uses six ordered lines). Unknown/reserved tables
or keys are `SPX-J120`. `[package] profile` selects the admitted consumer
profile. `[dependencies]` links packages from the compiler's closed bundled
`std.*` inventory at version `0.1.0`; unknown packages and unsatisfied ranges
fail with `SPX-J121`, while ordinary non-bundled packages still require the
separate resolution route. `[targets] matrix = ["wasm32"]` rejects native
builds with `SPX-J122`.

Import by stable identity:
`use function @id("calculator.add") from calculator.core as add;` directly
after `module`; `entry` names the module declaring `main`. Project v1 function parameters and results are limited
to Copy scalar values. Records, classes, variants, `Option`, and `Result` may
be used as module-local implementation details inside scalar-signature
functions, but cannot cross a function boundary; `SPX-G174` points at a
declaration whose signature leaves that profile. A test module's `main` returns `0` on success;
`semaprax test semaprax.toml` prints `project tests passed`. Each
`fn test_<name>() -> i64` with an `@id` runs independently, without parameters.
Failures report stable id and outcome (`failed calculator.tests.test_add: returned
2`), with `cases` in the `--json` envelope. A violated `requires` or `ensures`
reports the function, the clause, and the argument values (`contract: requires
right != 0 in calculator.divide` / `arguments: left = 1, right = 0`).
[Project Test Cases v1](PROJECT-TEST-CASES-V1.md) owns both.
`semaprax help library` prints the [standard library catalog](STANDARD-LIBRARY-CATALOG.md)
offline: every `std.*` function, contract, profile, and `[dependencies]` route.
Add the dependency and import its `@id`; the installed compiler supplies bundled
packages without a checkout.
For JSON escape expansion and decoded member-name comparison in a v25
`stream-text` project, use [JSON String Query v1](JSON-STRING-QUERY-V1.md) and
the `std.data.json.query` catalog entry.
Bounded Vec uses profile `owned-data-api.v1` and
`std.collections = "^0.1.0"`. Import `std.collections.vec.*` by stable identity
with an explicit Copy-scalar type argument. Mutators transfer and return the
owner; the package has no public exports or stable generic ABI.
For one API, use `semaprax help library <module|name|stable-id>`: exact identity,
dependency, profile, signature, effects, and contracts, without fuzzy or prefix
search. [CLI Help v4](CLI-HELP-V4.md) owns the bounded lookup measurements.
[Package Manifest v1](PACKAGE-MANIFEST-V1.md) owns the table layout,
[Project Manifest v1](PROJECT-MANIFEST-V1.md) the frozen one,
[examples/calculator-project](../examples/calculator-project/semaprax.toml) is
the committed instance. `semaprax new <dir> --template stdin-stream-text` creates
a Project v25 streaming command; `doctor --profile` reports support but does
not select the Project profile. `project-scaffold` accepts the same template
and chooses the required tables layout.

`semaprax lock semaprax.toml --write` pins the project to a deterministic
`semaprax.lock` (identity, source digests, interface digest, targets,
capabilities); `--verify` re-checks it and `--compare <base.lock>` reports
whether the interface change is breaking, exiting nonzero for CI. A
`[dependencies]` table names dotted package identities with `^`/`~`/`=` ranges;
`semaprax resolve semaprax.toml --target native64 --cache <dir> --write` selects
them against a local content-addressed cache and pins the per-target
resolution, and `--verify` re-checks it. A build does not yet link resolved
dependencies. See [Project Lock v1](PROJECT-LOCK-V1.md) and
[Project Dependency Resolution v1](PROJECT-DEPENDENCY-RESOLUTION-V1.md).

## Where the rules live

For an application-defined exit status, select Project v24 profile
`language-command-io.stream.v2` with the same `argv-utf8+stdin-stream.v1` input
and an explicit command `fn() -> i64`. Return a status from 0 through 255;
status 2 may accompany your own stderr diagnostic and empty stdout. Build with
`semaprax build --manifest-path semaprax.toml --target native --output app` and run
that binary. Out-of-range results or checked execution failures discard staged
output and produce the generic adapter diagnostic with status 2. Project v23
keeps its Bool status 0/1 mapping. [Streaming command exit status
v1](BOUNDED-STDIN-COMMAND-EXIT-V1.md) owns selection and the verification boundary.

For owned String helpers and `string_slice`/`string_trim` in a streaming native
project, select Project v25 `language-command-io.stream-text.v1` with the same
input and i64 command. [Stream Text Command v1](STREAM-TEXT-COMMAND-V1.md) owns
its limits; v23/v24 retain their older text/helper refusals.

- [RFC 0001](RFC-0001.md): language and toolchain contract.
- [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md): records, variants, generics,
  matching, `Option`, `Result`.
- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md): ownership and cleanup.
- Bounded references for [explicit mutation](EXPLICIT-MUTATION-V1.md),
  [field mutation](FIELD-MUTATION-V1.md), [while loops](WHILE-LOOPS-V1.md),
  [bounded Vec `for` traversal](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md),
  [refutable match](REFUTABLE-MATCH-V1.md), [string operations](STRING-OPS-V1.md),
  [owned string loops](OWNED-STRING-LOOPS-V1.md),
  [text toolkit and command-line programs](TEXT-TOOLKIT-V1.md),
  [string-keyed maps](STRING-COLLECTIONS-V1.md),
  [owned string views](OWNED-STRING-BORROWED-VIEW-V1.md),
  [indexed byte data](PORTABLE-INDEXED-BYTE-DATA-V1.md),
  [command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md), and
  [class inheritance](CLASS-INHERITANCE-V1.md).
- [Using the SEMAPRAX CLI](CLI-GUIDE.md) for every command's scoped help.

Copy records can contain direct non-generic payload-free variant fields. Variant
fields with payloads or generic arguments still have no executable record layout;
use separate scalar observations or keep that variant outside the record.
