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

The installed compiler prints this page verbatim with
`semaprax help language all`. For a shorter starting point, use
`semaprax help language`.

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
   `semaprax help all` and `semaprax help language all` are for broad
   questions.

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
Use `u8_from_i64(value)` for checked byte narrowing and `char_from_u8(byte)`
for byte-value scalars (all 256 values; not an ASCII check), not code points.
Use `char_from_i64(value) -> char` (`core.num.char_from_i64`) for Unicode
scalars `0..=0x10FFFF` except `0xD800..=0xDFFF`; NUL is valid. Example:
`let face = char_from_i64(128512); let text = string_from_char(face);`
Out-of-range values fail with `semaprax.convert.v1` code 1. The source
implementation and focused current-head qualification are pending.
Existing exact integer widening and checked integer conversions run on the
interpreter, native, and Core Wasm; float conversions and `string_from_str`
retain `SPX-W116`. `i64_from_u8(byte)` is allocation-free and infallible. Integer `%`
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
  taking Copy scalars or flat Copy variants, borrowed byte slices, named `str`
  views, exact compiler-owned `borrow Vec<T>` for Copy-scalar `T`, or consumed
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
  Use canonical `vec_*<T>` operations inside loops; imported generic aliases
  remain generic calls and are refused with `SPX-T252`.
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
- Construct a generic variant as `Option<i64>::Some { value: v }`; match it
  as `Option::Some { value: v } => …`. `Some(v)` is invalid. Generic calls
  spell type arguments, as in `identity<i64>(4)`.
- `record … with { field: value }` is immutable update. Record construction
  must name every field (`SPX-T213`).
- Monomorphic acyclic records may own String/Bytes, Copy scalars and nested
  records. Pass with `own` or `borrow`; results own their leaves. Generic,
  resource/view/class and invariant-bearing String records are `SPX-T309`;
  see [String records](OWNED-STRING-RECORDS-V1.md).
- Copy-record Vec admits identified flat records with 1–8 Copy fields. Project
  v30 adds `Vec<string>` and flat records with up to two direct owned
  String/Bytes fields. User records need `@id`; ask
  `help language author:copy-record-vec` or `help language author:owned-data`.
  See [Owned Leaf Collections v1](OWNED-LEAF-COLLECTIONS-V1.md).
- Import every named type in a function import's signature, including nested
  types and inferred factory results. For example, import
  `@id("std.pattern.matcher")` with `std.pattern.make`; `SPX-G172` names a
  missing type identity.
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
- `requires` after a record declares effect-free `bool` field invariants.
  Literals, `with` updates and assignments re-check them (`SPX-C101`/`C102`).
  Generic records refuse invariants (`SPX-C103`); executable owned records
  refuse them (`SPX-C104`). Direct string fields retain the graph/webapp exception.

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

Use `string_format` for literal templates such as `"id={}"`. See
`help language author:literal-format` for fields, scope and pending gates.

`SPX-H006` loan-work refusal names the function: split into helpers, not files
or higher limits. For loop named-slice refusals, bind the view before the
helper call and pass its name; keep its owner alive through last use.

`text: string` transfers ownership. Do not write `own string`
(`SPX-O002`). A read-only helper takes `text: borrow str`; pass
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

- `string_concat` results are owned. `string_as_str` borrows a plain `let`
  binding; literals and calls are `SPX-T266`. Pass its view to `borrow str` or
  `str_as_bytes`.
- Byte functions take `borrow Slice<u8>`. Get one from `str_as_bytes(view)`,
  `array_as_slice(array_binding)`, or `bytes_as_slice(bytes_binding)`.
- Build a bounded byte buffer in one write-once expression: `bytes_zeroed`
  requires a literal `usize` capacity; each `bytes_set` takes the prior link,
  a `usize` index expression, and a byte. Binding freezes it for borrowed reads.
  Re-opening a named binding is `SPX-T271`; a literal index >= capacity is
  `SPX-T272`. A computed out-of-bounds index fails before writing with
  `semaprax.byte-buffer.v1` code 1.
  [Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) owns the rule.
