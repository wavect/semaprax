# Language tour

Audience: developers learning to read and write SEMAPRAX.

Status: alpha tour of committed examples, not a readiness claim. See the
[completion matrix](COMPLETION-MATRIX.md) for supported features and
[RFC 0001](RFC-0001.md) for exact language rules.

> Prefer task-oriented guides? The user-facing [Semaprax Handbook](../handbook/README.md)
> teaches the same language through best-practice pages starting at
> [Essentials](../handbook/language/essentials.md). This tour remains the
> verbatim, compiler-checked example walkthrough.

Start with a program, then learn persistent IDs, contracts, mutation,
ownership, effects, and the semantic graph. Each section links to the exact
rule in its owning specification.

## How to read this tour

Every SEMAPRAX block is a verbatim excerpt from the linked example file.
`tests/documentation.rs` checks that excerpts stay in sync.

Run commands from the repository root. Use [Quickstart](QUICKSTART.md) to
install the CLI and the [CLI guide](CLI-GUIDE.md) for command details. Open an
example file when you need context around an excerpt.

Start with two commands: `semaprax check` verifies source, and `semaprax run`
verifies it before evaluating its entrypoint in the bounded
[interpreter](INTERPRETER-V1.md).

## A first program

One file defines one module. A function names its result type, and its last
expression returns the value. This core language has no `return` statement.
The entrypoint below returns an `i64`.

From [examples/meaning.spx](../examples/meaning.spx):

```semaprax
module examples.meaning;

@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(19, 23)
}
```

```sh
semaprax check examples/meaning.spx
semaprax run examples/meaning.spx
```

`check` prints `verified examples/meaning.spx (sha256:...)`. `run` prints `42`.

The formatter gives a program one canonical rendering. Check the example:

```sh
semaprax fmt examples/meaning.spx --check
```

No output and exit code `0` mean the file is already canonical.

## Persistent `@id` identity

`@id` gives a declaration a persistent identity in the semantic graph.
Renaming `add` to `plus` does not change `math.add`, so graph queries and
semantic patches can still identify it.

From [examples/meaning.spx](../examples/meaning.spx):

```semaprax
@id("math.add")
fn add(left: i64, right: i64) -> i64
```

```sh
semaprax graph examples/meaning.spx
```

The JSON marks `math.add` as an explicit, persistent function identity and
expression IDs as revision-scoped.

