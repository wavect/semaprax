# Agent quick reference

Status: public pre-alpha reference card. Every `semaprax` code block on this
page is a complete module that `tests/documentation.rs` checks against the
compiler: blocks without an `expect` marker must verify without diagnostics and
already be canonical; blocks with one must produce exactly that diagnostic code.

Audience: coding agents and their operators writing SEMAPRAX programs with a
bounded context window.

This page is the cheapest correct picture of the admitted language. It states
the shapes that compile today, the diagnostics that unfamiliar habits trigger,
and how to spend as few tokens as possible per edit-check cycle. It owns no
rule: [RFC 0001](RFC-0001.md) is the contract and the
[completion matrix](COMPLETION-MATRIX.md) is the status authority. The
[language tour](LANGUAGE-TOUR.md) explains the same shapes at length.

The installed compiler prints this page verbatim with `semaprax help language`,
so it is available without the source checkout.

## Spend tokens on source, not on dumps

- Edit loop: write the file, `semaprax fmt <file>` (rewrites canonically),
  `semaprax check <file> --json`, `semaprax run <file>`. One diagnostic per
  line with `code`, `message`, `location`, and `help`; stop on the first
  error's line and column.
- `fmt` deletes comments. Nothing you write after `//` survives, so put intent
  into `@id` names, contracts, and tests, not comments.
- Read the `.spx` source when it fits. On the committed calculator example,
  the source is 606 bytes, `semaprax graph` emits 24,419 bytes, and
  `semaprax context <file> app.main --depth 1` emits 2,279 bytes. `graph` is
  for tools that need cleanup plans and expression trees, not for orientation.
- For one declaration's callers, callees, contracts, or ownership across a
  file, use `semaprax context <file> <stable-id> --depth 1 --filters
  contracts,ownership --max-bytes 4096` and read `truncation` before trusting
  the answer. [Agent Context v2](AGENT-CONTEXT-V2.md) owns the schema.
- `semaprax --help` is a guided overview under 2 KB: the commands above,
  grouped by task, with one-line purposes. `semaprax help all` is the 7 KB
  exhaustive catalog; use `semaprax help <command>` for one command's exact
  grammar.
- Diagnostics carry stable `SPX-…` codes. Bind tests and repair logic to the
  code, never to the message text.

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
- Every declaration carries `@id("dotted.stable.name")`. Without it the
  compiler warns `SPX-S103`, and renaming the function changes its identity.
- The entry point is exactly `fn main() -> i64`. There is no other signature.
- A function body is a block: zero or more statements (`let`, assignment,
  `while`, `unsafe`) followed by exactly one tail expression whose value is the
  block's value. There is no `return`, no expression statement, no `for`, no
  `else if`, no tuple, and no unit value in user code.
- Canonical layout puts the function body's `{` on its own line and each
  statement on its own line; `if`, `match`, and record literals stay on one
  line. Let `fmt` do it.

## Scalars and literals

| Type | Literal | Notes |
| --- | --- | --- |
| `i64` | `42`, `-1` | Default integer type; overflow is a checked failure |
| `i32` | `42i32` | Suffix required, no implicit widening |
| `u8` | `255u8` | Bytes |
| `usize` | `3usize` | Lengths and indices; compare only with `usize` |
| `f64`, `f32` | `1.5`, `1.5f32` | |
| `bool` | `true`, `false` | `&&`, `\|\|`, `!` |
| `char` | `'a'`, `'\n'`, `'\u{2603}'` | |
| `string` | `"text"` | Owned UTF-8; `==` compares contents |
| `str` | none | Borrowed view: `borrow str` parameters or `string_as_str(binding)` |
| `[u8; N]` | `[97u8, 98u8]` | Fixed array; `array_as_slice(binding)` gives `Slice<u8>` |
| `Bytes`, `Slice<u8>` | none | Owned bytes and a borrowed byte view |

