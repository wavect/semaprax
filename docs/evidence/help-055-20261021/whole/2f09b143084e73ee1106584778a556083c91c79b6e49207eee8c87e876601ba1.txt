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
   For libraries, start with `semaprax help library`, then request an exact
   module, name or stable ID. Search the guide with `semaprax help language
   find:<word>:0` and request the next page only when needed. Use `help all`,
   `help language all` or `help library all` for an explicit full export.

The authored source-only `fused-construction-v1` opt-in uses
`run <file> --profile fused-construction-v1 --managed-bytes N` or
`build <file> --profile fused-construction-v1 --managed-bytes N --target native|wasm`.
It requires a finite explicit caller account, keeps ordinary admission closed,
and awaits executable qualification. SPX-F130 identifies the failed shape rule
at its source location; legacy counts of at most 32 rows use the ordinary route
without changing requirements. See [its versioned contract](FUSED-SOURCE-PROFILE-V1.md)
for the exact shape, target accounting, fuel and publication boundary.

`fmt` and single-file `patch` preserve `//` comments. Workspace transactions
do not promise that, so put durable intent in stable `@id` names, contracts,
and tests.

A workspace `SPX-G171` can refuse its static admission forecast or a later
live compiler acquisition. Read the reported boundary before restructuring source:
passing the forecast does not prove the live compiler fits. Preserve the required
behavior and existing caps. A smaller reachable program can reduce pressure when
it still meets the requirements; a compiler allocation or traversal improvement
can also reduce the same ledger usage. If the required program still refuses,
retain its exact source, compiler revision and complete diagnostic for a compiler
issue. Requested/remaining bytes identify the first refusal, not the total bytes
needed to make the program fit. Repeatedly splitting the same reachable source
into more modules does not by itself reduce a summed source forecast.

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
- Return types have no ownership qualifier: write `-> string` or `-> Vec<T>`,
  never `-> own string` or `-> own Vec<T>` (`SPX-P106`). In function signatures,
  `own` and `borrow` qualify parameters; the result type determines ownership.
- A function body has zero or more statements (`let`, assignment, `if`,
  `while`, `for`, `unsafe`) and one final expression. That expression supplies the block's
  value. Intermediate Copy scalars admit `expr;`; other discarded results
  require a binding (`SPX-T315`). The final value has no `;`.
  A function may also end with `return value;`, or use a returning
  guard `if condition { return value; }` before its remaining statements.
  A terminal `match` may use block arms that explicitly return on every path.
  These [acyclic returns](ACYCLIC-GUARD-RETURNS-V1.md) normalize to ordinary
  value `if`s and `match` arms. The separate [lexical exits](LEXICAL-STRUCTURED-EXITS-V1.md)
  profile admits `break;`, `continue;` and ordinary `return value;` as direct
  function/loop/control-block tails; executable qualification is pending.
  Use `if skip { continue; };` or `if done { break; };` inside a loop.
  With a valued `else`, `if skip { continue; } else { 0 }` may end with or
  without `;` when another statement or the tail follows; `fmt` drops the `;`.
  A final `;` before `}` remains `SPX-P106`; a next item starting with an
  operator or `(` keeps `SPX-T312`, so bind that item with `let`. Wrapping an
  exit guard in `let _ =`, or extending it with an operator, crosses a value
  boundary and remains `SPX-T312`. An exit guard
  with a normal arm uses exact `else { 0 }`; a nonzero, bool, owning or
  borrowed normal arm is also `SPX-T312`.
  An unconditional exit ends its block: do not add a dummy tail or unreachable
  effects. Continue advances an admitted source `for` exactly once; break/continue
  target only the nearest loop (`SPX-T313` otherwise). Exit crossings through
  initializers, arguments, aggregate fields, return operands, closures, unsafe or
  yields functions refuse (`SPX-T312`). Return must keep the declared result type
  and owning transfer (`SPX-T103`/`SPX-O104`). Only Copy scalar `expr;`
  statements are admitted; owning, aggregate and borrowed discards
  (`SPX-T315`), tuples and public unit/bottom values remain closed. See
  `help language author:lexical-exits`.
  A direct `match ...;` statement may carry a checked lexical exit when its
  normal result is Copy scalar. Binding that match with `let` still crosses a
  value boundary and refuses. This source extension is pending qualification.
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
- `str`: `str"text"` is static borrowed UTF-8; or use `string_as_str(binding)`.
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
  condition controls repetition. While bodies admit Copy-scalar operations,
  string literals and `string_*` calls. User calls declare read-only input
  effects, take Copy scalars or flat Copy variants, borrowed byte slices, named `str`, exact compiler-owned
  `borrow Vec<T>` for Copy-scalar `T`, or consumed strings, and return scalars,
  flat Copy variants or strings. Each iteration releases its strings. Matches over
  Copy scalars or variants with only Copy scalar payloads may yield strings or
  flat Copy variants. Construct Copy-scalar variants directly, including match
  scrutinees. [Copy record collections](COPY-RECORD-COLLECTIONS-V1.md)
  add checked flat Copy record construction, projection and private by-value
  helpers. Other aggregates need admission; moving an outer owner
  remains `SPX-T252`. Conditions settle temporary
  Strings before selection ([condition lifetime](STRING-CONDITION-LIFETIMES-V1.md)).
  Copy variant guards admit checked bool calls/blocks and case/wildcard/or
  patterns, require exhaustive unguarded fallback, and cannot consume an outer
  owner ([guard profile](GENERAL-LOOP-MATCH-V1.md)). Use canonical `vec_*<T>`
  operations inside loops; imported generic aliases retain `SPX-T252`.
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

- Generic templates reject lexical exits (`SPX-T226`): move `break`/`continue`/early `return` bodies to a monomorphic function. Admitted scalar/collection loops stay valid.

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

