# Language tour

Audience: programmers who already know a systems language and want to read and
write SEMAPRAX source.

Status: pre-alpha guided tour over committed examples. It teaches the shapes
that exist in this checkout and makes no readiness claim; the
[completion matrix](COMPLETION-MATRIX.md) is the sole status authority, and
[RFC 0001](RFC-0001.md) is the language contract.

This tour walks from a first program to the parts of SEMAPRAX that have no
direct equivalent in C, Rust, Go, or Zig: persistent declaration identity,
contracts in the signature, statement-level mutation, ownership with exactly
once cleanup, declared effects, and the semantic graph as a first-class
interface. It explains what each construct means and links to the
specification that owns the rules instead of restating them.

## How to read this tour

Every SEMAPRAX code block below is a **verbatim excerpt of a committed example
file**, and the line directly above each block links to that file. Nothing here
is paraphrased or invented syntax; `tests/documentation.rs` fails if a block
stops matching the file it names.

Each section states the idea, shows the excerpt, and gives the exact command to
run. Run the commands from the repository root with the standalone `semaprax`
compiler; [Quickstart](QUICKSTART.md) covers installing it and
[Using the SEMAPRAX CLI](CLI-GUIDE.md) covers the commands in full. Open the
linked file whenever you want the surrounding context an excerpt omits.

Two commands are worth internalizing before anything else. `semaprax check`
verifies a file and prints a content digest; `semaprax run` verifies it and
then evaluates the entrypoint through the interpreter described in
[Interpreter v1](INTERPRETER-V1.md).

## A first program

A file is one module. A function declares its result type, and its body is a
block whose final expression is its value — the admitted core has no `return`
statement form. The entrypoint produces `i64`.

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

Formatting is not a matter of taste: there is exactly one canonical rendering
of a given program, and the committed examples are already in it.

```sh
semaprax fmt examples/meaning.spx --check
```

That command prints nothing and exits `0`, which is what "already canonical"
looks like.

## Persistent `@id` identity

The `@id` attribute above a declaration is not documentation. It is the
declaration's persistent name in the semantic graph, independent of its source
spelling, its file, and its position. Rename `add` to `plus` and `math.add`
still refers to the same declaration; every graph, patch, and impact report
keeps pointing at it.

From [examples/meaning.spx](../examples/meaning.spx):

```semaprax
@id("math.add")
fn add(left: i64, right: i64) -> i64
```

```sh
semaprax graph examples/meaning.spx
```

The emitted JSON records which identities are stable and which are scoped to
one revision. From the observed output for this file:

```text
"identity":{"declarations":"explicit-persistent-or-automatic-unstable","values":"revision-scoped-structural","expressions":"revision-scoped-structural","match_arms":"revision-scoped-structural","patterns":"revision-scoped-structural","type_parameters":"owner-and-index-stable"},"module":"examples.meaning"
```

`math.add` appears in the same document as an explicit, persistent node:

```text
{"id":"math.add","kind":"function","name":"add","identity_origin":"explicit","persistent":true,
```

