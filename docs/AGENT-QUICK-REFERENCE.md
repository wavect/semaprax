# Agent quick reference

Status: public alpha reference card. Every `semaprax` code block on this
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

1. Write the file, run `semaprax fmt <file>`, then
   `semaprax check <file> --json`. Run it with `semaprax run <file>` when the
   check succeeds. Fix the first diagnostic at its reported line and column.
   Diagnostics contain `code`, `message`, `location`, and `help`; tests should
   match the stable `SPX-…` code, not message wording.
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
- A function body has zero or more statements (`let`, assignment, `while`,
  `unsafe`) and one final expression. That expression supplies the block's
  value. User code has no `return`, expression statement, `for`, `else if`,
  tuple, or unit value.
- Source blocks, delimiters, unary chains, and expression trees may nest at
  most 128 levels; `SPX-P207` asks you to extract a named helper.
- Canonical layout puts the function body's `{` on its own line and each
  statement on its own line; `if`, `match`, and record literals stay on one
  line. Let `fmt` do it.

## Scalars and literals

- `i64`: `42`, `-1`; default integer, checked overflow.
- `i32`: `42i32`; suffix required, no implicit widening.
- `u8`: `255u8`; byte value.
- `usize`: `3usize`; lengths and indices; compare only with `usize`.
- `f64`, `f32`: `1.5`, `1.5f32`.
- `bool`: `true`, `false`; `&&`, `||`, `!`.
- `char`: `'a'`, `'\n'`, `'\u{2603}'`.
- `string`: `"text"`; owned UTF-8, content equality with `==`.
- `str`: no literal; borrowed by `borrow str` or `string_as_str(binding)`.
- `[u8; N]`: `[97u8, 98u8]`; fixed; `array_as_slice(binding)` gives
  `Slice<u8>`.
- `Bytes`, `Slice<u8>`: no literal; owned bytes and borrowed byte view.

Operators do not mix types. If `n` is `usize`, `n < 5` fails with `SPX-T208`;
write `n < 5usize`. Use `string_concat`, not `+`, for strings.

## Control flow, mutation, contracts, effects

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

- `if` always has `else` and is an expression. Nest `if` inside `else { … }`
  instead of `else if`.
- A `while` condition must be `bool` and is checked before every iteration.
  Its body still needs a final expression, but that value is discarded; the
  condition controls repetition. While bodies admit only
  Copy-scalar operations and scalar-returning calls, plus the exact
  `byte_get`/`Option<u8>` inspection profile; record/variant construction and
  aggregate-returning calls are `SPX-T252`.
- Bindings are immutable unless `let mut`. Assignment is a statement:
  `x = x + 1;` or `point.x = 5;`. Parameters are immutable.
- Contracts are `requires`/`ensures` lines between the signature and the body;
  `result` names the return value. They are checked at run time.
- Effects: the module lists `permit { … }`, and every function that performs
  or calls into an effect declares `uses { … }`. Missing `permit` is
  `SPX-E101`; a missing `uses` is `SPX-E102`.
- `match` on scalars needs a final catch-all arm (`_` or a binding) without a
  guard, else `SPX-T257`.
- Match arms cannot yield nominal aggregates. `SPX-T258` means to use `if` to
  construct the record/variant, or extract scalars first.

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

- Give every field and case its own `@id`. Cases without payload are
  written `Name,` in the declaration and `Type::Name {}` everywhere else.
- Constructing a generic variant spells the type arguments:
  `Option<i64>::Some { value: v }`. Matching one does not:
  `Option::Some { value: v } => …`. Neither side accepts `Some(v)`. Generic
  functions are called with explicit type arguments: `identity<i64>(4)`.
- `record … with { field: value }` is immutable update. Record construction
  must name every field (`SPX-T213`).
- Classes hold fields and `fn name(self: Class, …)` methods, called as
  `value.method(args)`. `class Dog : Animal` inherits; `super.method()`
  dispatches to the parent. Records have no methods.

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