- The last field or case may omit its `,`; `fmt` writes it.
- Give every field and case its own `@id`. Cases without payload are
  written `Name,` in declarations and `Type::Name {}` elsewhere;
  the `{}` may be omitted (`Type::Name`) and `fmt` writes it back.
- Construct a generic variant as `Option<i64>::Some { value: v }`; match it
  as `Option::Some { value: v } => …`. `Some(v)` is invalid. Generic calls
  spell type arguments, as in `identity<i64>(4)`.
- If a Copy generic rejects `usize` or `u8` (`SPX-T223`), carry the value
  as `i64` and use the named integer conversions. `usize` must be at most
  9223372036854775807 (`i64::MAX`) to round-trip; larger values fail conversion.
- `record … with { field: value }` is immutable update. Record construction
  must name every field (`SPX-T213`).
- Owned String/Bytes records, whole-subtree binding, finite record loops and
  Copy/owned-leaf record Vecs each have an exact admitting profile. Pending
  features remain pending; see `help language detail:records`
  and `help language author:copy-record-vec` / `author:owned-data`.

- Import every type you name with explicit `use type`, including nested types,
  factory results, annotations, patterns and constructors (`SPX-G172`). See
  `help language detail:imports` for complete visibility rules and an import example.

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

`SPX-H006` loan-work refusal names the function: split into helpers, not files
or higher limits. For loop named-slice refusals, bind the view before the
helper call and pass its name; keep its owner alive through last use.

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

## Static borrowed UTF-8 literals

Use adjacent `str"..."` for immutable `borrow str`: `let key = str"é\u{0}";`
then `let view = str_as_bytes(key);` gives three exact UTF-8 bytes. Named
immutable aliases and synchronous borrowed-text calls follow existing loans;
literal evaluation shares module-held bytes without an owning String constructor.
Ordinary `"..."` still owns a String; use `string_as_str(owner)` where the existing
String profile admits it. Neither form implicitly converts to the other.

```semaprax
module app.static_text;

@id("app.measure")
fn measure(text: borrow str) -> i64
{
    str_len_bytes(text)
}

@id("app.main")
fn main() -> i64
{
    let key = str"é\u{0}";
    measure(key)
}
```

Empty, NUL, Unicode and escapes preserve decoded bytes. There is no interpolation,
mutable alias, aggregate storage, borrowed return or view escape. Each static
literal is bounded to 65536 decoded bytes (`SPX-T314`); selected HIR also bounds
the combined represented static/owned literals and format templates to 65536
(`SPX-H006`), without a deduplication discount. Native/internal aggregate Core-Wasm
and authenticated command Project definitions are authored; template/Project
execution qualification is pending. Public scalar/text/data adapters, standalone
String arenas and selected-final-UTF8 certificates refuse the static root.
See `help language author:static-utf8` and the
[owning contract](STATIC-BORROWED-UTF8-LITERALS-V1.md).

## Strings and bytes

`str_byte_at(text,index)` returns `Option<u8>`; match `Some`, then `i64_from_u8`.
`std.bytes.get_or`: `useful-data.v1` (`help library std.bytes.get_or`).

Use `string_format` for literal templates: `"id={}"`. See
`help language author:literal-format` for fields, scope and pending gates.

String parameters own their input implicitly: `text: string` transfers ownership;
`text: own string` is `SPX-O002`.
Read-only helpers take `text: borrow str`; pass `string_as_str(text)` before consumption.

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
- `string_take(name)` explicitly forwards a live named owned String and retires
  that binding; ordinary owning reads still clone. End overlapping views first.
  Computed/projected values and contracts are refused.
  See [named String transfer](OWNED-STRING-TRANSFER-V1.md) and
  `help language author:string-take`; owning qualification is pending.
- Byte functions take `borrow Slice<u8>`. Get one from `str_as_bytes(view)`,
  `array_as_slice(array_binding)`, or `bytes_as_slice(bytes_binding)`.
- `string_from_utf8(input: borrow Slice<u8>) -> string` (`core.string.from_utf8`)
  copies strict UTF-8 into a detached owner, preserving NUL/BOM/noncharacters
  without normalization/replacement. Malformed input: `semaprax.convert.v1` code 1.
  Public ABI, capabilities and closed adapter admission remain unchanged.
- `bytes_zeroed_bounded(requested, 131072usize)` allocates exactly `requested`
  bytes. Operand two must be a literal <=131072 (`SPX-T271`); analysis charges
  its full bound. Overbound requests fail before allocation:
  `semaprax.byte-buffer.v1` code 2 (`capacity_exceeded`). Bind before same-owner
  filling; allocate outside bounded loops. Physical exhaustion remains fail-stop.
- Buffer chains: `bytes_zeroed` requires a literal `usize` capacity;
  `bytes_set` takes the prior link, `usize` index, and byte. A bound chain
  freezes for borrowed reads.
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
- Single-file `run` interprets `main`; `--native` selects C11. `--json`,
  `--max-steps` and `--max-bytes` bound reports/execution. Exact
  `process.stdout.write` selects transcripts (above: `banana!0`). Args/file/stderr
  effects select the CLI route; `stdin_read` needs native `useful-data-command.v1`.
- Network and filesystem calls require exact capabilities and an injected
  provider. See `help language detail:io` for complete I/O rules.

- Interpreter/native recursion caps at 256 frames: overflow reports runtime
  capacity without language status/process signal. Raw Wasm traps at its engine stack limit.

## Compiler-owned functions

