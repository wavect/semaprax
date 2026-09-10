# Scalar Snapshot Closures v1

Status: implemented private Copy-scalar snapshot profile; **HOSTED GREEN** under
the [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md), including its admitted
Graph v37, SemanticProgram v5/ProgramRoot and interpreter/C11/Core-Wasm corpus.
Public callable ABI and owning captures remain separate.

Audience: language users, compiler contributors, backend implementers, and
workspace-service authors.

This additive LANG-07 profile extends Function Values v1/v2 with anonymous
functions carrying scalar snapshots. It is not a general owning closure ABI.

## Syntax and eligibility

```semaprax
module example.closures;
@id("example.main") fn main()->i64 {
    let mut offset = 2;
    let callback = fn(value:i64)->i64 { value + offset };
    offset = 100;
    callback(40)
}
```

The result is 42: construction snapshots `offset` before the later assignment.
Parameters and result are explicitly typed. Zero through eight parameters and
zero through eight captures are admitted. Every parameter, result and capture
is one of `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, or `bool`.

Free lexical values become captures; parameter and local bindings shadow outer
names normally. Captures and parameters are immutable inside the body. A mutable
outer binding is read once at construction, so later outer mutation does not
change the captured snapshot. Owning values, borrowed views, function values,
resource fields, and projected capture places are excluded. Capture expressions
are not authored: each is exactly one unprojected scalar place read.

The body admits scalar literals, local bindings and mutation, ordinary scalar
operators, conditionals, bounded while loops, and calls to eligible ordinary
local scalar functions. Body-local owning allocations, nested anonymous
closures and generic calls remain closed. Anonymous closures within generic
templates and closure creation inside while bodies are separately specified by
the additive [Closures v2 profile](CLOSURES-V2.md). Body analysis is bounded
at 4,096 nodes. Unsupported source profiles use `SPX-T288`.

A closure has the same `fn(T0,...) -> R` signature as a compatible named function.
It may be copied, selected, passed to private helpers, returned from private
helpers, or supplied to existing generic collection callback parameters.
No function-valued public boundary, captured environment ownership, capture
mutation, escaping borrow, or implicit ambient capability is introduced.

## Identity and independent replay

HIR retains `Closure { parameters, captures, body }`. Each capture binds one
private body parameter to one outer scalar snapshot. Capture order is the
strict order of outer `ValueId` identities, with no duplicates or unused slots.
The private callable identity is derived from the enclosing closure expression
identity under `semaprax.closure.v1`; any collision with an indexed declaration
is rejected. It is not an authored persistent declaration or backend address.

Capture values have expression identities `.capture.N` under the creation site.
The body uses its own derived function execution identity and the ordinary
`body` root path. Capture parameters precede explicit parameters in that private
function product. HIR independently checks exact types, scalar ownership,
outer availability, capture order, parameter identities, and all body expression
identities. Captures cannot be replaced with an effectful or failing expression.
The combined callable target universe is bounded at 256 entries.

## Creation and invocation

Closure creation reads the scalar snapshots and produces a Copy carrier. It
does not execute the body, run body contracts, allocate an owning environment,
or evaluate a body call. Creation therefore has the ordinary atomic Copy cleanup
behavior. General ownership and runtime child traversals visit capture values
only. Semantic queries separately retain the body and its callable dependencies.

Invocation evaluates the callable and then each argument once in left-to-right
order. The compiler derives a separate private function product for the body,
including ordinary loan, failure, and cleanup plans. Backends consume that
checked product rather than executing the body in the creator's cleanup scope.
Checked arithmetic and called-function failures remain sticky. Resource usage
and conservative cycle checks include the callable body at invocation, without
charging or executing it at construction.

Closure-bearing programs use a bounded environment carrier with one target
identity projection and eight scalar storage cells. Cells preserve exact scalar
bits and have target-declared types. Native/Wasm representations are projections;
source and graph identities do not depend on addresses or table positions.
Programs without closures retain their prior named-function representation.

## Evidence required for promotion

The implemented corpus covers snapshot timing, parameter shadowing, private
return/escape, all eight Copy scalar capture types, AST/HIR carrier replay,
Graph v37, SemanticProgram v5/ProgramRoot replay, and repeated generic
map/filter/fold execution with an explicit Core-Wasm Vec host settlement
inventory. Its current release evidence is hosted green; prior local observations
remain historical witnesses. Generic-template and loop construction belong to
the implemented v2 profile. This profile does not admit owning captures or public
callable signatures.

Focused checks must cover snapshot timing, scalar captures, parameter shadowing,
zero/eight captures and first-over-limit rejection, capture/body identity
mutation, copying/selecting/returning closures, generic map/filter/fold callbacks,
and repeated interpreter/native/Wasm success and checked failure. Source/graph
round trips and replay must retain both creation facts and the independently
checked private body. A source or HIR ownership rejection is not replaceable by
a backend failure.