- A `string` literal or `string_concat` result is owned. Borrow it with
  `string_as_str(binding)`; the argument must be a plain `let` binding, not a
  literal or call (`SPX-T266`). Pass the `str` view to `borrow str`
  parameters and to `str_as_bytes`.
- Byte functions take `borrow Slice<u8>`. Get one from `str_as_bytes(view)`,
  `array_as_slice(array_binding)`, or `bytes_as_slice(bytes_binding)`.
- Build an owned bounded byte buffer as one write-once expression:
  `bytes_zeroed` allocates at a `usize` literal capacity, and each `bytes_set`
  link takes the previous link, any `usize` index expression, and the byte.
  A binding freezes the result; read it with borrowed operations. You cannot
  re-open a named binding (`SPX-T271`); a literal index at or above capacity is
  `SPX-T272`, and a computed index outside the buffer fails at run time with
  `semaprax.byte-buffer.v1` code 1 before anything is written.
  [Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) owns the rule.
- One form re-opens a frozen buffer: the same-owner replacement
  `buffer = bytes_set(buffer, index, value)`, where the assignment target and
  the `buffer` operand are the same `let mut` binding. That is also the only
  `bytes_set` a bounded `while` body admits, so a loop fills a buffer the loop
  did not allocate. `bytes_zeroed` stays outside the loop (`SPX-T267`), and a
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
- Single-file `run` evaluates `app.main` in the bounded reference interpreter.
  `--json`, `--max-steps`, and `--max-bytes` are available; `--native`
  explicitly selects the generated C11 route. The exact
  `process.stdout.write` authority automatically selects the bounded stdout
  transcript interpreter, so the example above prints `banana!0`. `args_len`, `arg_utf8`,
  `stdin_read`, and `stderr_write` need a project with the
  `useful-data-command.v1` profile built for the native target.
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
- Reference-interpreter and generated native calls have a fixed 256-frame
  recursion bound. Exceeding it is a reported runtime-capacity failure, not a
  language status or process signal. Raw WebAssembly execution remains subject
  to its engine's visible stack-limit trap.

## Compiler-owned functions

| Function | Signature |
| --- | --- |
| `string_len`, `string_len_chars` | `(s: string) -> i64` bytes / scalars |
| `string_is_empty` | `(s: string) -> bool` |
| `string_concat` | `(a: string, b: string) -> string` consumes both |
| `string_starts_with`, `string_contains` | `(s: string, other: string) -> bool` |
| `string_from_char` | `(c: char) -> string` |
| `string_from_i64` | `(value: i64) -> string` canonical decimal text |
| `string_from_usize` | `(value: usize) -> string` canonical decimal text |
| `string_as_str` | `(binding: string) -> borrow str` |
| `str_len_bytes` | `(s: borrow str) -> i64` |
| `str_is_empty` | `(s: borrow str) -> bool` |
| `str_starts_with`, `str_contains` | `(s: borrow str, other: borrow str) -> bool` |
| `str_as_bytes` | `(s: borrow str) -> Slice<u8>` |
| `byte_len` | `(v: borrow Slice<u8>) -> usize` |
| `byte_get` | `(v: borrow Slice<u8>, i: usize) -> Option<u8>` |
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
| `box_get<T>` | `(value: borrow Box<T>) -> T` synchronous Copy access |
| `box_into_inner<T>` | `(value: own Box<T>) -> T` consuming extraction |

These names are reserved. Declaring your own `string_len` is `SPX-S113`.

The three Box operations select a compiler-owned, uniquely owned allocation.
An authored `record Box<T>` without them is still an inline record. The bounded
Box profile has no owned payload, public generic ABI, region, arena, or
shared-ownership surface; see [Owned Bounded Box v1](OWNED-BOUNDED-BOX-V1.md).

To print a computed integer from one file, render it, borrow the resulting
string, and write its bytes:

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

`semaprax run count.spx` prints `42`. Use `string_from_i64` for signed values.

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

<!-- expect: SPX-P106 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let x = 2;
    if x == 0 { 0 } else if x == 1 { 1 } else { 2 }
}
```

No `else if`. Write `else { if x == 1 { 1 } else { 2 } }`.

<!-- expect: SPX-P203 -->
```semaprax
module app.habit;