| Function | Signature |
| --- | --- |
| `string_len`, `string_len_chars` | `(s: string) -> i64` bytes / scalars |
| `string_is_empty` | `(s: string) -> bool` |
| `string_take` | `(name: string) -> string`; explicit named-owner transfer, qualification pending |
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
| `bytes_zeroed_bounded` | `(requested: usize, bound: usize) -> Bytes` exact dynamic length, literal bound |
| `bytes_set` | `(b: own Bytes, i: usize, v: u8) -> Bytes` write-once chain |
| `bytes_as_slice` | `(b: borrow Bytes) -> Slice<u8>` |
| `array_as_slice` | `(a: borrow [u8; N]) -> Slice<u8>` |
| `stdout_write`, `stderr_write` | `(v: borrow Slice<u8>) -> usize` |
| `args_len` | `() -> usize` |
| `arg_utf8` | `(i: usize) -> borrow str` |
| `stdin_read` | `() -> own Bytes` |
| `file_read` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` |
| `file_write_new`, `file_write_atomic` | `(path: borrow Slice<u8>, length: usize, data: borrow Slice<u8>, data_length: usize) -> usize` |
| `file_stat`, `file_create_dir`, `file_remove` | `(path: borrow Slice<u8>, length: usize) -> usize` |
| `file_list` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` |
| `box_new<T>` | `(value: T) -> Box<T>` for an explicit admitted Copy scalar |
| `box_get<T>` | `(value: borrow Box<T>) -> T` Copy read |
| `box_into_inner<T>` | `(value: own Box<T>) -> T` consumes |

`string_format("id={}", 3)` consumes String without JSON escaping.
Exact grammar/gates: `help language author:literal-format`.

Reserved `string_len`: `SPX-S113`.

Box boundaries: `help language detail:boxes` gives complete constraints.

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

Permits: `fs.read`, `process.args.read`, `process.stderr.write`; optional
`process.stdout.write`. `run lines.spx -- data.txt` supplies `arg_utf8`;
`--native`: native. `main`: `0..=255`, then output. `file_read_text`
stays below cwd; failure: one stderr line, exit 1.
[Text Toolkit][toolkit].

`SPX-T269`: direct writes stay outside loops and within profile capacity:
Default stdout/stderr: 65,536 combined bytes; v28: 1 MiB staged appends.
[Transcript rules][transcript],
[Project v7][v7].

Source-library CLI: `source-command.v1`, empty web exports, exact
`[command]` main ID, `argv-utf8+file-text.v1`, sorted capabilities, `native64`.
Web/Wasm/npm refuse; Project interpretation lacks argv/files. Tests: `semaprax test --target native`. [V26][v26],
[Tests][native-tests]. V28 `source-command.resource-output.v1`
keeps file quotas; Strings/authenticated text borrows/combined staged output: 1 MiB. [V28][v28].

Starter and library card:

```sh
semaprax new decimal-command --template source-command-file-text
cd decimal-command
semaprax help library std.int.decimal
semaprax check .
semaprax build --manifest-path semaprax.toml --target native --output app
./app digits
```

Pins `std.int.decimal = "=0.1.0"`; import `canonicalize`, `add`, `divide`
by IDs from its library card.

ASCII patterns: `[dependencies]` `std.pattern = "^0.1.0"`, `owned-data-api.v1`;
import `std.pattern.matcher`/functions by ID. `semaprax help library std.pattern`
gives imports/escapes/renewal. One `Matcher`: `compile` before observers;
pass its returned owner to `full-match` on independent named inputs.
Check `result-valid` before packets; captures are input byte offsets.
Bounded ambiguous searches may refuse at the work limit; no general regex/Unicode.
Sorted optional `std/packages.json` `api`: catalogue/direct-import
conformance only; source visibility/verification still required.

Build `lines.spx`: fresh native `--output`, no `--profile`:
`text-toolkit-v1`/`internal-strings-v1` are Wasm/web exports. `SPX-I307` needs
fresh output or removing a confirmed own artifact; no overwrites.

V23: `argv-utf8+stdin-stream.v1`, `language-command-io.stream.v1`; native reuses
4096 bytes; `stdin_read()` stays snapshot. Open prefills; zero=EOF, short reads=chunks.
Open/Next: `process.stdin.read`; Eof/Chunk: pure named-reader inspection.
Open once/path outside loops. Readers: no constructor/generic/aggregate/public
ABI escape. End chunk borrows before Next (`SPX-T265`); exact acyclic
`own StdinReader -> StdinReader` helpers renew. [Stream][stream].

Native v27 starter: `semaprax new <dir> --template stdin-stream-data`.

Exit codes: `help language specifications`.

Pure single-file `run`: `semaprax.interpret.v1`, then internal Strings for owned
`string` parameters/results (`SPX-F102`). Both refuse: ordinary diagnostic.
Retry JSON `schema`: `semaprax.interpret.internal-strings.v1`. Permit-selected
command/stdout skip fallback. [Strings][internal-strings].

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

`nums.txt` lines `4`, ` 5 `, `x`, `10`: `lines: 4`, `sum: 19`, exit 0;
no args: stderr usage, exit 2. Bind `arg_utf8(i)` before forwarding; copy with
`string_from_str` for flags. Loop `string_to_i64` cases:
`Option::Some { value }`/`Option::None {}`. Loops admit multiple `if`s, `&&`/`||`, `match`es. Bytes: `string_byte_at(s, i) == 32` checks space allocation-free.

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

`vec_of<T>(items)` uses one explicit admitted element type and exact capacity;
`vec_of<T>()` is empty. Items run once left to right, with one ordinary push
commit each. Existing bounds, failure cleanup and String clone rules stay.
`vec_of` is reserved (`SPX-P220`); generated owner capture/forgery refuses
(`SPX-T318`). See `help language author:vec-constructor`; qualification pending.

```semaprax
module app.constructor;

@id("app.main")
fn main() -> i64
{
    let values = vec_of<i64>(19, 23);
    if vec_len<i64>(values) == 2usize { 42 } else { 0 }
}
```