- `buffer = bytes_set(buffer, index, value)` is same-owner replacement and the
  only `bytes_set` admitted inside bounded `while`. `bytes_zeroed`, `bytes_copy`,
  and `vec_clone_at` on Bytes-bearing records allocate and are refused there
  (`SPX-T267`); borrowed views cannot cross replacement (`SPX-T265`).

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
- Single-file `run` evaluates `main` in the bounded interpreter; `--native`
  selects C11. `--json`, `--max-steps` and `--max-bytes` bound reports and
  execution. Exact `process.stdout.write` selects transcript output (the
  example prints `banana!0`). Args, file or stderr effects select the CLI
  route below; `stdin_read` needs native `useful-data-command.v1`.
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
| `string_format` | `(literal, fields...) -> string`; literal-only template, up to 32 `i64`, `u8`, `usize`, `bool`, or owned String fields |
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

`string_format("id={}", 3)` consumes String without JSON escaping.
Exact grammar/gates: `help language author:literal-format`.

Reserved `string_len`: `SPX-S113`.

Boxes allocate uniquely; authored `record Box<T>` stays inline. Owned payloads,
public generic ABI and shared ownership remain outside [Box v1](OWNED-BOUNDED-BOX-V1.md).

Render, borrow and write an integer:

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

`run count.spx` prints `42`; signed values use `string_from_i64`.

[String replacement](STRING-REPLACEMENT-V1.md) settles the old owner after
RHS success. Owning names move; active views prevent replacement.

Conditions admit String owners, temporaries and checked
[borrowed predicates](BORROWED-STRING-PREDICATE-CONDITIONS-V1.md). An append loop:

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

`semaprax run join.spx` prints `0,1,2,3,4`. Loop conditions settle String
temporaries before either Boolean outcome; consuming the enclosing owner is
`SPX-T252`.

## Command-line programs

`fs.read`, `process.args.read`, or `process.stderr.write` (optionally
`process.stdout.write`) selects CLI behavior. `semaprax run lines.spx -- data.txt`
passes `data.txt` to `arg_utf8`; add `--native` for native. `main` returns
`0..=255`; output appears after return. `file_read_text` stays below cwd;
checked read failure prints one stderr line and exits 1.
[Text Toolkit v1](TEXT-TOOLKIT-V1.md).