A declaration without `@id` gets an automatic, unstable identity. Public
declarations need explicit `@id`; expression identities may change with each
revision. See [RFC 0001](RFC-0001.md#program-representation).

## Contracts live in the signature

`requires` is a precondition. `ensures` is a postcondition; `result` names the
returned value inside it. These clauses belong to the signature and appear
in the semantic graph.

From [examples/meaning.spx](../examples/meaning.spx):

```semaprax
@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}
```

```sh
semaprax context examples/meaning.spx math.add --depth 1 --filters contracts
```

The query returns `requires` and `ensures` as structured expressions under
`"contracts"`. Every build checks types, effects, ownership, and exhaustive
matches. A safe profile turns obligations it cannot prove statically into
runtime guards. See [RFC 0001](RFC-0001.md#contracts-and-verification) and
[Agent context v1](AGENT-CONTEXT-V1.md) for exact stages and output.

## Blocks and conditionals are expressions

`if` has two result arms, and a block evaluates to its last expression.
`let` creates an immutable binding.

From [examples/control_flow.spx](../examples/control_flow.spx):

```semaprax
@id("flow.choose")
fn choose(flag: bool, base: i64) -> i64
{
    let first = base + 1;
    if flag { let second = first + 1; second } else { 0 }
}
```

```sh
semaprax run examples/control_flow.spx
```

That prints `42`. Evaluation runs left to right; lazy boolean operands run
only when needed.

## Records and immutable update

A `record` groups named fields. Fields have persistent `@id` values, and a
literal may list fields in a different order from the declaration.

From [examples/records.spx](../examples/records.spx):

```semaprax
@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,
    @id("geometry.point.y")
    y: i64,
    @id("geometry.point.enabled")
    enabled: bool,
}
```

`base with { field: value }` creates an updated record without changing the
original, even when the field is nested.

From [examples/records.spx](../examples/records.spx):

```semaprax
@id("geometry.line.shift")
fn shift(line: Line, amount: i64) -> Line
{
    line with { start: line.start with { x: line.start.x + amount } }
}
```

```sh
semaprax run examples/records.spx
```

That prints `42`. [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) owns records,
variants, matching, and the ownership of aggregate places.

## Variants and matching

Variants hold one of several named cases. `Option<T>` and `Result<T, E>` are
built into the compiler, so they need no import. A `match` binds a case's
payload by field name.

From [examples/spxgrep-project/src/tests.spx](../examples/spxgrep-project/src/tests.spx):

```semaprax
@id("spxgrep.tests.main")
fn main() -> i64
{
    let sample = [97u8, 98u8, 99u8];
    let view = array_as_slice(sample);
    match byte_get(view, 1usize) { Option::Some { value: byte } => if byte == 98u8 { 0 } else { 1 }, Option::None {} => 2, }
}
```

```sh
semaprax run examples/spxgrep-project/src/tests.spx
```

That prints `0`, because byte 1 of `[97, 98, 99]` is `98`.

`match` also accepts scalar literals, alternative patterns, guards, bindings,
and a wildcard. The compiler checks that all cases are covered.

From [examples/refutable_match.spx](../examples/refutable_match.spx):

```semaprax
@id("refutable.sign_class")
fn sign_class(value: i64) -> i64
{
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, n => 1, }
}
```

```sh
semaprax run examples/refutable_match.spx
```

That prints `-5`. Arm order matters: `-1` matches the first alternative
before the later negative-number guard. See
[Refutable match v1](REFUTABLE-MATCH-V1.md).

## Mutation is explicit and is a statement

A binding is immutable unless declared `mut`. Assignment is a statement, not
a value; a block still needs a final expression.

From [examples/explicit_mutation.spx](../examples/explicit_mutation.spx):

```semaprax
@id("mut.accumulator")
fn accumulator() -> i64
{
    let mut total = 0;
    total = total + 5;
    total = total * 2;
    let base = 3;
    let mut other = base;
    other = other + base;
    total + other
}
```

```sh
semaprax run examples/explicit_mutation.spx
```

That prints `500016`. Arithmetic is checked by default, so an overflowing
addition is a failure rather than a wrap.
[Explicit mutation v1](EXPLICIT-MUTATION-V1.md) owns the admitted profile.

A `mut` record or class value can update a field in place. Unlike `with`, this
changes the local value instead of creating another one.

From [examples/field_mutation.spx](../examples/field_mutation.spx):

```semaprax
@id("fm.track")
fn track(flag: bool) -> i64
{
    let mut origin = Point { x: 20, y: 2, enabled: false };
    origin.x = origin.x + 20;
    origin.y = origin.y + origin.x;
    let mut branch = Point { x: 0, y: 0, enabled: false };
    let delta = if flag { branch.x = 5; branch.x } else { branch.y = 6; 0 - branch.y };
    let mut counter = Counter { value: 3 };
    counter.value = counter.value * counter.value;
    origin.x + origin.y + delta + counter.get()
}
```

```sh
semaprax run examples/field_mutation.spx
```

That prints `96`. [Field mutation v1](FIELD-MUTATION-V1.md) owns which places
are assignable.

## Loops

`while` is a statement. Its body still ends in an expression, but the loop
discards that value.

From [examples/while_loops.spx](../examples/while_loops.spx):

```semaprax
@id("loops.digit_sum")
fn digit_sum(value: i64) -> i64
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
```

```sh
semaprax run examples/while_loops.spx
```

That prints `41` — the digit sum of `98765` plus `factorial(3)`.
[Bounded while-loops v1](WHILE-LOOPS-V1.md) owns the admitted loop profile.

## Classes

A `class` groups fields and methods. `self` is an explicitly typed parameter;
a method can return a changed copy.

From [examples/classes.spx](../examples/classes.spx):

```semaprax
@id("example.counter")
class Counter {
    @id("example.counter.value")
    value: i64,

    @id("example.counter.get")
    fn get(self: Counter) -> i64
{
        self.value
    }

    @id("example.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter
{
        Counter { value: self.value + amount }
    }
}
```

```sh
semaprax run examples/classes.spx
```

That prints `42`, and the example's own condition checks that the original
`base` counter still reads `40` after `bumped`.
[Class inheritance v1](CLASS-INHERITANCE-V1.md) owns classes and inheritance.

## Strings

Use named functions, rather than operators, to compare or join `string` values.

From [examples/string_ops.spx](../examples/string_ops.spx):

```semaprax
@id("ops.combine")
fn combine(left: string, right: string) -> string
{
    string_concat(left, right)
}
```

```sh
semaprax run examples/string_ops.spx
```

That prints `7`, the value the example returns when the concatenated message
has length `11` and equals `"hello world"`.
[String operations v1](STRING-OPS-V1.md) owns the admitted operation set.

## Ownership: `own` and `borrow`

A `resource` has one owner and a declared cleanup strategy. An `own`
parameter takes ownership; a `borrow` parameter reads without taking it. You
may borrow a value many times but transfer it once. The compiler reports
ownership errors before backend execution.

From [examples/ownership.spx](../examples/ownership.spx):

```semaprax
@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64
{
    1
}

@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64
{
    inspect(buffer)
}

@id("buffer.pipeline")
fn pipeline(buffer: own Buffer) -> i64
    ensures result == 2
{
    inspect(buffer) + consume(buffer)
}
```

```sh
semaprax check examples/ownership.spx
semaprax context examples/ownership.spx buffer.pipeline --depth 1 --filters ownership
```

`context` reports which parameters are `own` or `borrow` and where a loan
begins and ends. `pipeline` first borrows the buffer for `inspect`, then
transfers it to `consume`.

This example proves checking, not resource execution. `semaprax run
examples/ownership.spx` rejects with `SPX-B104` because native resource
lowering requires lifecycle declarations and the verified cleanup ABI. Check
the [completion matrix](COMPLETION-MATRIX.md) before assuming a resource path
executes.

## Cleanup and finalizers

Cleanup is declared. A resource can have a `trivial` drop or an imported
finalizer. An `interface` states the finalizer's capability, effects, failure
mode, and consumed value.

From [examples/lifecycle.spx](../examples/lifecycle.spx):

```semaprax
@id("platform.token")
resource Token {
    @id("platform.token.drop")
    drop import "platform.token.finalize";
}

@id("platform.token.host")
interface TokenHost
    permits { platform.token.release }
{
    @id("platform.token.finalize")
    import fn finalize(token: own Token) -> unit
        effects { platform.token.release }
        failure infallible
        consumes token always;
}
```

```sh
semaprax check examples/lifecycle.spx
```

Automatic finalization must be infallible and consume the token. A fallible
operation must be an explicit consuming `close`. Every initialized owned
resource that is not transferred is finalized exactly once on each
language-level exit. Cleanup order is canonical; downstream tools must not
sort or repair it.
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md#safety-contract) owns the
safety contract and the cleanup plan.

Like the ownership example, this file demonstrates declaration and checking.
`semaprax run examples/lifecycle.spx` reports the same `SPX-B104` rejection.

## Effects and capabilities

Authority is explicit: a function declares effects with `uses`, a module
grants capabilities with `permit`, and callers declare the effects of their
callees. Linking a module grants no ambient authority.

From [examples/effects.spx](../examples/effects.spx):

```semaprax
permit { clock.read }

@id("clock.logical_tick")
fn logical_tick(value: i64) -> i64
    uses { clock.read }
    ensures result == value + 1
{
    value + 1
}

@id("app.main")
fn main() -> i64
    uses { clock.read }
{
    logical_tick(41)
}
```

```sh
semaprax run examples/effects.spx
semaprax context examples/effects.spx app.main --depth 1 --filters effects
```

`run` prints `42`. `context` shows `clock.read` on both caller and callee, so
you can check that authority did not widen silently.

[RFC 0001](RFC-0001.md#effects-and-capabilities) owns the model and
[Capability manifest v1](CAPABILITY-MANIFEST-V1.md) owns how an application
grants capabilities.

## The semantic graph is the other interface

`.spx` is the human-readable Git form. Tools and agents usually ask the
versioned semantic graph for checked meaning. Two commands expose it:

`graph` emits the whole module as deterministic JSON, including identities,
types, contracts, calls, and cleanup plans.

```sh
semaprax graph examples/meaning.spx
```

The output's `revision` digest matches `semaprax check` for the same source.
Bind cached graph answers to that digest so an old answer is not mistaken for
current source. Read the `schema` from the output; it varies by feature.
[Migrations](MIGRATIONS.md) tracks schema versions.

`context` answers one bounded question. It starts at a declaration, keeps the
requested fact families, and reports whether its byte or node budget cut off
the answer.

```sh
semaprax context examples/meaning.spx math.add --depth 1 --filters contracts
semaprax context examples/effects.spx app.main --depth 1 --filters effects
```

Available filters include `contracts`, `ownership`, `effects`, `types`,
`targets`, `diagnostics`, and `tests`; `semaprax help context` prints the exact
accepted form. [Agent context v1](AGENT-CONTEXT-V1.md) and
[Agent context v2](AGENT-CONTEXT-V2.md) own the response schema, the budget
contract, and the resume rules.

Graph output, formatting, Wasm bytes, diagnostics, and semantic patches are
deterministic. Different output for the same input is a bug.

## Where to go next

- [RFC 0001](RFC-0001.md) is the language and toolchain contract, and the
  authority for anything this tour summarizes.
- [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) and
  [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) own algebraic data and
  cleanup respectively.
- The bounded language references — [explicit mutation](EXPLICIT-MUTATION-V1.md),
  [field mutation](FIELD-MUTATION-V1.md), [while loops](WHILE-LOOPS-V1.md),
  [refutable match](REFUTABLE-MATCH-V1.md), [string operations](STRING-OPS-V1.md),
  [class inheritance](CLASS-INHERITANCE-V1.md) — state exactly which shapes are
  admitted today.
- [Using the SEMAPRAX CLI](CLI-GUIDE.md) covers formatting, checking, building,
  and inspecting beyond the commands used here.
- [Quickstart](QUICKSTART.md) walks the bounded calculator project workflow,
  which is where multi-file projects, `semaprax test`, and build targets enter.
- [Completion matrix](COMPLETION-MATRIX.md) is the only place to learn what is
  actually implemented, and to what evidence standard.