`vec_sort<T>(values)` consumes and returns a Copy-scalar vector in ascending
order; use `values = vec_sort<T>(values)` for a mutable binding. Capacity and
length stay unchanged. Floats use IEEE total order, including signed zeros and
NaN encodings. Owned payloads such as Bytes are rejected.
[Copy Scalar Sort v1](COPY-SCALAR-SORT-V1.md) owns the operation.

A list of Copy scalars is a `Vec<T>`. Every `vec_*` call spells its element
type. Borrowing a vector requires a named carrier: use `let values = make();`
then `vec_len<T>(values)`, rather than `vec_len<T>(make())` (`SPX-O118`).
Build with `let mut`, then move the finished vector into an immutable
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
and consuming traversal remain outside this `for` form. Statement-tail
`break`/`continue` use the separate, qualification-pending
[lexical profile](LEXICAL-STRUCTURED-EXITS-V1.md); `continue` advances once.
See [Owned Bounded Vec For Traversal v1](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md).

`for byte in view { body }` visits an immutable named `Slice<u8>` in
ascending order, with a fresh immutable `u8` item. Name a view from
`str_as_bytes(text)`, `bytes_as_slice(buffer)`, `array_as_slice(array)`, or
`byte_range(view, start, end)` first. The compiler keeps the length snapshot,
checked byte read and loop counter; no buffer copy is needed. The underlying
owner stays frozen for the whole traversal, including through subrange aliases.
`continue` advances once; `break` and `return` leave without advancing.
Nested immutable byte loops may use views already named before the outermost
byte loop. Each ancestor owner stays frozen until its own loop ends. Computed
or mutable iterable views, inside-body view declarations, item mutation/shadowing
and mixed source-for nesting remain refused. Ordinary safe while/Vec operations
keep their existing rules. Executable qualification is pending; see
[Borrowed Byte Slice For Traversal v2](BORROWED-BYTE-SLICE-FOR-TRAVERSAL-V2.md).

`for borrow row at index in rows { body }` is a scoped row view; `at index`
is optional. Qualification remains pending. See `help language detail:record-traversal` for complete row and loan rules
and `help language author:record-traversal`.

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

`vec_of` requires one explicit element type.

<!-- expect: SPX-P220 -->
```semaprax
module bad.constructor;

@id("bad.main")
fn main() -> i64
{
    let values = vec_of(1);
    0
}
```

<!-- expect: SPX-T312 -->
```semaprax
module bad.exit_guard;

@id("bad.main")
fn main() -> i64
{
    let mut index = 0;
    while index < 2 {
        index = index + 1;
        if index == 1 { continue; } else { 7 };
        0
    }
    index
}
```