`SPX-T269`: keep direct writes outside loops and within selected-profile
capacity admission. The default combined stdout + stderr cap is 65,536 bytes;
Project v28 permits 1 MiB staged appends. See [transcript rules](BOUNDED-STDOUT-TRANSCRIPT-V1.md)
and [Project v7](PROJECT-MANIFEST-V1.md#additive-project-manifest-v7-line-command-profile).

Project `source-command.v1` requires a native64 table manifest and exact
`[command]` identity; Web/Wasm/npm refuse it. Interpreter argv/file refusal is
`SPX-F102`; `test --target native` executes authenticated tests. V28 adds bounded
1 MiB String/output without changing v26's file quotas. See [v26](PROJECT-MANIFEST-V26.md),
[v28](PROJECT-MANIFEST-V28.md) and [native tests](PROJECT-NATIVE-TEST-V1.md).

Dependency starter and library card:

```sh
semaprax new decimal-command --template source-command-file-text
cd decimal-command
semaprax help library std.int.decimal
semaprax check .
semaprax build --manifest-path semaprax.toml --target native --output app
./app digits
```

It pins `std.int.decimal = "=0.1.0"`; import `canonicalize`, `add`, and `divide`
by IDs from `semaprax help library std.int.decimal`.

For bounded ASCII byte patterns, declare `std.pattern = "^0.1.0"` in an
`owned-data-api.v1` package and import by stable ID. `help library std.pattern`
gives exact calls and limits. Compile before observing a matcher; captures are
byte offsets. This is not a general Unicode regex engine.

Build `lines.spx` natively to fresh `--output`; omit `--profile` because
`text-toolkit-v1` and `internal-strings-v1` are Wasm/web export profiles. On
`SPX-I307`, choose a new output or remove your prior artifact after checking
ownership; the compiler never overwrites it.

Project v23 streaming uses `argv-utf8+stdin-stream.v1` and a reusable native
4096-byte buffer. Open/Next need `process.stdin.read`; end each borrowed chunk
before Next (`SPX-T265`). Readers have no public ABI escape. See the
[streaming contract](BOUNDED-STDIN-STREAM-V1.md).

Native v27 starter: `semaprax new <dir> --template stdin-stream-data`.

Exit codes: `semaprax help language specifications`.

On the pure single-file interpreter route, `run` tries `semaprax.interpret.v1`,
then retries refusals with the internal String profile for owned `string`
parameters/results otherwise refused by `SPX-F102`. If both refuse, report the
ordinary diagnostic. Retry JSON `schema` is
`semaprax.interpret.internal-strings.v1`. Permit-selected command and stdout
runners skip this fallback. See [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md).

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

With `nums.txt` containing `4`, ` 5 `, `x`, `10`, the sample prints `lines: 4`
and `sum: 19` (exit 0); no args prints usage to stderr (exit 2). Bind
`arg_utf8(i)` before forwarding; copy to `string` with `string_from_str` to
compare a flag. Match `string_to_i64` directly; loop cases use
`Option::Some { value }` and `Option::None {}`. Multiple `if`s, `&&`/`||`, and
`match`es are admitted. Offsets are bytes; `string_byte_at(s, i) == 32` checks
space without allocation.

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

`str_as_bytes` accepts a borrowed `str` view. For a named owned `string`, the
view may be composed directly as `str_as_bytes(string_as_str(text))`; bind the
result to a `Slice<u8>` before using it as a loop-carried view. Within a loop
body, [Owned String Loops v2](OWNED-STRING-LOOPS-V2.md#immutable-input-and-borrowed-views-590)
also permits named local views over one exact named String owner:
`let view = string_as_str(line); let bytes = str_as_bytes(view);`. Keep the
owner alive through the last use. The additive projected-string-view work
extends this to `string_as_str(record.text)` and nested named record paths
ending in a String field, rooted in a live named own/borrow record with no
invariants. The fused form `str_as_bytes(string_as_str(record.inner.text))`
also works in a nonescaping borrowed call or loop. It preserves the full owner
and field path; it does not clone or move the String. Literal, temporary, call,
and constructor roots remain refused. Source implementation and qualification
are pending; see [Projected String Views v1](PROJECTED-STRING-VIEWS-V1.md).

Use `u8_from_i64(value)` then `char_from_u8(byte)` for byte conversion; use
`char_from_i64(value)` for arbitrary Unicode code points.

## Habits from other languages: diagnostic index

| You wrote|Code|Fix|
| ---|---|---|
| native cleanup refusal postcheck|`SPX-B104`|Repro/check cleanup; String-condition scalar match “parent is not canonical”: backend regression|
| for range|`SPX-P106`|while; mutable counter; discard tail|
| assignment loop|`SPX-P203`|Scalar tail discard, e.g. 0|
| local named result|`SPX-S109`, `SPX-T201`|Rename binding and uses to outcome; result names the return value only in ensures|
| statement call|`SPX-P106`|let _ = f(x) or tail result|
| tuple|`SPX-P106`|No tuples; declare a record|
| Option::Some { value: 1 }|`SPX-T221`|Option<i64>::Some { value: 1 }|
| index + 1 when index: usize|`SPX-T208`|Literals default to i64; use index + 1usize|
| unsuffixed i32|`SPX-T232`|Suffix: 5i32|
| 4i64|`SPX-P003`|Write 4; unsuffixed integers are i64. Explicit suffixes: i32, u8, usize|
| i64 max+1 / parenthesized MIN negation|`SPX-P003`|One literal: -9223372036854775808/-2147483648i32; spaces trivia, parens separate; -MIN/MIN÷-1 overflow|
| "a" + "b"|`SPX-T250`|string_concat("a", "b")|
| literal/String as str arg|`SPX-T205`|Bind String; pass string_as_str(s)|
| conversion type error|`SPX-T205`|i64_from_f64(3.0) or usize_from_i64(1)|
| excess conversion args|`SPX-T204`|One arg: f64_from_i64(1)|
| float Map key/Set element|`SPX-T274`|Keys string/i64/bool; values String/Copy scalars|
| implicit helper ownership|`SPX-O001`|own/borrow required; results move|
| String/collection record|`SPX-T309`|IDs; monomorphic acyclic; own/borrow; no invariants|
| record method|`SPX-T203`|get(point) or class; records lack methods|
| shadowed binding|`SPX-T209`|Rename binding|
| assign immutable|`SPX-U101`|let mut|
| bool main|`SPX-T104`|i64 result; CLI 0 succeeds|
| reuse after own|`SPX-O101`|Callee borrow, or fresh value|
| struct/enum/pub/const|`SPX-P104`|record/variant; omit visibility; return values|
| missing arm comma|`SPX-P106`|Arm commas; final field/case comma optional|
| compound assign|`SPX-P201`|x = x + 1|
| ternary expression|`SPX-P106`|if c { a } else { b }|
| break / continue|`SPX-P106`|Exit test in while|
| x as i64|`SPX-P106`|Checked named conversion/suffixed literal|
| Rust/JS closure|`SPX-P201`|fn(x: i64) -> i64 { x + 1 }|
| use std::io;|`SPX-G170`|Built-ins need no import. std.* packages: declare its Project dependency; import by stable ID|
| factory import lacks exact type|`SPX-G172`|Add the direct use type @id("…") from module as Type shown in help, including nested exposed types; inferred results grant no import authority|
| noncanonical Project source|`SPX-G170`|semaprax fmt <manifest>; if manifest layout blocks discovery, first semaprax fmt --manifest <manifest>, then retry|
| interpreter: source-command.v1/v28 Project|`SPX-F102`|Interpreter lacks argv/file provider; build: semaprax build <manifest> --target native -o <fresh-path>, run in Project directory. Declared tests: semaprax test <project> --target native [Native Tests](PROJECT-NATIVE-TEST-V1.md)|
| output exists|`SPX-I307`|Fresh --output; never overwrite; remove confirmed own artifacts only|
| f()? in main|`SPX-T218`|Result-only propagation; main matches|
| array literal|`SPX-T262`|Byte arrays; Vec<i64> otherwise|
| fn f() or -> ()|`SPX-P106`, `SPX-P105`|Result type required without unit|
| a[0]|`SPX-P106`|byte_get(array_as_slice(a), 0usize) (Option<u8>)|
| Some(1), None|`SPX-T203`, `SPX-T202`|Option<i64>::Some { value: 1 }, Option<i64>::None {}|
| s.len() on string|`SPX-T203`|string_len(s); only classes have methods|
| str_as_bytes/nested string_as_str|`SPX-T263`, `SPX-T266`|For named String: str_as_bytes(string_as_str(text)); name loop-carried slices before loops|
| direct output repeats per path / is loop-reachable|`SPX-T269`|Direct writes outside loops; profile limits. Default stdout+stderr ≤65536 bytes; Project v28 staged appends ≤1 MiB|
| string_as_str("literal")|`SPX-T266`|let s = "literal"; string_as_str(s)|
| payload/generic ==|`SPX-T207`|Match; == only payload-free/nongeneric variants|
| payload in or-pattern|`SPX-M105`|Payload-free alternatives; split payload arms|
| String/int/invalid Vec|`SPX-T001`/`SPX-T281`|String/scalars; explicit Copy Vec<T>; authenticated imports|
| bad [modules]|`SPX-J100`|2–16 sorted sources; one bounded nonentry test module. entry="app", sources=["a.spx","b.spx"], tests=["app.tests"] [Manifest](PACKAGE-MANIFEST-V1.md)|
| generic while call|`SPX-T252`|vec_len<T>; imported generic aliases closed [While](WHILE-LOOPS-V1.md)|
| rejected while helper|`SPX-T252`|Borrow compiler Copy scalar Vec<T>; return scalar/flat Copy variant/string|
| outer owner changes in while|`SPX-T252`|Preserve outer ownership|
| Bytes+usize renewal/input views|`SPX-T252`/`SPX-T265`|Pure nongeneric call; return only one whole same type owner; whole named independent Slice/str borrows only [renewal hook](IO-LINES-V1.md#cursor-transitions), executable gate pending|
| Vec capacity >8192|`SPX-T282`|Reduce vec_with_capacity<T>; Vec-only bound [Vec](OWNED-BOUNDED-VEC-V1.md)|
| lookalike Vec wrapper|`SPX-T283`|Exact std.collections.vec.* ID; no substitute [Vec](OWNED-BOUNDED-VEC-V1.md)|
| function: >256 shared loans|`SPX-H006`|Fewer loans; fixed limit [Loans](SHARED-LOAN-PLAN-V1.md)|
| function: >4096 loan points|`SPX-H006`|Simpler flow; admitted helpers|
| function: >4096 CFG edges|`SPX-H006`|Simpler flow; admitted helpers|
| loan work >1000000|`SPX-H006`|Less work; fixed bound|

## Web applications

`semaprax webapp app.spx -o out [--title "Application name"]` turns one module into a full-stack web app:
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
- Run `semaprax fmt app.spx && semaprax webapp app.spx -o out --title "Shop" && node
  out/server.mjs --self-test-offline` for deterministic in-memory runtime
  checks with no data directory, listener, or child. `--self-test [--data DIR]`
  remains the real loopback-server check; full acceptance remains mandatory.
  `semaprax webapp app.spx --api` lists the API.
- Cross-row rules: `<entity>_constraint_name(fields, other_<entity>_<field>)
  -> bool` (optional `_name` suffix) checks every distinct row pair; incoming changes recheck it.
- Migration: `<entity>_migrate_<field>(old_<field>: type) -> type` (no
  parameters for a default); restart with `--migrate` after reviewing changes.
  The server validates the whole migrated state and saves the previous bytes.
- API: `GET`/`POST /api/<entity>`, `GET`/`PUT`/`DELETE /api/<entity>/<id>`,
  `GET /api/<entity>/<id>/history`, `?format=csv`, `GET /api/audit`; with
  accounts `POST /api/session {"login", "password"}` (or the declared login
  field name in place of `login`) and `DELETE
  /api/session`. Before **every mutation**, GET `/api/session/csrf`, retain
  its cookie and send its JSON `token` as `X-CSRF-Token` (refresh after sign-in).
  Sign-in is rate limited. `node out/server.mjs [--port N] [--data DIR]
  [--setup] [--migrate]` serves until killed; start it in the background.

## Projects

`str_byte_at(text, 0usize)` reads borrowed UTF-8 bytes; match its `Option<u8>`
and widen `Some` with `i64_from_u8`. `std.bytes.get_or` is available in
`useful-data.v1` (`semaprax help library std.bytes.get_or`).

Keep `semaprax.toml` beside `src/`. Tables are canonical; the frozen
one-line-per-key `semaprax.project.v1` layout remains admitted.

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

Use canonical table order, blank lines, one-line arrays, no comments. Frozen v1
has six ordered lines; `SPX-J100` identifies the first differing line and
unknown tables/keys give `SPX-J120`. `[package] profile` selects boundary
carriers. Bundled `std.*` is `0.1.0`; unknown packages/ranges give `SPX-J121`
on the separate resolution route. A wasm-only target rejects native builds
with `SPX-J122`.

Import functions by stable ID after `module`, e.g.
`use function @id("calculator.add") from calculator.core as add;`; `entry`
names the module with `main`. Project v1 admits Copy scalars; profiles add only
their stated carriers. On `SPX-G174`, keep unsupported aggregates local or
select an admitting profile. For H006's 4,096 loan-point limit, extract named
helpers with admitted signatures; other H006 errors need their specific fix.

A test module `main` returns 0 on success; each `@id`'d `fn test_<name>() ->
i64` runs independently. Failures report stable IDs/outcomes and contract
clause/arguments. Ordinary tests use the interpreter; v26/v28 source-command
profiles can select authenticated native roots with
`semaprax test <project> --target native` ([bounds](PROJECT-NATIVE-TEST-V1.md),
[cases](PROJECT-TEST-CASES-V1.md)).

Use `semaprax help library` to list modules, `... all` for the offline
[catalog](STANDARD-LIBRARY-CATALOG.md), or
`... <module|name|stable-id>` for exact lookup (no fuzzy/prefix search).
Import its `@id` and dependency; bundled packages supply contracts/profiles.
Bounded Vec uses `owned-data-api.v1` and `std.collections = "^0.1.0"`; import
`std.collections.vec.*` by ID with an explicit Copy-scalar type argument.
Mutators transfer and return the owner; there is no public export or stable
generic ABI. [Package Manifest v1](PACKAGE-MANIFEST-V1.md) owns table layout;
[Project Manifest v1](PROJECT-MANIFEST-V1.md) owns the frozen format.

`semaprax new <dir> --template source-command-file-text` creates v26
`source-command.v1`/native64 with `argv-utf8+file-text.v1`; Web/Wasm/npm refuse
it. Interpreter `run`/`test` retain `SPX-F102`; use native `semaprax test .`.
`doctor --profile` reports but does not select profiles. Opt-in v28
`source-command.resource-output.v1` keeps the v26 ABI and adds bounded 1 MiB
Strings, borrowed text and staged output; see [limits](PROJECT-MANIFEST-V28.md).

`stdin-stream-data` selects native v27 `language-command-io.stream-data.v1`
(private Copy-scalar `Vec<T>`); v29 adds private Copy records/`Vec<R>`, and
v30 `language-command-io.owned-data.v1` adds private owned `Vec<string>` and
flat owned-leaf records. Schema `Vec` declarations alone grant no carrier.
`text: string` consumes; `text: own string` is `SPX-O002`. Push consumes;
`for own` uses `vec_into_iter`. Bytes allocation/copy or cloning a Bytes-bearing
record in `while` is `SPX-T267`. See `help language author:owned-data` and the
[v27](STREAM-DATA-COMMAND-V1.md), [v29](STREAM-DATA-COMMAND-V2.md),
[v30](STREAM-OWNED-DATA-COMMAND-V1.md) references. The String-plus-scalar
[owned-leaf-command](../examples/owned-leaf-command-project/README.md) gate is pending.

Project v31 `language-command-io.collection-record.v1` adds private typed
records with admitted Vec shapes; command/entry stay `fn() -> i64`. Prefer
`vec_field<Row>(values,index,"field")` for queries: the literal resolves an
explicit field; Copy returns by value, String/Bytes as `borrow str`/`borrow
Slice<u8>`. Fuse bytes with `str_as_bytes(vec_field<Row>(values,index,"text"))`;
named record paths allow `string_as_str(record.text)`. The view locks the whole
named vector generation through last use; do not move/sort/push/reserve before
then. `vec_clone_at` is for an owned result. These private reads apply only in
v30/v31/v32, not public ABIs. Implementation/qualification pending; see
[scoped Vec field reads](SCOPED-VEC-FIELD-READS-V1.md).

`semaprax lock --write` pins identity, sources, interface, targets and
capabilities; `--verify` checks it and `--compare <base.lock>` reports breaking
changes. Dependency ranges use `^`, `~` or `=`. `semaprax resolve` pins
per-target resolution; Build does not yet link resolved dependencies. See
[Lock](PROJECT-LOCK-V1.md) and [resolution](PROJECT-DEPENDENCY-RESOLUTION-V1.md).

## JSON documents and cursors

Strict JSON uses `std.data.json.scan = "^0.1.0"` and `profile =
"useful-data.v1"`. Call `strict_end(input, 32usize, policy)` by ID first.
Policy `0` allows duplicate names; `1` rejects decoded duplicates. Success is
input length; larger values encode the error offset. Navigation and exact
number spans use borrowed bytes. See [Strict JSON Scan v1](STRICT-JSON-SCAN-V1.md).
`std.data.json.query` decodes string tokens ([JSON String Query v1](JSON-STRING-QUERY-V1.md)).

`semaprax json-codec` derives checked source from an authenticated Project record.
`help language author:json-owned-request` selects `owned-request.v1` or
`stream-owned-request.v1`. A request has `Vec<string>` identifiers (≤8) and
`Vec<Row>` (≤256). A flat stable-ID `Row` has a `string` identifier plus
0–6 `i64`, `u8`, `usize` or `bool` fields; each field has an ID and order may
vary. In the ASCII owned profiles, identifiers are unique, 1–16 ASCII
letters, digits, `_` or `-`, and nonempty rows need an identifier. Encoders borrow
both vectors and `output_limit`. `json_<Request>_owned_decode` returns
independent Strings, so completed decode permits release of normalized input
Bytes. Streaming errors before Ready use raw offsets; later errors use
normalized-buffer offsets. The ASCII owned profile accepts only its identifier
policy. The separate `utf8-owned-request.v1` selector requires
`--max-string-bytes N` (canonical 1..64 decoded UTF-8 bytes per string); it
accepts empty and duplicate values, including Unicode and NUL, in the same
0..8 and 0..256 array bounds. The second array may be nonempty when the first
is empty. External input retains the existing 65,536-byte borrowed-root
limit; internal owned Bytes views retain their 131,072-byte bound. Neither
limit is increased. Source field identifiers stay ASCII.
This is not a stream selector; see `help language author:json-utf8-owned-request`.
Owned runtime support is private to
`owned-data-api.v1` or native v30; v29 views keep Copy tokens tied to source
lifetime. Source and current-head qualification remain pending; this is not
full #724.

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