A declaration without `@id` still gets an identity, but an automatic one that
is not promised to survive edits. Public declarations should carry an explicit
`@id`. [RFC 0001](RFC-0001.md#program-representation) owns the representation
rule; the repository invariant is that public declarations have persistent
`@id` identities while expression identities may be revision-scoped.

## Contracts live in the signature

`requires` states a precondition, `ensures` states a postcondition, and
`result` names the function's result inside `ensures`. These clauses are part
of the function's declared meaning, not assertions in the body, so they travel
with the signature into the graph and into every consumer of it.

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

That query returns the contract as structured data rather than text — the
`requires` list and the `ensures` list appear as expression trees under a
`"contracts"` key, keyed by the parameter and result identities. Verification
is progressive rather than all-or-nothing: types, effects, ownership, and
exhaustiveness are checked on every build, cheap contract discharge follows,
and obligations that are not statically discharged in a safe profile become
runtime guards. [RFC 0001](RFC-0001.md#contracts-and-verification) owns the
staging; the `context` shape is owned by
[Agent context v1](AGENT-CONTEXT-V1.md).

## Blocks and conditionals are expressions

`if` is an expression with both arms, and a block's value is its final
expression. A `let` binding is immutable.

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

That prints `42`. Evaluation order is left to right throughout the language,
and lazy boolean operands execute only when they are required — this is an
invariant, not an implementation detail you may rely on by accident.

## Records and immutable update

A `record` is a nominal product type. Fields carry their own `@id`, so a field
identity is as persistent as a declaration identity, and field order in a
literal does not have to match the declaration.

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

`base with { field: value }` produces a new record from an existing one,
including through nesting, without mutating the base.

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

Variants are the sum half of the data model. `Option<T>` and `Result<T, E>` are
compiler-owned variants rather than library types: they appear as
`"identity_origin":"compiler_owned"` declaration nodes in a module's graph even
when the module never mentions them, which is why an `Option` match needs no
import. A `match` over a variant binds case payloads by field name.

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

`match` also works on scalars, with literal arms, or-patterns, guarded arms,
bindings, and a wildcard. Arms are checked for exhaustiveness.

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

That prints `-5`. Arm order matters: `-1` reaches the `-1 | -2` arm before the
guarded `n if n < 0` arm. [Refutable match v1](REFUTABLE-MATCH-V1.md) owns the
admitted pattern surface and its diagnostics.

## Mutation is explicit and is a statement

A binding is immutable unless it says `mut`, and assignment is a *statement*,
never an expression that yields a value. That is why the mutating blocks below
still end in a value expression: every block needs one.

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

A `mut` binding of a record or class instance can also be assigned field by
field, which is a different operation from `with` update: it writes into the
existing local instead of producing a new value.

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

`while` is a statement and produces no value. Because every block ends in a
value expression, an admitted `while` body ends in one too, and the loop
discards it.

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

A `class` groups persistent-identity fields with methods. `self` is an ordinary
declared parameter with a declared type, and a method that "changes" an
instance returns a new one.

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

`string` values are compared and combined through named operations rather than
operators.

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

This is the part with no shortcut. A `resource` is a uniquely owned opaque
value with a declared destruction strategy. A parameter is annotated `own` when
the call *takes* the value and `borrow` when the call only reads it. An owned
value may be borrowed any number of times and transferred exactly once, and
ownership errors are compile-time diagnostics rather than backend accidents.

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

`check` verifies the file. The `context` query reports the mode of every
parameter; from the observed output, the three functions differ exactly where
the source says they do:

```text
"ownership":{"parameters":[{"id":"declaration:15:buffer.pipeline:value:param:1:0","mode":"own"}],"result":"value"}
"ownership":{"parameters":[{"id":"declaration:14:buffer.consume:value:param:1:0","mode":"own"}],"result":"value"}
"ownership":{"parameters":[{"id":"declaration:14:buffer.inspect:value:param:1:0","mode":"borrow"}],"result":"value"}
```

`pipeline` borrows the buffer for `inspect`, then transfers it to `consume`;
the same output carries a loan plan recording where that borrow starts and
ends. Note what the tour does *not* do here: `semaprax run
examples/ownership.spx` reports
`error[SPX-B104]: native resource lowering requires lifecycle declarations and
the verified cleanup ABI`. Resource *checking* is what this example
demonstrates; ordinary native resource execution is not admitted in this
checkout. Consult the [completion matrix](COMPLETION-MATRIX.md) before
assuming any resource path executes.

## Cleanup and finalizers

Destruction is a declared contract, not a convention. A resource's `drop` can
be `trivial`, as above, or delegate to an imported finalizer. The finalizer is
declared inside an `interface` that names the capability it needs, the effects
it performs, whether it can fail, and what it consumes.

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

`failure infallible` and `consumes token always` are the two clauses to read
closely: automatic finalization must not fail, so a resource operation that
*can* fail is an explicit consuming `close`, never an implicit finalizer. Every
successfully initialized owned resource that is not transferred is finalized
exactly once on every language-level exit, including contract failure and
checked arithmetic failure, and that cleanup order is canonical — downstream
tools must never sort or repair it.
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md#safety-contract) owns the
safety contract and the cleanup plan.

Like the ownership example, this file demonstrates declaration and checking.
`semaprax run examples/lifecycle.spx` reports the same `SPX-B104` rejection.

## Effects and capabilities

Authority is part of a function's type. A function declares the effects it
performs with `uses`, a module grants capabilities with `permit`, and a caller
must declare the effects of its callees — nothing receives ambient filesystem,
process, network, clock, or signing authority by being linked in.

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

`run` prints `42`. The `context` query shows the effect on both the caller and
the callee, so a reviewer can see that authority did not widen silently. From
the observed output:

```text
"facts":[{"id":"app.main","kind":"function","name":"main","calls":["clock.logical_tick"],"reference_index":{"values":["declaration:8:app.main:value:result:0:"],"declarations":[]},"effects":["clock.read"]},{"id":"clock.logical_tick","kind":"function","name":"logical_tick","calls":[],"reference_index":{"values":["declaration:18:clock.logical_tick:value:param:1:0","declaration:18:clock.logical_tick:value:result:0:"],"declarations":[]},"effects":["clock.read"]}]}
```

[RFC 0001](RFC-0001.md#effects-and-capabilities) owns the model and
[Capability manifest v1](CAPABILITY-MANIFEST-V1.md) owns how an application
grants capabilities.

## The semantic graph is the other interface

Everything above is the human projection. The `.spx` text is the canonical Git
projection of a versioned semantic graph, and that graph — not the text — is
the preferred interface for tools and agents. Two commands expose it.

`semaprax graph <file>` emits the whole module: identity policy, type facts,
declaration nodes, contract expression trees, call edges, and a cleanup plan
per function, as deterministic JSON.

```sh
semaprax graph examples/meaning.spx
```

For `examples/meaning.spx` the observed document begins by naming its schema
and binding itself to the exact source bytes:

```text
{"schema":"semaprax.graph.v10","revision":"sha256:42aeae2650d15b1e44b8fd6d8a7ce6018d61f43e0e7988a58da2426b2f0c1657"
```

That `revision` digest is the same one `semaprax check` prints, which is what
makes graph answers safe to cache and to review: a stale answer cannot be
mistaken for a current one. The `schema` version depends on which features the
module uses, so read it from the output rather than assuming a number;
[Migrations](MIGRATIONS.md) tracks the schema history.

`semaprax context <file> <symbol-or-id>` answers a *bounded* question instead
of dumping a module — it walks outward from one declaration to a requested
depth, keeps only the fact families you ask for, and reports its own budget and
truncation so a consumer knows whether it saw everything.

```sh
semaprax context examples/meaning.spx math.add --depth 1 --filters contracts
semaprax context examples/effects.spx app.main --depth 1 --filters effects
```

Available filters include `contracts`, `ownership`, `effects`, `types`,
`targets`, `diagnostics`, and `tests`; `semaprax help context` prints the exact
accepted form. [Agent context v1](AGENT-CONTEXT-V1.md) and
[Agent context v2](AGENT-CONTEXT-V2.md) own the response schema, the budget
contract, and the resume rules.

Graph output is deterministic, as are source formatting, Wasm bytes,
diagnostics, and semantic patches. If two runs on the same bytes disagree, that
is a bug, not a tolerance.

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