An exit guard is a statement boundary. Write `else { 0 }` for its normal
arm; move useful work into the following statements. Binding the guard or
adding an operator still crosses a value boundary.

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
    return;
}
```

A return needs a value: write `return 42;` or use the tail expression `42`.

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

A `while` body needs a final expression: add `0` as its last line. The
condition before the body controls repetition; the body value is discarded.

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

Two admitted borrowed `str` views use `a == b` or `a != b` for exact UTF-8
byte equality, including NUL. This does not clone their owners or normalize
Unicode. Keep both roots live; mixed Str/String and Str ordering remain
refused. [Borrowed Str Equality v1](BORROWED-STR-EQUALITY-V1.md) has authored
backend gates; current-head qualification is pending.

Use `u8_from_i64(value)` then `char_from_u8(byte)` for byte conversion; use
`char_from_i64(value)` for arbitrary Unicode code points.

## Habits from other languages: diagnostic index

| You wrote|Code|Fix|
| ---|---|---|
| vec_of missing type/reserved name/oversized expansion|SPX-P220|Use vec_of<T>(items), one admitted type and existing bounds; vec_of is reserved|
| forged constructor/generated owner capture|SPX-T318|Reparse canonical vec_of source; use an explicit Vec chain to read intermediate owners|
| native cleanup refusal postcheck|SPX-B104|Check cleanup; String-condition scalar match “parent is not canonical”: backend bug|
| for range|SPX-P106|while; mut counter; discard tail|
| return; without a value|SPX-P106|return 42; or the tail expression 42|
| assignment loop|SPX-P203|Scalar tail, e.g. 0|
| local named result|`SPX-S109`, `SPX-T201`|Rename binding/uses to outcome; result denotes return only in ensures|
| owned/aggregate/borrowed statement result|SPX-T315|Bind explicitly; intermediate Copy scalar expr; admitted|
| tuple|SPX-P106|Declare a record|
| Option::Some { value: 1 }|SPX-T221|Option<i64>::Some { value: 1 }|
| index + 1 when index: usize|SPX-T208|Default i64; index + 1usize|
| unsuffixed i32|SPX-T232|Suffix: 5i32|
| 4i64|SPX-P003|Write 4 (default i64); suffixes: i32/u8/usize|
| i64 max+1 / parenthesized MIN negation|SPX-P003|Literal: -9223372036854775808/-2147483648i32; spaces trivia, parens separate; -MIN/MIN÷-1 overflow|
| "a" + "b"|SPX-T250|string_concat("a", "b")|
| literal/String as str arg|SPX-T205|Static str"text"; or bind String and pass string_as_str(s)|
| exit guard has nonzero/owning/view normal arm or is bound|SPX-T312|Bare statement guard; exact else { 0 }; move useful work after it|
| conversion type error|SPX-T205|i64_from_f64(3.0) or usize_from_i64(1)|
| excess conversion args|SPX-T204|One: f64_from_i64(1)|
| float Map key/Set element|SPX-T274|Keys string/i64/bool; values String/Copy|
| implicit helper ownership|SPX-O001|own/borrow; results move|
| String/collection record|SPX-T309|IDs; acyclic monomorphic; own/borrow; no invariants|
| record method|SPX-T203|get(point) or class; no record methods|
| shadowed binding|SPX-T209|Fresh name; `_` is an ordinary binding|
| assign immutable|SPX-U101|let mut|
| bool main|SPX-T104|i64; CLI 0 succeeds|
| reuse after own|SPX-O101|Callee borrow or fresh value|
| struct/enum/pub/const|SPX-P104|record/variant; no visibility; return values|
| missing arm comma|SPX-P106|Arm commas; last field/case comma optional|
| compound assign|SPX-P201|x = x + 1|
| ternary expression|SPX-P106|if c { a } else { b }|
| break / continue|SPX-P106|Exit test in while|
| x as i64|SPX-P106|Named checked conversion/suffix|
| Rust/JS closure|SPX-P201|fn(x: i64) -> i64 { x + 1 }|
| use std::io;|SPX-G170|Built-ins need no import; std.*: Project dependency + stable-ID import|
| resolved-core builder ledger refusal|`SPX-G171`|Preserve behavior/caps; record source, requested/remaining/floor and compiler stage. Reduce source or optimize its compiler owner. Ledger bytes are not RSS/peak; do not infer unknowns.|
| factory import lacks exact type|SPX-G172|Directly import exact/nested exposed type identities (help); inference grants no import authority|
| noncanonical Project source|SPX-G170|semaprax fmt <manifest>; blocked discovery: semaprax fmt --manifest <manifest>, then retry|
| interpreter: source-command.v1/v28 Project|SPX-F102|Interpreter lacks argv/files; semaprax build <manifest> --target native -o <fresh-path>; run in Project dir. Tests: semaprax test <project> --target native [Tests](PROJECT-NATIVE-TEST-V1.md)|
| explicit fused source account or selection refused|SPX-F130|Use the exact source-only fused-construction-v1 shape with main() -> i64 and explicit managed bytes 1..=1073741824; refusal has no ordinary fallback|
| output exists|SPX-I307|Fresh --output; no overwrite; remove only confirmed own artifacts|
| f()? in main|SPX-T218|Result-only; main matches|
| array literal|SPX-T262|Byte arrays; Vec<i64> otherwise|
| fn f() or -> ()|`SPX-P106`, `SPX-P105`|Result type required; no unit|
| a[0]|SPX-P106|byte_get(array_as_slice(a), 0usize) (Option<u8>)|
| Some(1), None|`SPX-T203`, `SPX-T202`|Option<i64>::Some { value: 1 }, Option<i64>::None {}|
| s.len() on string|SPX-T203|string_len(s); only classes have methods|
| str_as_bytes/nested string_as_str|`SPX-T263`, `SPX-T266`|Named String: str_as_bytes(string_as_str(text)); bind loop-carried slices beforehand|
| Bytes allocation capacity: loop/cycle, per-value or full-path bound|SPX-T267|Follow the diagnostic's named limit; reduce complete allocation/input-read demand or per-value capacity. Keep Bytes allocation/copy and allocating calls out of ordinary while/cycles; nonallocating updates may fill a live owner where equivalent. Drops do not refund cumulative limits. Separate finite scratch/fused routes need their own proof/profile and qualification; help grants no admission. [Bytes](OWNED-BOUNDED-BYTE-BUFFER-V1.md)|
| direct output repeats per path / is loop-reachable|SPX-T269|Build bounded output; publish once outside loops/cycles; default stdout+stderr ≤65536 bytes; v28 direct appends ≤1 MiB|
| string_as_str("literal")|SPX-T266|let s = "literal"; string_as_str(s)|
| payload/generic ==|SPX-T207|Match; == only payload-free/nongeneric variants|
| invalid/duplicate pattern binding|SPX-M104|Fresh unique local; result is reserved; record/variant payloads: field: fresh_alias; keep field names unchanged|
| payload in or-pattern|SPX-M105|Payload-free only; split payload arms|
| String/int/invalid Vec|`SPX-T001`/`SPX-T281`|String/scalars; explicit Copy Vec<T>; authenticated imports|
| bad [modules]|SPX-J100|2–16 sorted sources; one bounded nonentry test module: entry="app", sources=["a.spx","b.spx"], tests=["app.tests"] [Manifest](PACKAGE-MANIFEST-V1.md)|
| generic while call|SPX-T252|vec_len<T>; imported generic aliases closed [While](WHILE-LOOPS-V1.md)|
| rejected while helper|SPX-T252|Borrow compiler Copy-scalar Vec<T>; return scalar/flat Copy variant/string|
| outer owner changes in while|SPX-T252|Keep outer ownership|
| Bytes+usize renewal/input views|`SPX-T252`/`SPX-T265`|Pure nongeneric call; whole same-type owner return; whole named independent Slice/str borrows [renewal hook](IO-LINES-V1.md#cursor-transitions), executable gate pending|
| Vec capacity >8192|SPX-T282|Reduce vec_with_capacity<T>; Vec cap [Vec](OWNED-BOUNDED-VEC-V1.md)|
| lookalike Vec wrapper|SPX-T283|Exact std.collections.vec.* ID only [Vec](OWNED-BOUNDED-VEC-V1.md)|
| function: >256 shared loans|SPX-H006|Fewer loans; fixed cap [Loans](SHARED-LOAN-PLAN-V1.md)|
| function: >4096 loan points|SPX-H006|Simplify; admitted helpers|
| function: >4096 CFG edges|SPX-H006|Simplify; admitted helpers|
| loan work >1000000|SPX-H006|Less work; fixed cap|

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

`semaprax.toml` beside `src/`; canonical tables or frozen one-line-per-key v1.

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

Tables: ordered sections, blank lines, one-line arrays, no comments.
SPX-J100: first canonical mismatch. Frozen v1: six ordered lines.
Unknown/reserved: SPX-J120. `[package] profile`: boundary carriers.
Bundled std.*: 0.1.0; unknown packages/ranges: SPX-J121; separate resolution.
`[targets] matrix = ["wasm32"]`: native refuses, SPX-J122.
[Tables](PACKAGE-MANIFEST-V1.md); [frozen v1](PROJECT-MANIFEST-V1.md).

After `module`: `use function @id("calculator.add") from calculator.core as add;`.
`entry`: main module. V1: Copy scalars; profiles add carriers. SPX-G174:
Unsupported aggregates stay local or need an admitting profile.
H006, 4096 loan points: extract admitted helpers; other causes differ.

Test `main`: 0 succeeds; each `@id` fn test_<name>() -> i64 runs separately.
Failures: ID/outcome, contract clause/arguments. Projects interpret; v26/v28
need authenticated native roots: `semaprax test <project> --target native`.
[Bounds](PROJECT-NATIVE-TEST-V1.md); [cases](PROJECT-TEST-CASES-V1.md).

Start with `help library` for the module index; exact lookup:
`help library <module|name|stable-id>` (no fuzzy/prefix match).
Search: `help language find:<word>:0`, then pages.
Use `help library all` only for an explicit full offline
[catalog](STANDARD-LIBRARY-CATALOG.md) export. Import IDs/dependencies.
Vec: `owned-data-api.v1`, `std.collections = "^0.1.0"`, exact
std.collections.vec.* IDs, explicit Copy-scalar type args. Mutators transfer/return
owners; no public/generic ABI.

`new <dir> --template source-command-file-text`: v26 source-command.v1/native64,
argv-utf8+file-text.v1. Web/Wasm/npm refuse; interpreter run/test: SPX-F102.
Use native `semaprax test .`. `doctor --profile`: reports only. V28
source-command.resource-output.v1 keeps v26 ABI; 1 MiB Strings, borrowed
text/staged output: [limits](PROJECT-MANIFEST-V28.md).

`new <dir> --template stdin-stream-data`/`project-scaffold`: v27,
`language-command-io.stream-data.v1`; main/command fn() -> i64.
V27 admits immutable `borrow Vec<T>` for
all eight Copy scalars
(`i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, `bool`);
pure `main`/test closures may interpret without
authority, stdin provider, or command adapter. Web/Wasm/npm refuse v27.
`stdin-stream-text` stays v25
for private owned-String helpers. v24/v25 retain closed helper boundaries.
[V27](STREAM-DATA-COMMAND-V1.md).