Operators never mix types: `n < 5` fails with `SPX-T208` when `n` is `usize`;
write `n < 5usize`. Strings do not support `+`; use `string_concat`.

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
- A `while` body ends with a `bool` tail that decides whether to loop again,
  normally the loop condition repeated.
- Bindings are immutable unless `let mut`. Assignment is a statement:
  `x = x + 1;` or `point.x = 5;`. Parameters are immutable.
- Contracts are `requires`/`ensures` lines between the signature and the body;
  `result` names the return value. They are checked at run time.
- Effects: the module lists `permit { … }`, and every function that performs
  or calls into an effect declares `uses { … }`. Missing `permit` is
  `SPX-E101`; a missing `uses` is `SPX-E102`.
- `match` on scalars needs a final catch-all arm (`_` or a binding) without a
  guard, else `SPX-T257`.

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

- Every field and case carries its own `@id`. Cases without payload are
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

- `own T` parameters consume the argument; a second use is `SPX-O101`.
  `borrow T` parameters read it. Resources declare `drop trivial;` or
  `drop import "host.symbol";`.
- The reference interpreter behind `semaprax run` rejects modules that declare
  resources with `SPX-B104`. Verify them with `check`, exercise them through a
  project's native or Wasm build, and keep interpreter-run examples free of
  `resource` declarations.

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
- `stdout_write(slice)` needs both `permit { process.stdout.write }` and
  `uses { process.stdout.write }` and returns the `usize` byte count.
- `run` evaluates in the reference interpreter. `args_len`, `arg_utf8`,
  `stdin_read`, and `stderr_write` need a project with the
  `useful-data-command.v1` profile built for the native target.

## Compiler-owned functions

| Function | Signature |
| --- | --- |
| `string_len`, `string_len_chars` | `(s: string) -> i64` bytes / scalars |
| `string_is_empty` | `(s: string) -> bool` |
| `string_concat` | `(a: string, b: string) -> string` consumes both |
| `string_starts_with`, `string_contains` | `(s: string, other: string) -> bool` |
| `string_from_char` | `(c: char) -> string` |
| `string_as_str` | `(binding: string) -> borrow str` |
| `str_len_bytes` | `(s: borrow str) -> i64` |
| `str_is_empty` | `(s: borrow str) -> bool` |
| `str_starts_with`, `str_contains` | `(s: borrow str, other: borrow str) -> bool` |
| `str_as_bytes` | `(s: borrow str) -> Slice<u8>` |
| `byte_len` | `(v: borrow Slice<u8>) -> usize` |
| `byte_get` | `(v: borrow Slice<u8>, i: usize) -> Option<u8>` |
| `byte_range` | `(v: borrow Slice<u8>, start: usize, end: usize) -> Slice<u8>` |
| `bytes_copy` | `(v: borrow Slice<u8>) -> Bytes` |
| `bytes_as_slice` | `(b: borrow Bytes) -> Slice<u8>` |
| `array_as_slice` | `(a: borrow [u8; N]) -> Slice<u8>` |
| `stdout_write`, `stderr_write` | `(v: borrow Slice<u8>) -> usize` |
| `args_len` | `() -> usize` |
| `arg_utf8` | `(i: usize) -> borrow str` |
| `stdin_read` | `() -> own Bytes` |

These names are reserved; declaring your own `string_len` is `SPX-S113`.

## Habits from other languages and what the compiler says

Each block below is what an agent typically writes first. The marker names
the diagnostic it produces; the fix is in the text after it. For parser and
source-verifier rejections the compiler prints the same fix as the diagnostic's
`help` line, and for the rest the message itself names the accepted form, so
act on the diagnostic before re-reading this page.

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

Other first-attempt diagnostics and their fixes:

| You wrote | Code | Fix |
| --- | --- | --- |
| `for i in 0..n { … }` | `SPX-P106` | Use `while` with a `let mut` counter and a `bool` tail |
| `f(x);` as a statement | `SPX-P106` | Discard it with `let _ = f(x);` or make it the tail |
| `let t = (1, 2);` | `SPX-P106` | No tuples; declare a `record` |
| `id(4)` for `fn id<T>` | `SPX-T225` | `id<i64>(4)` |
| `Option::Some { value: 1 }` | `SPX-T221` | `Option<i64>::Some { value: 1 }` |
| `"a" + "b"` | `SPX-T250` | `string_concat("a", "b")` |
| `f("abc")` or `f(owned)` for `borrow str` | `SPX-T205` | `let s = "abc"; f(string_as_str(s))` |
| `point.get()` on a record | `SPX-T203` | Records have no methods; call `get(point)` or use a `class` |
| `let x = 1; let x = x + 1;` | `SPX-T209` | No shadowing; pick a new name |
| `fn main() -> bool` | `SPX-T104` | `main` returns `i64`; `0` conventionally means success |
| a second `consume(b)` after `own` | `SPX-O101` | Take `borrow` in the callee or pass a fresh value |
| `struct`, `enum`, `pub`, `const` | `SPX-P104` | `record`, `variant`, no visibility keyword, a function returning the value |
| `x: i64` as the last field without `,` | `SPX-P106` | Every field and every match arm ends with `,`, including the last |
| `x += 1;` | `SPX-P201` | `x = x + 1;` |
| `fn f()` or `-> ()` | `SPX-P106`, `SPX-P105` | Every function returns `i64` or `bool`; there is no unit |
| `a[0]` | `SPX-P106` | `byte_get(array_as_slice(a), 0usize)` returns `Option<u8>` |
| `Some(1)`, `None` | `SPX-T203`, `SPX-T202` | `Option<i64>::Some { value: 1 }`, `Option<i64>::None {}` |
| `s.len()` on a `string` | `SPX-T203` | `string_len(s)`; no type but a `class` has methods |
| `String`, `int`, `Vec` as types | `SPX-T001` | `string`, `i64`/`i32`/`u8`/`usize`, `[u8; N]`/`Bytes`/`Slice<u8>` |

## Projects

A project is a `semaprax.toml` beside a `src/` directory:

```toml
schema = "semaprax.project.v1"
name = "calculator"
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["calculator.add"]
tests = ["calculator.tests"]
```

Modules import by stable identity, not by path:
`use function @id("calculator.add") from calculator.core as add;` at the top of
the importing file. A test module is an ordinary module whose `main` returns
`0` on success; `semaprax test semaprax.toml` prints `project tests passed`.
A failure prints only `project tests failed with result N`, so return a
distinct non-zero value from each failing check (`if a { if b { 0 } else { 2 } }
else { 1 }`) instead of a single `1`; the number is the only clue to which
check failed.
The [standard library catalog](STANDARD-LIBRARY-CATALOG.md) lists every
`std.*` function with its contract; to use one, copy its package's library
file from `std/` into `src/`, list it in `sources`, and import the function by
its `@id` as above.
[Project Manifest v1](PROJECT-MANIFEST-V1.md) owns the manifest,
[examples/calculator-project](../examples/calculator-project/semaprax.toml) is
the committed instance, and `semaprax project-scaffold --name <name>` prints a
complete scaffold to stdout without writing files.

## Where the rules live

- [RFC 0001](RFC-0001.md): language and toolchain contract.
- [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md): records, variants, generics,
  matching, `Option`, `Result`.
- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md): ownership and cleanup.
- Bounded references for [explicit mutation](EXPLICIT-MUTATION-V1.md),
  [field mutation](FIELD-MUTATION-V1.md), [while loops](WHILE-LOOPS-V1.md),
  [refutable match](REFUTABLE-MATCH-V1.md), [string operations](STRING-OPS-V1.md),
  [owned string views](OWNED-STRING-BORROWED-VIEW-V1.md),
  [indexed byte data](PORTABLE-INDEXED-BYTE-DATA-V1.md),
  [command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md), and
  [class inheritance](CLASS-INHERITANCE-V1.md).
- [Using the SEMAPRAX CLI](CLI-GUIDE.md) for every command's scoped help.
