# Static Protocol Conformance v1

Audience: language users, compiler contributors, agent authors, and reviewers.

Status: implementation and regression evidence authored, **unrun**. The user's
instruction deliberately skips tests, compiler/interpreter/target execution,
and long quality gates. This is not verified backend, conformance, performance,
or full-programme completion evidence.

Static implementations bind local protocol requirements to existing ordinary
functions by persistent declaration identity. The compiler checks these source
declarations before resolving backend HIR. There is no dispatch instruction,
protocol value, runtime witness table, new ABI, or runtime receiver conversion.
An ordinary function call still has its ordinary checked behavior.

```spx
module geometry;

@id("geometry.point")
record Point { @id("geometry.point.x") x: i64, }

@id("geometry.read-x")
protocol ReadX {
    @id("geometry.read-x.get")
    fn get(self: Self) -> i64;
}

@id("geometry.point.read-x")
impl "geometry.read-x" for "geometry.point" {
    "geometry.read-x.get" = "geometry.point.get";
}

@id("geometry.point.get")
fn get(point: Point) -> i64 { point.x }

@id("geometry.main")
fn main() -> i64 { get(Point { x: 7 }) }
```

The formatter preserves both protocol requirements and implementation tables,
including explicit IDs. It emits protocols after host-import interfaces,
implementations after protocols, and ordinary functions afterward, retaining
source declaration/member order within those groups. Formatting no longer
silently drops protocols. Programs without protocols or implementations retain
their ordinary canonical source bytes.

## Admission rules

An implementation requires an explicit `@id`; the protocol and receiver strings
identify declarations rather than paths or display names. The receiver is a
local explicitly identified monomorphic record. The protocol and every bound
requirement have explicit local identities. Each required method appears
exactly once, no unknown method appears, and a function cannot satisfy two
members of the same implementation. A protocol/receiver pair has at most one
local implementation.

Every bound function is an explicit, monomorphic, top-level local function other
than `main`. The first required parameter must use `Self` or the protocol's name;
that parameter's type is replaced with the selected record type for comparison.
Parameter order, parameter ownership modes, all remaining types, and result
type must match exactly. Parameter display names need not match. Protocol v1's
existing closed type rules still apply: no generic arguments, no `Self` result
or nonreceiver parameter, and no newly widened protocol signature vocabulary.

Protocol signatures have no effects or preconditions in this syntax. A bound
function therefore declares no effects and has no `requires` predicates. Its
ordinary `ensures` predicates remain allowed and are checked by the existing
language pipeline. A matching signature alone never establishes a valid body,
ownership plan, cleanup plan, effect closure, or target profile: ordinary source
verification and HIR resolution remain mandatory before backend consumption.

Implementation IDs share the declaration namespace with functions, records,
fields, variants/cases/payloads, class methods, resource lifecycle declarations,
host interfaces/imports, protocols, and protocol methods. Prelude IDs also remain
reserved. Original workspace ASTs are checked before imports become synthetic
local stubs, preventing imported functions/types from impersonating local
implementation targets. A workspace-wide source identity check also catches
collisions involving protocol/implementation IDs absent from the runtime graph.

New implementation and selector IDs contain 1–240 ASCII letters, digits, `.`,
`_`, `:`, or `-`. Implementation IDs additionally reject `auto:` and `semaprax.`
prefixes. This is a deliberately closed selector vocabulary, not a migration of
every legacy declaration's identity grammar. Display renaming an implementation
function does not change its binding because the table contains its stable ID.

## AST and read-only facts

`Program.implementations` holds `ProtocolImplementation` values with
`stable_id`, `explicit_id`, `protocol_id`, `receiver_id`, `members`, and `span`.
Each `ProtocolImplementationMember` holds `method_id`, `function_id`, and `span`.
These remain canonical source declarations. Resolved runtime Program schemas
and existing backend function representations remain unchanged.

`static_protocol::validate(&Program)` validates the static declaration rules.
`member_matches(protocol, method, receiver, function)` only answers signature
eligibility; it is not complete source or identity admission.
`static_protocol::facts(&Program)` first bounds/derives the table and then
requires successful full single-module HIR resolution. It returns a JSON Value
with schema `semaprax.static-protocol-conformance.v1`, path/module provenance,
sorted protocol/method inventories, and sorted implementation/member tables.
`full_source_admitted` is true only on this public full-verification route.
Source type strings are explicitly labeled as display projections, not resolved
HIR type identities. No report grants source or execution authority.

The crate-private `declaration_facts` seam provides the same source-owned table
with `full_source_admitted:false` for callers already holding independently
admitted workspace HIR and source facts. Image query owners bind this table to
that retained source and image revision; the table does not deserialize or
replace HIR. Facts use stable-ID ordering, while source formatting preserves
declaration order.

Bounds are 256 protocol declarations and implementations per module, 256
requirements/bindings per declaration, 4,096 total requirements and separately
4,096 total implementation bindings per module, and 64 parameters per method.
The workspace identity check accepts at most 16 modules and 65,536 source IDs.
Fact construction uses a conservative 4 MiB JSON string/entry charge before
cloning report arrays; individual report strings are bounded to 4,096 bytes,
and type names are checked before formatting. These charges bound report
construction, not the entire source AST, HIR, process heap, or RSS. Existing
source, graph, and target profile bounds still apply. New formatter output uses
the existing bounded writer, with additive metadata accounting.

## Legacy behavior and remaining evidence

The frozen `protocol-check` v1 projection has an explicitly empty conformance
section and cannot honestly represent real implementations. It rejects any
impl-bearing source with `SPX-Q110` instead of emitting that section. Its
existing v1 envelope verifier remains unchanged. Protocol-bearing canonical
source/revision bytes change because declarations previously disappeared during
formatting; the existing pinned protocol-envelope golden digest must be
independently rederived after execution is authorized. It has deliberately not
been guessed or silently updated in this unrun change. The runtime graph schema
lattice is not expanded by these source facts.

`SPX-Q106` covers invalid explicit/local targets and static declaration grammar;
`SPX-Q107` covers incomplete, repeated, or incompatible bindings;
`SPX-Q108` covers identity and duplicate protocol/receiver collisions;
`SPX-Q109` covers capacity; `SPX-Q110` closes legacy projection on real impls.
Existing parser errors and protocol signature/identity `SPX-Q104`/`SPX-Q105`
diagnostics retain their meaning.

Core evidence is authored in `tests/static_protocol_conformance_v1.rs`: canonical
source round trips, real admitted facts, display-rename stability, exact local
inventory, ownership/signature/effect/precondition rejection, body verification,
duplicate identities/pairs/functions, bounds, and legacy projection refusal.
Workspace/image/candidate integration has separate owning regressions. No test
was run for this change. Cross-module protocol implementations, generic
conformance, inherited/default methods, dynamic dispatch, runtime witnesses,
and protocol-typed public APIs remain unsupported.