V29: private Copy records/Vec<R>; v30 language-command-io.owned-data.v1:
private Vec<string>/flat owned-leaf records. Vec schema grants no carrier.
`text: string` consumes; `text: own string`: SPX-O002. Push consumes;
for own: vec_into_iter. Bytes allocation/copy or Bytes-record clone in while:
SPX-T267. `help language author:owned-data`,
[v29](STREAM-DATA-COMMAND-V2.md), [v30](STREAM-OWNED-DATA-COMMAND-V1.md).
[Owned-leaf gate](../examples/owned-leaf-command-project/README.md): pending.
For own renews outer Row Vec: `out=vec_push<Row>(out,item)`. Rederived
immutable-empty literal-index clone refusal skips unreachable copy charges;
no loop allocation/refund.
[Renewal](CONSUMING-OWNED-RECORD-VEC-RENEWAL-V1.md).

V31: private records/admitted Vecs, scoped field reads; qualification pending.
Details: `help language detail:field-views`;
`help language author:record-field-views`.

`vec_sort_by<Row>(rows,"priority:desc,arrival:asc")`: 1..8 unique literal keys,
one admitted flat monomorphic record (SPX-T311). Consumes Vec; stable
ties/payload owners. End loans first. Descending swaps operands (signed MIN safe).
vec_sort_owned keeps declaration order. [Sort](EXPLICIT-RECORD-SORT-KEYS-V1.md).

`vec_update_field<Row>(rows,index,value,"count")`: consume/renew Vec, write one
exact Copy scalar. Last arg: literal field name. Invariant-free flat
owned-leaf records only (SPX-T320); usize index, exact RHS type. End field loans. No leaf clone/capacity change. Gate pending:
[contract](OWNED-RECORD-SCALAR-FIELD-UPDATE-V1.md), `help language author:update-field`.

`vec_unique_by<Row>(rows,"id")` -> bool: duplicate check on one literal String or
i64/i32/u8/usize/bool/char field; named own/borrow record Vec (SPX-T319).
Borrows rows; preserves keys/order/generation. Floats, Bytes, paths
and temporaries refuse. [Uniqueness](BORROWED-RECORD-FIELD-UNIQUENESS-V1.md).

`lock semaprax.toml --write`: identity/source/interface/targets/capabilities;
--verify checks; --compare <base.lock>: breaking interfaces (CI nonzero).
Ranges: ^/~/=. `resolve semaprax.toml --target native64 --cache <dir> --write`
pins targets; --verify checks. Build links no dependencies.
[Lock](PROJECT-LOCK-V1.md); [resolution](PROJECT-DEPENDENCY-RESOLUTION-V1.md).

## JSON documents and cursors

Strict JSON: `std.data.json.scan = "^0.1.0"`, profile `useful-data.v1`.
Call `strict_end(input, 32usize, policy)` by ID first: 0 allows duplicate names,
1 rejects decoded duplicates. Success = input length; larger values encode
error offset. Navigation/numbers borrow bytes. `std.data.json.query`
decodes strings. See [scan](STRICT-JSON-SCAN-V1.md), [query](JSON-STRING-QUERY-V1.md).