@id("app.main")
fn main() -> i64
{
    let mut x = 0;
    if x == 0 { x = 1; }
    x
}
```

Every block yields a value, so a branch that only assigns still ends with an
expression: `if x == 0 { x = 1; x } else { x }`.

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

| You wrote | Code | Fix |
| --- | --- | --- |
| `for i in 0..n { … }` | `SPX-P106` | Use `while` with a `let mut` counter and an ordinary discarded tail |
| a `while` body ending after assignment | `SPX-P203` | Add the continuation condition as the body's final expression |
| `f(x);` as a statement | `SPX-P106` | Discard it with `let _ = f(x);` or make it the tail |
| `let t = (1, 2);` | `SPX-P106` | No tuples; declare a `record` |
| `id(4)` for `fn id<T>` | `SPX-T225` | `id<i64>(4)` |
| `Option::Some { value: 1 }` | `SPX-T221` | `Option<i64>::Some { value: 1 }` |
| `index + 1` when `index: usize` | `SPX-T208` | Integer literals default to `i64`; write `index + 1usize` |
| `let a: i32 = 5` | `SPX-T232` | Suffix the literal: `let a: i32 = 5i32` |
| `9223372036854775808` or `-(9223372036854775808)` | `SPX-P003` | The signed minimum is one literal: write `-9223372036854775808`, or `-2147483648i32` for `i32`. Whitespace between the sign and the magnitude is trivia; a parenthesis is not. `-MIN` and `MIN / -1` still fail closed on checked overflow |
| `"a" + "b"` | `SPX-T250` | `string_concat("a", "b")` |
| `f("abc")` or `f(owned)` for `borrow str` | `SPX-T205` | `let s = "abc"; f(string_as_str(s))` |
| `point.get()` on a record | `SPX-T203` | Records have no methods; call `get(point)` or use a `class` |
| `let x = 1; let x = x + 1;` | `SPX-T209` | No shadowing; pick a new name |
| assignment to an immutable binding | `SPX-U101` | Declare it with `let mut` before assigning |
| `fn main() -> bool` | `SPX-T104` | `main` returns `i64`; `0` conventionally means success |
| a second `consume(b)` after `own` | `SPX-O101` | Take `borrow` in the callee or pass a fresh value |
| `struct`, `enum`, `pub`, `const` | `SPX-P104` | `record`, `variant`, no visibility keyword, a function returning the value |
| `x: i64` as the last field without `,` | `SPX-P106` | Every field and every match arm ends with `,`, including the last |
| `x += 1;` | `SPX-P201` | `x = x + 1;` |
| `fn f()` or `-> ()` | `SPX-P106`, `SPX-P105` | Every function returns `i64` or `bool`; there is no unit |
| `a[0]` | `SPX-P106` | `byte_get(array_as_slice(a), 0usize)` returns `Option<u8>` |
| `Some(1)`, `None` | `SPX-T203`, `SPX-T202` | `Option<i64>::Some { value: 1 }`, `Option<i64>::None {}` |
| `s.len()` on a `string` | `SPX-T203` | `string_len(s)`; no type but a `class` has methods |
| `str_as_bytes(text)` when `text: string` | `SPX-T263` | Borrow first with `str_as_bytes(string_as_str(text))` |
| `string_as_str("literal")` | `SPX-T266` | Bind the literal, then pass that binding to `string_as_str` |
| `String`, `int`, or unsupported `Vec` inference/element types | `SPX-T001`/`SPX-T281` | `string`, `i64`/`i32`/`u8`/`usize`; in a Project prefer the authenticated `std.collections` aliases, and always spell an admitted Copy scalar plus every wrapper or `vec_*<T>` type argument explicitly |

### Bounded Vec traversal

Use `for item in values { body }` to borrow-traverse a simple immutable
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

## Projects

A project puts `semaprax.toml` beside `src/`. Use the extensible table layout
below. The committed examples' frozen, one-line-per-key
`semaprax.project.v1` layout also remains admitted:

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

Manifest bytes must be canonical: keep the shown table order, one blank line
between tables, one-line arrays, and no comments. A non-canonical manifest
fails with `SPX-J100`; its `help` line names the first differing line (for the frozen
one-line-per-key layout, the six lines in order); an unknown or reserved table
or key fails with `SPX-J120`. `[package] profile` selects the admitted consumer
profile. `[dependencies]` links packages from the compiler's closed bundled
`std.*` inventory at version `0.1.0`; unknown packages and unsatisfied ranges
fail with `SPX-J121`, while ordinary non-bundled packages still require the
separate resolution route. `[targets] matrix = ["wasm32"]` rejects native
builds with `SPX-J122`.

Import modules by stable identity, not by path:
`use function @id("calculator.add") from calculator.core as add;` directly
after the `module` line of the importing file; `entry` names the one module
that declares `main`. Project v1 function parameters and results are limited
to Copy scalar values. Records, classes, variants, `Option`, and `Result` may
be used as module-local implementation details inside scalar-signature
functions, but cannot cross a function boundary; `SPX-G174` points at a
declaration whose signature leaves that profile. A test module is an ordinary module whose `main` returns
`0` on success; `semaprax test semaprax.toml` prints `project tests passed`.
Give each check its own `fn test_<name>() -> i64` with an `@id` in the test
module: every such zero-parameter function runs on its own and a failure is
reported by stable id and outcome (`failed calculator.tests.test_add: returned
2`), with `cases` in the `--json` envelope. A violated `requires` or `ensures`
reports the function, the clause, and the argument values (`contract: requires
right != 0 in calculator.divide` / `arguments: left = 1, right = 0`).
[Project Test Cases v1](PROJECT-TEST-CASES-V1.md) owns both.
The [standard library catalog](STANDARD-LIBRARY-CATALOG.md), printed offline
by `semaprax help library`, lists every `std.*` function with its contract,
required project profile, and exact `[dependencies]` route. Add the dependency
to the table manifest and import the function by its `@id` as above; an
installed compiler supplies the bundled package without a repository checkout.
Bounded Vec uses profile `owned-data-api.v1` and
`std.collections = "^0.1.0"`. Import `std.collections.vec.*` by stable identity
with an explicit Copy-scalar type argument. Mutators transfer and return the
owner; the package has no public exports or stable generic ABI.
For one API, prefer
`semaprax help library <module|name|stable-id>`: the exact lookup prints only
the matched stable identity, dependency row, required profile, signature,
effects, and contracts. It does not do fuzzy or prefix search. The guarded
`std.core.compare` result is 226 bytes and 68 lexical units, with guarded
ceilings of 512 bytes and 128 units; both measures are more than 50 times
smaller than the 22,076-byte, 6,662-unit full catalog.
[Package Manifest v1](PACKAGE-MANIFEST-V1.md) owns the table layout,
[Project Manifest v1](PROJECT-MANIFEST-V1.md) the frozen one,
[examples/calculator-project](../examples/calculator-project/semaprax.toml) is
the committed instance, and `semaprax project-scaffold --name <name>` prints a
complete scaffold to stdout without writing files.

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

- [RFC 0001](RFC-0001.md): language and toolchain contract.
- [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md): records, variants, generics,
  matching, `Option`, `Result`.
- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md): ownership and cleanup.
- Bounded references for [explicit mutation](EXPLICIT-MUTATION-V1.md),
  [field mutation](FIELD-MUTATION-V1.md), [while loops](WHILE-LOOPS-V1.md),
  [bounded Vec `for` traversal](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md),
  [refutable match](REFUTABLE-MATCH-V1.md), [string operations](STRING-OPS-V1.md),
  [owned string views](OWNED-STRING-BORROWED-VIEW-V1.md),
  [indexed byte data](PORTABLE-INDEXED-BYTE-DATA-V1.md),
  [command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md), and
  [class inheritance](CLASS-INHERITANCE-V1.md).
- [Using the SEMAPRAX CLI](CLI-GUIDE.md) for every command's scoped help.