`json-codec` derives checked source from authenticated Project records.
`author:json-owned-request`: `owned-request.v1`/`stream-owned-request.v1`;
≤8 `Vec<string>` identifiers, ≤256 `Vec<Row>` rows. Stable-ID flat Row: one
String identifier, 0–6 `i64`/`u8`/`usize`/`bool` fields, each with an ID,
in any order. ASCII identifiers are unique, 1–16 letters/digits/`_`/`-`;
nonempty rows need an identifier. Encoders borrow vectors, accept `output_limit`.
`json_<Request>_owned_decode` returns independent Strings: release normalized
Bytes after decode. Stream errors use raw offsets before Ready, normalized
offsets afterwards. Owned runtime is private `owned-data-api.v1` or native v30; v29 Copy views
retain source lifetime. Qualification/#724 pending.

Typed source: `author:json-request-response`, [ShiftSim](../examples/shiftsim-typed-record-successor/README.md).
Combined candidate:

```sh
cd examples/shiftsim-typed-record-successor
semaprax json-codec . --plan codec-plan.json --output codecs.candidate.json
```


Single-codec profiles:

```sh
cd examples/shiftsim-typed-record-successor
semaprax json-codec . --source src/request.spx --type shiftsim.request \
  --output src/request.generated.spx --profile stream-owned-request.v1
semaprax json-codec . --source src/response.spx --type shiftsim.report \
  --output src/response.generated.spx --profile bounded-collection-response.v1 \
  --max-string-bytes 16
```

For single codecs, derive both outputs from the unchanged bootstrap; replay each
against that revision with exact path/record ID/profile/bounds before copying either output.
Installing one generated module first changes the revision; edits remove evidence.
CLI leaves source/manifests untouched. Tests: `standard_library::application_json::owned::catalog_scoped_records::`.

`author:json-nested-response`: `bounded-nested-response.v1` requires both
`--max-string-bytes 1..64` and `--max-array-items 1..256`. Acyclic records:
depth 8, ≤64 expanded paths, 1..8 fields, up to two expanded Vec fields of Copy scalars
or flat rows with ≤2 Strings. Primitive `Vec<string>` refuses SPX-J180 in v1.
Additive `bounded-nested-response.v2` reads those slots using
`vec_str_at(values, at)` within named `borrow Vec<string>` helpers; its
`nested_response_v2` APIs and `nested-response-v2` identities are distinct.
The same caps and independent caller profile/loan checks remain authoritative.
`json_Root_nested_response_encoded_len(value: borrow Root)` preflights all fields;
`json_Root_nested_response_encode(value: borrow Root, output_limit: usize)` returns
`Encoded { text }` or `Refused { required }`. `usize::MAX` signals string/array
or 131072-byte output bounds exceeded. Allocate output only after complete preflight,
in declaration order, without clones. Owners retain runtime bounds;
The request 264 new-String census does not limit this response selector.
No capability/profile/decoder/business mapping.
See [nested response v1](APPLICATION-JSON-NESTED-RESPONSE-V1.md) and
[String-vector response v2](APPLICATION-JSON-NESTED-RESPONSE-V2.md).

`author:collection-records`: v31 `Report { rows: Vec<Row> }`;
`author:nested-outcomes`: v32 nested success/error carriers. Import signature
roots (SPX-G172). [Private layouts](PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md) may be
inferred; authored child names need imports. Bind variant payloads with
`match own`; keep each match-arm comma **inside** its closing brace. Aggregate
results require an independently admitted finite collection outcome;
ordinary String/general variant joins do not gain that authority.

Stream Ready owns its payload: `Outcome::Ready { value } => match own value`,
then exact `OrderRequest { configuration, lines, urgent }`, then owning match
on configuration; do not retain a borrowed Ready byte slice. Consume owners once;
views stay within owner lifetime;
no dummy owners. Success/stderr/status:
`examples/stream-nested-order-json-project/src/app.command.spx`.
Normalizer `Ready { bytes, length }` passes exactly
`byte_range(bytes_as_slice(bytes), 0usize, length)` while bytes remains live.


## Detailed profile rules

Exact topics name `help language detail:<slug>` to retrieve one complete section
below. `help language all` retains this appendix and every compiler-checked
example; lookup does not change admission or qualification status.

### JSON profile details

One immutable authored revision per bundle. Project
admission checks before publishing original schemas,
candidate sources and exact receipts. Install through ordinary source/managed
authority; receipts grant none. See [codec plans](APPLICATION-JSON-CODEC-PLAN-V1.md).

Nonstream `utf8-owned-request.v1` requires canonical
`--max-string-bytes 1..64` decoded bytes; empty/duplicate/Unicode/NUL values
are allowed, arrays remain 0..8/0..256, including rows with no identifiers.
External borrowed input ≤65,536 bytes; internal owned Bytes views ≤131,072.
Source field names stay ASCII. See `author:json-utf8-owned-request`.

Decoded rows: `vec_field<Patient>(request.patients,index,"priority")`.
String borrows retain the named Vec through last use. Response encoding
of `Report { assignments: Vec<Assignment>, metrics: Metrics }` supplies no
request decoding/business mapping. Checked helpers; caller's admitted
Project profile controls runtime. Qualification pending.

### Record profile details

- Monomorphic acyclic records may own String/Bytes, Copy scalars and nested
  records. Pass with `own` or `borrow`; results own their leaves. Generic,
  resource/view/class and invariant-bearing String records are `SPX-T309`;
  see [String records](OWNED-STRING-RECORDS-V1.md).
- Whole-subtree binding (source implementation; qualification pending):
  `match own packet { Envelope { left, right, sequence } => sequence, }`
  moves each named record child without recursively unpacking it. Include every
  field exactly once; `_` cannot hide an owned subtree. The scrutinee must be a
  named unprojected owner with no live descendant loan. Complete hidden children
  still count toward limits; `match borrow` keeps recursive patterns.
  See [owned subtree patterns](NESTED-OWNED-RECORD-DESTRUCTURING-V2.md).
- Finite record loops (source implementation; qualification pending): an
  effect-free, parameter-free i64 function may use `let mut i = 0;` immediately
  before `while i < N`, with literal `N` in 1–32. Each body constructs one fresh
  immutable monomorphic record, then discards one owning record match and ends
  in `0`. Its single guard-free arm first increments that exact counter by one;
  branch tails may continue, break, return or complete normally. Extra arm
  statements, counter shadowing and nested loops refuse. Actual allocations
  still obey the existing 32-event/2 MiB limits. See
  [the exact admitted shape](FINITE-OWNED-RECORD-DISCARD-LOOPS-V1.md).
- Copy-record Vec admits identified flat records with 1–8 Copy fields. Project
  v30 adds `Vec<string>` and flat records with up to two direct owned
  String/Bytes fields. User records need `@id`; ask
  `help language author:copy-record-vec` or `help language author:owned-data`.
  See [Owned Leaf Collections v1](OWNED-LEAF-COLLECTIONS-V1.md).

### Network and filesystem details

- TCP: `net_connect`, `net_send`, `net_recv`, `net_stream_stdout`, `net_wait`,
  `net_close` need `permit`/`uses` of `network.connect`, `network.read`,
  `network.write` and an injected provider. Owned `net_recv` is refused inside
  `while` (`SPX-T270`). [Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md).
- `file_read` / `file_write_new` require explicit `fs.read` / `fs.write` and an
  injected [Filesystem I/O v1](FILESYSTEM-IO-V1.md) provider. Paths are bounded
  relative byte prefixes; writes create new files without overwriting.
  [Filesystem I/O v2](FILESYSTEM-IO-V2.md) adds stat, sorted immediate-name listing,
  mkdir, removal and atomic replacement. Only stat/list accept empty paths for
  the injected root. `std.fs` composes typed Path, FileInfo and Reader/Writer.

### Box profile details

Boxes allocate uniquely; authored `record Box<T>` stays inline. Owned payloads,
public generic ABI and shared ownership remain outside [Box v1](OWNED-BOUNDED-BOX-V1.md).

### Scoped record traversal details

`for borrow row at index in rows { body }` adds a scoped field-only row view
for an immutable named vector of an admitted Copy record or flat owned-leaf
record. Omit `at index` when no index is needed. Scalars copy; `row.text` and
`row.bytes` borrow their stored String/Bytes until the iteration ends. The whole
vector stays frozen through the loop, including Copy-only reads. A flat
owned-leaf row may also be lent whole, only as a direct argument to an ordinary
`borrow` parameter of its exact record whose effect-free helper reads just
Copy scalar fields and returns a Value Copy scalar; every other use of the row stays refused. Qualification
is pending; see `help language author:record-traversal`,
[Scoped Record Vec Traversal v2](SCOPED-RECORD-VEC-TRAVERSAL-V2.md) and
[Scoped Record Row Loan v1](SCOPED-RECORD-ROW-LOAN-V1.md).

### Earlier-row field queries

An explicit `vec_field<Row>(rows,index-1usize,"field")` may read an earlier row
inside the loop when guarded by `index > 0usize`; its usual bounds and loan
rules apply. `row.field` always reads the current row snapshot.

### Project field-view details

V31 language-command-io.collection-record.v1: private typed records/admitted Vec;
entry/command fn() -> i64. `vec_field<Row>(value.items,index,"field")`: live named
Vec path; `index` is `usize`; literal field (SPX-T310).
Copy returns by value; String/Bytes return nonescaping borrow str/Slice<u8>.
End views before moving/replacing/sorting/pushing/reserving/clearing roots;
temporary/call roots and escaped/stale views refuse.
`str_as_bytes(vec_field<Row>(value.items,index,"text"))` fuses reads;
`string_as_str(value.inner.label)` keeps the live record path. Avoid vec_clone_at
for queries: it clones owners. These reads apply only in v30/v31/v32, not public ABIs;
gate pending: [scoped reads](SCOPED-VEC-FIELD-READS-V1.md).

### Record import visibility details

- Import every type you name from a function signature, including nested
  types and factory results. A private inferred signature may retain hidden
  layout facts; it does not let you name that type, its cases or siblings in
  annotations, patterns or constructors. Use an explicit `use type` for those.
  With `std.pattern.make`, add
  `use type @id("std.pattern.matcher") from std.pattern as Matcher;`
  `SPX-G172` names the missing type identity.

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

[toolkit]: TEXT-TOOLKIT-V1.md
[transcript]: BOUNDED-STDOUT-TRANSCRIPT-V1.md
[v7]: PROJECT-MANIFEST-V1.md#additive-project-manifest-v7-line-command-profile
[v26]: PROJECT-MANIFEST-V26.md
[native-tests]: PROJECT-NATIVE-TEST-V1.md
[v28]: PROJECT-MANIFEST-V28.md
[stream]: BOUNDED-STDIN-STREAM-V1.md
[internal-strings]: INTERPRETER-INTERNAL-STRINGS-V1.md
[manifest]: PACKAGE-MANIFEST-V1.md
[while]: WHILE-LOOPS-V1.md
[renewal]: IO-LINES-V1.md#cursor-transitions
[vec]: OWNED-BOUNDED-VEC-V1.md
[loans]: SHARED-LOAN-PLAN-V1.md

The reviewed [Catalog paired plan](../examples/catalog-scoped-record-successor/codec-plan.json)
selects both request and response modules with their existing bounds. Derive the
whole bundle twice from the unchanged bootstrap and compare all bytes before
staging its complete source inventory, as in the
[installation workflow](../examples/catalog-scoped-record-successor/README.md).
A bundle is read-only evidence; it does not publish a workspace generation.
