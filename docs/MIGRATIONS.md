# Protocol migrations

SEMAPRAX is pre-alpha, but agent-facing changes are still explicit. Consumers must inspect the declared schema field rather than assuming every JSON object has the latest shape.

## Graph-v6 CLI context to agent-context v1

`semaprax context` now emits `semaprax.agent-context.v1` instead of a Graph-v6
context view. Consumers must check `schema`, honor byte/node budgets,
truncation reasons, omitted counts, and resume frontiers, and must not interpret
unavailable target/diagnostic/test filters as empty facts. The legacy Rust
`graph::context_json(program, symbol, depth)` API is unchanged; new Rust
consumers use `graph::agent_context_json` with `AgentContextOptions`. There is
no schema negotiation or silent fallback.

## Agent Context v1 to v2 directional queries

Agent Context v1 remains the exact CLI and Rust API default. An explicit
`--direction forward|reverse|both` selects `semaprax.agent-context.v2`; Rust
consumers use `AgentContextV2Options` and `agent_context_v2_json`. Consumers
must bind replay to the query direction and treat `frontier` as omitted
selected-direction traversal nodes while treating `reference_frontier` as
referenced non-selected relation targets. Reference closure is not a
truncation reason and does not contribute to traversal omitted/deferred counts.

V2 retains the v1 byte, node, depth, filter, and fail-closed limits. It reports
the same program-selected Graph v10/v11/v12/v13/v14 schema and changes no Graph
bytes, source revision, HIR, or CleanupPlan. Frozen SHA-256 KATs are forward
`922404133444942ab86607772362098e0f5656add6bea607a890be2bcfe5b7c9`,
reverse `9a2ebfe569926e67f436379cf2b5c96d510daadd11d0a295ed54903cb612627b`,
and both `4ec8a62a17551e87dc301d08f0a09c6159445757bca6dd9920a7db4e3790ce17`.
V1 golden preservation and v2 execution are hosted green in [run 31397881268,
Ubuntu job
93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327).
V2 is not general impact analysis or a reverse index for non-call semantics.

## Semantic Patch v1 to v2

Schema-less patches retain the exact v1 parser and operation domain: one
revision base, explicit function/resource renames, and `require
no-new-effects`. The first non-comment line `schema
semaprax.semantic-patch.v2` opts into persistent record/case-member and
variant-case renames plus exact addressed `i64`/`bool` generic-call
type-argument replacement. V2 rejects schema placement confusion, duplicate or
overlapping selectors, automatic/compiler-owned identities, stale call tuples,
and any post-HIR semantic delta outside the selected identities. Pattern
shorthand expands its label while preserving the original binding, `ValueId`,
and `Place` root.

V2 does not migrate Graph or CleanupPlan: the program-selected Graph
v10/v11/v12/v13/v14 schema and CleanupPlan v2/v3 remain authoritative. It also
does not authenticate patch-file path/content provenance, add multi-file
commit, or open general type, shape, generic-composition, repair, or impact
operations. The frozen mixed-transaction post-edit revision and old/new
function-instance KATs remain recorded in
[the v2 contract](SEMANTIC-PATCH-V2.md).

The implementation commit `b92ce68` did not have a green hosted matrix: [run
31400888352](https://github.com/wavect/semaprax/actions/runs/31400888352) was
cancelled after its isolated Wasmtime job failed on stale runtime-lock state.
Commit `f95d243` reconciled that lock. Attempt 1 of its workflow was cancelled
and is not evidence; the exact head's [run 31401200449 attempt
2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2) is
terminal green, including [Ubuntu job
93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044)
and [Wasmtime job
93505622110](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622110).

## Semantic Patch to read-only Impact v1

`semaprax impact <file> <patch.spatch>` is an additive preview command. It
accepts the existing schema-less Patch v1 and explicit Patch v2 grammars and
does not alter either transaction format. Consumers must inspect report schema
`semaprax.semantic-impact.v1`, bind the source base/candidate revisions and
patch schema/digest, preserve operation and change indices, and honor exact
byte/node/depth truncation plus frontier and omitted/deferred counts.

The preview rechecks its source snapshot but performs no A0 lock, staging, or
commit. Its patch digest authenticates the exact bytes read once into the
preview, not the continuing patch path; provenance-sensitive callers must
authenticate that input externally. Rename source consumers are projection
facts, while only exact generic-call instance changes seed finite reverse-call
impact over explicit persistent callables. Automatic behavioral callers fail
closed as `SPX-G110`, and existing generic-function limits continue to report
`SPX-T226`. This is not a migration to repository-wide or general non-call
impact. The canonical report KAT is recorded in [the Impact v1
contract](SEMANTIC-IMPACT-V1.md). Its exact `1b3731a` full matrix is hosted
green in [run 31408654657 attempt
2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
including [Ubuntu job
93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).

## SPX-S103 to Diagnostic Repair v1 and isolated Semantic Patch v3

Existing `SPX-S103` diagnostics remain unchanged. The additive command
`semaprax repairs <file> assign-function-id <automatic-function-id>` now
returns canonical `semaprax.diagnostic-repair.v1` JSON for one eligible
automatic function in the closed acyclic scalar Graph-v10 domain. The additive
command `semaprax repair <file> <repair-id> --persistent-id <persistent-id>`
returns canonical `semaprax.diagnostic-repair-preview.v1` JSON after
independently proving the exact one-annotation candidate. Both commands are
read-only.

The preview contains an exact three-line LF-terminated
`semaprax.semantic-patch.v3` file with one `assign-function-id` operation.
Unlike the query and instantiation commands, `semaprax patch` may apply that
v3 file: it reauthenticates the revision-bound repair ID, diagnostic, target,
name, closed persistent ID, reduced program domain, and complete candidate
identity rebase, then uses unchanged A0. V3 is isolated from schema-less v1 and
explicit v2; it composes with neither grammar and admits no other operation.
Semantic Impact v1 remains a Patch v1/v2 preview and rejects every
syntactically valid, canonical v3 as `SPX-G110` before semantic selector
interpretation. Malformed or noncanonical v3 remains `SPX-G101`.

The operation classification is exactly `breaking_identity_rebase`. Consumers
must not treat it as a stable-ID-preserving rename: the automatic function ID
and every revision-scoped identity below the selected function intentionally
change, while direct callers change their callee reference. Graph-v10 revision
and identity-bearing content therefore change, and identity-bearing CleanupPlan
content may rebase. No Graph or CleanupPlan schema/version or semantic shape is
widened, Graph v11-v14 is not admitted for repair, and backend/runtime semantics
do not change. The exact schemas,
key/line order, domains, KATs, limits, and nonclaims are frozen in
[`DIAGNOSTIC-REPAIR-V1.md`](DIAGNOSTIC-REPAIR-V1.md). Local Phase A integration
is 13/13; the Phase B semantic integration corpus is 7/7; v3 A0 hook units are
4/4; aggregate v3 integration-plus-hook evidence is 9/9; and library 404/404,
full-preservation, and security gates are green. Hosted evidence is pending.
V3 additionally runs function/call-site bounds on parsed AST before HIR and
caps its initial A0 source read plus both final rechecks at 16 MiB. Initial
oversize fails `SPX-R101`; concurrent final growth past the bound fails
`SPX-I207` without replacing the grown source. Patch v1/v2 reads are unchanged.

## Persistent identities are NUL-free

Persistent semantic identities and logical import keys may not contain a literal NUL byte. Source validation reports the declaration-specific stable diagnostic before resolution or graph serialization; `\0` remains an unsupported source-string escape. Public consumers of transformed resolved HIR must likewise reject NUL in declaration IDs, types, expressions, places, call/record/field references, and attached cleanup inventory or plan metadata before code generation or serialization. Regenerate or rename any pre-alpha fixture that constructed such an identity directly.

## Semantic graph v1 to v2

Graph v2 adds:

- top-level declaration/expression identity policy;
- deterministic revision-scoped structural nodes for function bodies and local bindings;
- `requires_graph` and `ensures_graph` expression structures;
- contract calls in function call dependencies and bounded context traversal.

The existing textual `requires` and `ensures` arrays remain during this migration. A v1-only consumer should reject `semaprax.graph.v2` explicitly. A compatible consumer may continue reading declaration IDs, names, signatures, effects, contracts, and calls while progressively adopting the new structural fields.

## Semantic graph v2 to v3

Graph v3 is a breaking migration from parsed syntax to validated resolved HIR:

- public Rust APIs `graph::to_json` and `graph::context_json` are fallible and return verification/HIR diagnostics instead of producing an unchecked graph;
- top-level `entrypoint` is a declaration ID and `view` distinguishes complete module graphs from bounded context slices;
- context views report their declaration-ID `root`, requested `depth`, `truncated` state, and sorted dependency `frontier`;
- declaration nodes expose `identity_origin` and `persistent`, because only explicit `@id` identities survive renames;
- parameters, local bindings, places, and function results use resolved value IDs; calls and dependency arrays use resolved declaration IDs rather than display names;
- `calls` is a sorted, de-duplicated dependency set; individual call occurrences remain in contract and body expression trees;
- expressions expose resolved `type_id` and `ownership_mode`; `ownership_mode` is a boundary mode, not flow-sensitive availability state;
- `i64` literal `value` fields are decimal strings, preserving the full signed 64-bit domain in JavaScript/TypeScript consumers;
- `let` statements carry their binding's value ID only inside `binding`; statements do not claim a separate identity until a dedicated statement-ID domain exists;
- top-level `type_facts` entries are keyed by the resolved type identity and carry resolved type structure plus copy, resource containment, size, drop, and layout facts;
- context output includes only selected functions and their referenced nominal type declarations rather than every resource in the module;
- textual `requires` and `ensures` arrays are removed; `requires_graph` and `ensures_graph` are authoritative structured contracts;
- HIR spans are omitted because the canonical source revision is whitespace-insensitive while source positions are not.

The `revision` remains derived from the canonical human-readable source projection; graph wire-format changes alone do not alter it. A v2 consumer must reject `semaprax.graph.v3` until it supports the new type/value identity tables and fallible API. Exact v3 module fixtures live in `tests/snapshots/meaning.graph.json` and `tests/snapshots/control_flow.graph.json`.

## Semantic graph v3 to v4

Graph v4 adds the first algebraic-data declarations and expressions:

- record declarations are `record` nodes whose ordered `fields` array contains stable field declaration IDs;
- every field is a separate `field` node with its stable ID, display name, explicit/automatic identity origin, persistence flag, owner record ID, declaration-order `index`, and resolved `type_id`;
- record construction expressions use `kind: "construct_record"`, a stable record declaration ID, and source-ordered initializer entries containing stable field IDs and values;
- projection expressions and place projections use stable field IDs rather than display names;
- context slices close transitively over nominal record types referenced by selected functions and over the nominal types of their fields;
- the type-facts table includes every field type required by selected record declarations;
- validated-HIR and graph reference checks fail closed before an unresolved or foreign record/field reference can be serialized.

The SHA-256 revision contract is unchanged. A v3 consumer must reject `semaprax.graph.v4` until it understands record/field nodes and record expression kinds.

## Semantic graph v4 to v5 and explicit resource lifecycles

Graph v5 adds persistent resource-lifecycle, interface, and logical-import declarations. Resource nodes now reference a `resource_drop` node. Imported drop nodes reference an `import` node and target-neutral `import_key`; interface nodes expose their authority ceiling and import IDs; import nodes serialize parameter ownership, consumption on failure, unit-result publication rules, effects, required authority, and normalized failure contracts. Context slices close a referenced resource through its lifecycle, complete owning interface, import signatures, and their nominal types.

The initial source grammar deliberately uses an import's explicit `@id` as its v1 logical import key while resolved HIR stores `import_id` and `import_key` separately. This is a versioned source projection choice, not a permanent conflation of conceptual identity and target binding keys.

Rust API consumers must update exhaustive matches and construction code:

- `Program` adds `interfaces: Vec<InterfaceDeclaration>`;
- `TypeDeclarationKind::Resource` becomes `Resource { lifecycles: Vec<ResourceLifecycleDeclaration> }`;
- the parsed AST adds resource lifecycle kind/declaration, interface/import declaration, and import-failure types;
- `ResolvedProgram` adds resolved interfaces;
- resolved resource declarations carry `ResolvedResourceDrop` and its strategy;
- resolved HIR adds interface/import parameter, unit-result publication, authority, and normalized-failure structures;
- `DeclarationKind` adds `ResourceDrop`, `Interface`, and `Import`.

Legacy `resource Name;` still parses so `check` can report `SPX-O112`, but it is no longer a valid resource declaration. Migration requires an explicit, persistent lifecycle ID and an authored choice:

```semaprax
@id("token.type")
resource Token {
    @id("token.type.drop")
    drop trivial;
}
```

or a complete `drop import` plus interface/import contract. Formatting never invents `drop trivial`; it is an audited semantic assertion. Phase 1 by itself did not execute cleanup: native resource builds still retain `SPX-B104`, and Wasm retains `SPX-W111` for every shape outside the later, separately versioned narrow owned ABI.

The SHA-256 algorithm and domain separator are unchanged, but migrated canonical source receives a new revision as expected. A v4 consumer must reject `semaprax.graph.v5`. The former exact v5 fixtures were superseded by the Graph v6 cleanup-plan migration below.

## Rust HIR cleanup inventory

`ResolvedFunction` now carries a mandatory `cleanup: CleanupInventory`. Direct Rust consumers that construct or transform resolved HIR must preserve the exact inventory or rerun source resolution; `hir::validate`, native lowering, and Wasm lowering reject a missing or stale inventory with `SPX-H006` before any target feature gate.

The inventory schema is `semaprax.cleanup-inventory.v1`. It catalogs canonical storage candidates for owned non-copy parameters, droppable local bindings, owned-producing expression temporaries, and droppable provisional results. Recursive shapes retain declaration-ordered field IDs, and every resource leaf has an exact projected place, lifecycle ID, and distinct liveness-flag identity. Entry state lists only owned droppable parameters. `discovery_index` is deterministic structural discovery order, not runtime initialization or finalization order.

The inventory remains a structural Rust HIR boundary. It does not itself contain CFG edges, path-sensitive liveness, transfers, call commits, status sources, cleanup regions/exits, finalization order, result publication, or a backend trace. Those facts now live in the separately versioned plan below.

## Graph v5 to v6 and Rust HIR cleanup plans

`ResolvedFunction` now also carries mandatory `cleanup_plan: CleanupPlan` using schema `semaprax.cleanup-plan.v1`. Direct Rust consumers that construct or transform resolved HIR must rerun source resolution or preserve the exact canonical plan. Validation first checks core HIR, then rebuilds `CleanupInventory`, then rebuilds the plan without consulting the attached plan; any mismatch is `SPX-H006` before native or Wasm lowering.

Graph v6 embeds the complete plan under each selected function's `cleanup` member. It adds tagged storage/place, recursive liveness shapes, status sources and stable arithmetic codes, transitions, blocks, edges, regions, guarded finalizers, exits, and scalar/owned result commits. Arrays are already in canonical semantic order and consumers must not sort them. Context slices include complete plans for selected functions without unrelated functions.

The canonical source revision algorithm and domain separator are unchanged. The same source can therefore have the same revision in Graph v5 and v6 while the graph payload differs; caches and protocol negotiation must key by `(graph schema, revision)`. A v5 consumer must reject `semaprax.graph.v6`. Exact v6 scalar, control-flow, record, and lifecycle snapshots replace the v5 fixtures.

## Graph v6 to v7 and executable record updates

Graph v7 adds the resolved `update_record` expression. Its `base` edge is
serialized before its authored `fields` vector; every replacement entry carries
the persistent field declaration ID and its value edge. Producers must preserve
replacement order because evaluation consumes the complete base first and then
evaluates replacements left-to-right. Consumers must not resolve fields from
display names or sort replacement entries.

The source revision algorithm is unchanged. Consequently identical source that
does not use record update can have the same revision under Graph v6 and v7,
while the graph payload and agent-context `source_graph_schema` differ. Cache
and protocol keys must include the exact graph schema. A v6 consumer must reject
`semaprax.graph.v7`, and a v7 consumer must reject `semaprax.graph.v6`; neither
may silently reinterpret an unknown expression kind. Agent Context v1 remains
the context-envelope schema and now declares `semaprax.graph.v7` as its source.

## Graph v7 to v8 and executable copy variants

Graph v8 adds persistent `variant`, `variant_case`, and variant payload-field
declarations plus revision-scoped `construct_variant`, `match`, `match_arm`,
variant-pattern, wildcard-pattern, and arm-local payload-binding structure.
Case and payload vectors remain declaration ordered. Match-arm vectors remain
authored ordered. CleanupPlan v1 keeps its schema identity while adding the
closed `variant_case` edge condition, which binds one scrutinee expression ID,
one stable case declaration ID, and a boolean match polarity. Producers and
consumers must not substitute display names, numeric tags, or boolean-result
edges for this meaning.

The source-revision algorithm remains unchanged, so identical source without
variants may retain its revision while the graph payload changes. Cache and
protocol keys must include the exact schema. A v7 consumer must reject
`semaprax.graph.v8`; a v8 consumer must reject `semaprax.graph.v7` and every
unknown declaration, expression, pattern, or edge kind. Agent Context v1 keeps
its envelope schema but now declares `semaprax.graph.v8` as its
`source_graph_schema`. Graph v8 does not imply generic variants, `Option`,
`Result`, `?`, resource-bearing payloads, ownership-aware matching, a stable
public ABI, callable/component aggregate support, or public resource admission.

## Graph v8 to v9, revision v2, and the ordinary prelude

Graph v9 adds owner/index-stable generic type parameters, exact ordered nominal
argument trees, compiler-owned declaration provenance, and the authenticated
`semaprax.prelude.v1` contract. The ordinary compiler-owned `Option<T>` and
`Result<T, E>` declarations, cases, and payload fields use persistent reserved
IDs. Authored source must not redeclare their names or IDs, and consumers must
not reinterpret them as backend intrinsics or user declarations.

Graph revision v2 uses the domain `semaprax.graph-revision.v2\0` and
length-delimits canonical source, prelude schema, and exact prelude contract
bytes. Therefore even source that does not mention generic variants can receive
a different revision from the v1 algorithm. A prelude-contract change cannot
silently retain an old semantic revision. Cache and protocol keys must include
the exact graph schema and revision; consumers that persist compiler-owned
facts should also authenticate the prelude schema/digest carried by Graph v9.

A v8 consumer must reject `semaprax.graph.v9`; a v9 consumer must reject
`semaprax.graph.v8`, an unknown prelude schema/digest, malformed type-parameter
identity, and every unsupported argument tree. Agent Context v1 retains its
envelope schema and now declares `semaprax.graph.v9` as its
`source_graph_schema`, including referenced compiler-owned prelude declarations
only when the bounded context closes over them. Graph v9 does not imply generic
functions or records, nested/resource arguments, `?`, non-copy matching, a
stable public aggregate ABI, callable/component aggregate support, or public
resource admission.

## Graph v9 to v10 and CleanupPlan v1 to v2

Graph v10 adds revision-scoped `try_result` meaning for the bounded ordinary
`Result<T, E>` postfix-`?` slice. Each node authenticates the exact source and
outer concrete instances, compiler-owned Result/Ok/Err case and field IDs,
one operand evaluation, an `Err` exit classified as a normal result rather
than a physical failure, and the shared postcondition epilogue. A v9 consumer
must reject `semaprax.graph.v10`; a v10 consumer must reject v9, unknown Try
fields, and rehashed substitutions of any authenticated ID, instance, exit, or
epilogue meaning. Agent Context v1 retains its envelope schema and now reports
`semaprax.graph.v10` as `source_graph_schema`.

CleanupPlan v2 adds `StageCopyResult` with two closed producers: the ordinary
body expression and an authenticated Try residual. A Try residual binds the
Try and operand expression IDs, exact source and target `Result` instances,
and all compiler-owned Result member IDs. Exactly one producer reaches the
shared postcondition/publication join on each normal path. Physical operation
failure remains the pre-existing sticky status path and must never be repaired
or reclassified as semantic `Err`. Consumers must reject v1 as missing the
staging contract, reject unknown producer kinds, and reject missing, duplicate,
wrong-instance, wrong-expression, or wrong-member staging before backend use.

This cleanup schema change does not broaden
`semaprax.conformance-trace.v1`; its reference executor authenticates staging
but still returns the stable unsupported-result boundary at terminal Copy
aggregate materialization. It also does not open resources, nested/non-copy
Result arguments, residual conversion, `?` in contracts, public aggregate ABI,
or callable/component aggregate signatures. Historical Graph-v6/v7/v8/v9 and
CleanupPlan-v1 text above remains the migration record, not current schema
truth.

## Graph v10/v11 to v12 for generic Copy records

Graph schema selection is now a program-wide ordered lattice. A validated
program containing any generic record declaration emits
`semaprax.graph.v12`; otherwise authenticated Option propagation emits v11;
otherwise legacy and Result-only programs remain byte-compatible v10. Every
Agent Context v1 envelope reports the same program-level source schema even
when its root or selected facts do not reference the declaration that caused
the upgrade. Consumers must reject relabeling between v10, v11, and v12.

V12 generic record declaration nodes carry ordered owner/index-stable type
parameters and `type_id: null`, because an empty concrete instance does not
exist. Generic record constructors retain their exact expression type ID and
add the structured concrete nominal record type with ordered direct
`i64`/`bool` arguments. Field IDs remain template-stable and field template
types retain owner/index identity. Programs without generic record
declarations preserve their prior v10/v11 bytes and KATs.

CleanupPlan does not migrate for this tranche: admitted generic records are
resource-free Copy values, so canonical v2 (or v3 when Option propagation is
independently present) remains sufficient and introduces no generic-record
slot/action. This migration does not open generic functions/inference,
nested/resource/non-Copy arguments or fields, public
aggregate/callable/FFI ABI, or resource admission.

## Graph v12 to v13 for irrefutable Copy-record patterns

Graph schema selection adds a higher program-wide branch. A validated program
containing any authenticated explicit record pattern emits
`semaprax.graph.v13`; otherwise a generic record declaration selects v12,
Option propagation selects v11, and legacy or Result-only programs remain v10.
Every Agent Context v1 envelope reports that same program-level source schema,
even when its selected root does not reach the pattern. A top-level wildcard
record arm creates no bindings and does not by itself select v13. Consumers
must reject relabeling among v10, v11, v12, and v13.

V13 record-pattern nodes bind the exact concrete record instance, stable record
and field IDs, declaration-ordered field entries, recursive nested patterns,
and canonical binding IDs and types. The bounded source form is irrefutable and
has exactly one arm: every record field appears once and contains a binding,
`_`, or another exact record pattern. A binding may receive an entire
resource-free Copy-record field by value. The scrutinee is evaluated once and
the arm result remains scalar `i64`/`bool`.

CleanupPlan does not migrate for this tranche. Record destructuring is
straight-line projection over an admitted Copy value, so it adds no storage
slot, transition, status source, cleanup action, or `VariantCase` edge;
canonical v2 remains sufficient, or v3 when Option propagation is independently
present. Programs without an explicit record pattern preserve their prior
v10/v11/v12 graph bytes and known answers. This migration does not open
refutable or literal patterns, guards, or-patterns, rest patterns, nested
variant patterns,
ownership modes, resource/non-Copy matching, aggregate-valued arm results,
generic functions/inference, or public aggregate/callable/FFI ABI.

## Graph v13 to v14 for bounded generic Copy functions

Any validated generic function declaration selects `semaprax.graph.v14`,
including an unused template; otherwise explicit record patterns select v13,
generic records v12, Option propagation v11, and legacy/Result programs v10.
Every legacy and Agent Context root reports that same program-wide source
schema, and consumers must reject relabeling among v10-v14.

V14 adds exact `function_template`, `function_instance`, and `call_instance`
meaning. A same-schema v14 correction adds the previously missing array
delimiters around function-template `type_parameters`; two-parameter templates
previously produced invalid JSON in module, bounded-context, and Agent Context
projections. Template parameters still retain owner/index identity. A nonpersistent
concrete instance derives from the template declaration ID plus exact ordered
`i64`/`bool` arguments, and its domain-separated execution identity scopes
values and expressions. Only explicitly referenced instances are serialized
and lowered. An unused template is checked over every direct-scalar
substitution and selects v14 without fabricating an instance. Because the
schema name remains v14 while the corrected canonical bytes change, consumers
must migrate to these frozen SHA-256 KATs:

```text
module:        7a61fa6229f2db7aca6a035fd961720e8a401c138cc66c9cd71c64d45bed5efd
Agent Context: 2841401e7ba85fa8e47b3c35a15ae401b4a271d2500d70bbf3627f1453869eb6
context:       d7bda2be1fc366195ffb00a9e20b2b03204b4dd6f46e8019842dd84f70b54ab8
```

These corrected projections are parse-verified locally and hosted green in
[run 31390043736, Ubuntu job
93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296).
The earlier generic execution [run 31385406865, Ubuntu job
93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338)
predates this serializer correction and remains separate backend evidence.

CleanupPlan does not migrate: canonical v2 remains byte/schema/meaning
unchanged and propagated-call status producers retain the persistent template
ID. Exact instance authentication belongs to validated HIR and Graph v14.
Programs without a generic function preserve prior v10-v13 bytes and known
answers. This migration opens no inference, constraints, aggregate/resource/
non-Copy signatures, generic-to-generic calls, recursion, effects, generic
entrypoint, callable/resource/component admission, stable ABI, or CleanupPlan
v4 claim. The separately gated exact private Component v9 profile does not
broaden that Graph migration into general/public Component admission.

## Internal variant-layout digest v1 to v2

The compiler-internal Native64/Wasm32 variant-layout digest now uses the exact
domain `semaprax.variant-layout.v2\0`. Its authenticated preimage adds the full
concrete nominal instance identity plus each payload field's template and
substituted type identities. This prevents two instantiations that share the
same persistent variant/case/field declarations from sharing layout evidence.
Cached v1 digests and known answers are incompatible and must be regenerated;
consumers must not relabel a v1 digest as v2.

This migration changes only the internal evidence digest. Declaration-order
`u32` tags, target-specific size/alignment/offset rules, the one-byte empty
payload policy, and emitted physical representations are unchanged. Neither
digest version is a stable public aggregate ABI, and v2 does not open public
aggregate signatures, resource-bearing variants, or callable/component
admission.

## Normalized status v1 and conformance trace v1

This release introduces two independent public protocol schemas:

- `semaprax.status.v1` is the target-neutral normalized failure record stored behind a context-local ABI token. It fixes the required `schema`, 1–255-byte UTF-8 `domain_id` without NUL, nonzero `code`, `class`, and boolean-or-`"unknown"` `retryable` fields; compiler-owned contract and arithmetic domains have exact versioned codes. The byte bound is normative for source, HIR, native, Wasm, and adapters. Token zero remains success and has no status record. A physical token, arena index, host exception, or opaque diagnostic detail is never part of status JSON.
- `semaprax.conformance-trace.v1` is the canonical semantic event envelope. It fixes resolved function/invocation identities, ordered cleanup places and projections, ownership transitions, atomic call commits, callable/finalizer import events, frame-local `select_failure`, guarded finalization, result commits, and the terminal result/status outcome. Callable import completion may contain success or normalized failure. Finalizer import completion is a distinct success-only Rust variant even though both project to wire kind `import_end` and are distinguished by `site.kind`.

The exact JSON contract, event field order, status tables, examples, excluded physical fields, and outstanding validation requirements are documented in [Conformance trace v1](CONFORMANCE-TRACE-V1.md).

These are first-version protocols, not an in-place extension of an unversioned wire format. Consumers must inspect `schema` before reading any other semantic field. A status consumer must reject any schema other than `semaprax.status.v1`, an unknown class/retryability representation, an empty domain, or code zero. A trace consumer must reject any schema other than `semaprax.conformance-trace.v1`, every unknown event/site/result/outcome kind, and every required v1 field it cannot validate. In particular, consumers may not ignore `select_failure`, reinterpret a finalizer `import_end` as a fallible callable import, sort event/projection/argument vectors, accept physical target fields, or downgrade an unknown future schema to v1. Producers requiring a new event meaning, field meaning, status mapping, or incompatible encoding must publish a new schema rather than silently changing v1.

Trace data must also be bound out of band to the exact validated program, Graph schema/revision, cleanup plan, and scenario. A source revision alone is insufficient as a cache key because Graph and trace schemas can change without changing canonical source. Cache and negotiation keys must include at least the status schema, trace schema, Graph schema/revision, and scenario identity. A consumer must reject a trace whose referenced semantic IDs do not belong to that bound program and invocation path.

Implementation status remains deliberately bounded. Public normalized-status
types, compiler-owned mappings, a context-local status arena, public trace
types, deterministic canonical JSON, independent inventory/HIR coverage and
path-state replay, and a scenario-driven single-frame reference executor exist.
The new `semaprax.semantic-event-dictionary.v1` projection assigns deterministic
nonzero ordinals to exact event shapes and fingerprints its complete canonical
JSON. Generated native callable C and the real narrow Wasm owned adapter emit
actual executed ordinals for the same authoritative 14-case corpus. The private
native loader/authority/ledger host now invokes those generated providers at
O0/O2; independent materialization proves exact reference/native-host/Wasm
traces and outcomes. Unknown or zero ordinals fail closed, and consumers may not
infer or repair events. Descriptor v2 additionally binds
`semaprax.trace-path-certificate.v1`, a canonical compiler-owned trie-DFA that
authenticates the complete ordinal sequence and terminal outcome before host
materialization.

That equality does not make the native resource backend publicly executable.
The ordinary compiler now exposes build-only preflight and a deterministic
hashed shared-library bundle for one explicitly selected direct-trivial owned
function; loading, invocation, adoption, and authority remain feature-gated,
and ordinary native execution still returns `SPX-B104`. General
physical/malformed-response fallback cleanup and quiescence, production
Android/iOS profiles, recursive callee execution, callable imports, imported finalizers,
aggregates, and broader control flow remain absent. The bounded Linux Rust-host
ASan lane is green in [public job
93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).
Native resources, records, and every Wasm resource shape outside the documented
narrow slice remain fail closed.

The Linux
[dynamic-provider sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 ASan+UBSan generated-provider cases through the Rust
host. It did not instrument the Rust host, and unrelated Clippy/GCC failures
kept the overall workflow run red; it adds no Windows evidence.

The internal native invocation context and first-slice trace storage are now one-shot objects that require canonical C zero initialization before their initialization functions are called, for example `struct spx_context context = {0};`. This replaces the earlier accepted-but-indeterminate stack declaration form. Generated entry wrappers and repository probes have migrated. Embedders using `SPX_NO_ENTRY_WRAPPER` must zero-initialize context, trace-buffer, and trace-event storage; reinitialization or storage aliasing is rejected to preserve invocation isolation. This runtime scaffold remains private and does not lift native resource execution.

## Rust AST resource declarations to nominal type declarations

The public pre-alpha Rust AST migration from the earlier graph v4 tranche represents both resources and records through `Program::types: Vec<TypeDeclaration>`. `Program::resources` is removed, and `Type::Resource(String)` becomes `Type::Named(String)` because a nominal reference may name either kind. Graph v5 further changes the resource variant as described above.

The lexer now tokenizes `.` separately so expression projection is unambiguous. Module names, capability/effect names, and named types still accept qualified identifiers through parser-specific `IDENT ("." IDENT)*` rules. Canonical formatting expands record initializer shorthand (`Point { x }` becomes `Point { x: x }`) and preserves initializer evaluation order.

This migration enables `check`, HIR, `graph`, and `context` for records. `build` fails closed with `SPX-B103` (native) or `SPX-W110` (Wasm) until aggregate layout and cleanup semantics land.

## Whole-record to prefix-aware ownership

Resource-containing record projections now carry prefix-aware availability instead of conservatively moving the complete root. Moving one owned non-copy field leaves disjoint sibling fields available. Reusing that field or an enclosing parent reports `SPX-O109`; a place moved on only some control-flow paths reports `SPX-O110`. Existing whole-resource moves retain `SPX-O101` and `SPX-O107`. Borrowed or shared projections cannot cross an owned field or parameter boundary and report `SPX-O108`. Validated HIR independently replays the same rules; Graph v6 additionally exposes the resulting cleanup-plan places, flags, transfers, and guarded exits.

## Web manifest v2 to v3 and Wasm owned ABI v1

Browser packages now emit `semaprax.web.v3`. The existing `module`,
`graph_revision`, `wasm`, `entry`, and `capabilities` fields keep their v2
meanings and canonical order. Version 3 adds one required member:

```json
{"owned_abi":{"schema":"semaprax.wasm-owned.v1","functions":[]}}
```

`functions` is in declaration order. Each admitted entry fixes its persistent
function, resource, and lifecycle IDs; deterministic `semaprax_owned_N` export;
source-parameter ABI kinds; and exact result kind. Scalar-only packages still
use web manifest v3 with an empty array. Consumers must not infer an owned ABI
from Wasm signatures or export spelling, and must reject an unknown
`owned_abi.schema`, a missing field,
an unknown parameter/result kind, or a mapping that disagrees with the module.
A v2-only consumer must reject v3; migration consists of validating the new
object before instantiation, not silently treating it as optional metadata.

`semaprax.wasm-owned.v1` is narrower than RFC 0003 and the Component Model. It
admits one direct trivial-resource identity and a restricted direct body. Its
generated JavaScript facade binds invocation to the exact generated metadata,
uses branded one-shot trusted-adoption tickets, keeps ownership imports private,
authenticates the exact generated Wasm bytes with an embedded SHA-256 digest,
checks canonical argument encodings and aligned result ranges before commit,
and exposes normalized
`semaprax.status.v1` records with the canonical `domain_id` field. Unsupported
resource shapes retain `SPX-W111`. A same-realm `Symbol.for` allocator
coordinates runtime tags across separately evaluated copies of the generated
host. The surrounding realm and that reserved global binding are trusted v1
host state; hostile pre-poisoning, cross-realm, and worker identity isolation
remain outside v1.
The adapter emits compiler-generated semantic event ordinals and the shared
14-case suite materializes them to exact reference/native-host/Wasm traces and
outcomes. The full [owned-resource vertical
contract](OWNED-RESOURCE-VERTICAL-V1.md), Components, imports/finalizers,
broader shapes, public native execution/admission, and the remaining platform/fallback
evidence remain later gates.

## Native adapter descriptor v1 to callable descriptor v2

Native adapter descriptor v1 remains descriptor-only and promises no callable
owner API. Callable admission uses a separate private `SPXNABI2` wire rather
than extending or reinterpreting v1. Descriptor v2 binds twelve
independently domain-separated fingerprints, exact getter and callable symbols,
the required `0x0f` call profile, request/response/event and dictionary bounds,
the complete ordered parameter signature, opaque-`u64` owned payload kind,
and the exact result mapping. The event dictionary and trace-path certificate
are not embedded; their independent fingerprints are.

Private consumers must select a decoder from the eight-byte magic before
loading. They must never pass `SPXNABI1` to callable admission, infer v2 fields
from a v1 function-template hash, accept unknown obligation bits, or repair
noncanonical fields. The compiler's staged encoder and the unpublished host's
independent strict parser are cross-tested, including every-byte mutation,
truncation, and trailing data. The physical ownership host now binds the exact
v2 getter/callable and uses the strict request/response protocol for the O0/O2
14-case corpus. Windows callable/dependency isolation is confirmed in
[run 31257545008, job 93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).
The Rust-host ASan lane is green in [public job
93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).
Mobile profiles, general fallback cleanup/quiescence, and public native
execution/admission remain absent, so this migration does not change
`SPX-B104`.

## Native settlement proof model v1 to v2

The private draft settlement certificate and receipt projections move from v1
to v2. Version 2 replaces the misleading `call_contract` field with the
domain-separated `recovery_contract`, adds exactly one post-commit start plus
typed `Finalize`, `StageOwnedResult`, and trace-bound `CertifyOutcome` progress
edges, and authenticates those fields under new v2 certificate and receipt
fingerprint domains. Draft v1 bytes and fingerprints are intentionally not
accepted or reinterpreted as v2.

This is a private proof-model migration, not descriptor-v3 or runtime wiring.
No settlement file is added to callable-v2 bundles, no loader or host receives
v2 settlement authority, ordinary native resource execution remains
`SPX-B104`, and public callable-v3 admission remains absent.

## Pre-v3 settlement phase model

The hidden `NativeSettlementTransaction` now supplements the atomic proof-model
`settle` helper with closed `Executing`, `DecisionLocked`,
`ActionInProgress`, `ProviderSettled`, model `ReceiptCommitted`, and
`Quarantined` phases. Future callable-v3 consumers must not treat either model
as one physical commit. The protocol has three ordered irreversible boundaries:
`CallCommit`, exact `SettlementDecisionCommit`, and host `ReceiptCommit`. A
host unwind before the decision lock selects
`Abort(HostUnwind)`; after the lock it resumes the exact decision and may not
replace `Accept` with `Abort`. Unknown or conflicting phase evidence
quarantines. An action records `Finalizing` before its effect, and interruption
there quarantines without retry.

The model/provider `Published` disposition means only that a result was
selected in candidate evidence. It is not public ledger publication. Candidate
receipt bytes must be independently validated, exact-instance bound, and
authenticated by host-only authority before one ledger `ReceiptCommit`.
Existing certificate-v2, receipt-v2, `SPXNPRF1`, and callable-v2 bytes are
unchanged by this clarification; none may be reinterpreted as descriptor-v3 or
physical host authority. The phase model allocates and provides no
exact-instance reservation, host authentication, ownership ledger,
allocation-free postcommit guarantee, FFI/provider, loader pin, or physical
finalizer authority.

## Private callable settlement proof v1

The compiler and unpublished native host add the private `SPXNPRF1` proof
envelope. It embeds one exact, unchanged `SPXNABI2` descriptor plus the
canonical binary settlement graph and binds them through separate schema,
v2-byte, graph, and envelope fingerprint domains. The graph also carries the
exact source v2 call-contract and trace-path-certificate fingerprints, which
the independent parser requires to match the embedded descriptor.

This is an additive proof format, not an in-place v2 migration and not callable
ABI v3. `SPXNABI1`, `SPXNABI2`, and `SPXNPRF1` are mutually incompatible; there
is no negotiation or fallback. Existing v2 descriptor/provider/bundle bytes and
symbols do not change. The v2 loader rejects the proof magic before opening an
image, default consumers cannot import the proof surface, and no proof byte
grants loading, execution, adoption, settlement, or finalizer authority.

## Pre-v3 proof to private `SPXNABI3` metadata

The [native callable ABI v3](NATIVE-CALLABLE-ABI-V3.md) is a new metadata
format, not an extension or reinterpretation of `SPXNPRF1` or `SPXNABI2`.
Consumers must dispatch on the full eight-byte magic and exact version, reject
unknown or malformed versions, and never negotiate or fall back. V3 carries its
settlement graph directly and uses new descriptor, graph, wire-schema, ABI,
contract, and symbol domains. V2 and proof bytes, hashes, symbols, bundles, and
public build-only behavior remain unchanged.

The legacy dynamic-loader constructors reject every bounded descriptor
beginning with `SPXNABI3` before path canonicalization, image load, or symbol
lookup. This includes a same-magic blob with a changed version, header size, or
total length; it must not fall through to v2 classification or generic
descriptor loading. A separate private dynamic-v3 constructor now admits only
an exact bounded descriptor whose getter, execute, settle, and returned
descriptor storage all prove canonical root-image provenance, then retains a
separate immutable copy of the admitted bytes. A distinct bounded process-
lifetime static-registration model binds exact descriptor/getter/execute/settle
addresses and target identity without exposing paths, close, or unload. It is
exercised with non-Apple fake functions, and a mandatory macOS gate now requires
the unpublished loader and host static-only path to type-check for five iOS
device, simulator, and Catalyst Rust targets. A new feature-only evidence
harness additionally cross-emits one exact arm64-Simulator provider and the
mandatory job is configured to link and run it against the private host at
`-O0`/`-O2`. [Run 31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
proved that path. This private migration changes no public API or compatibility
promise.

V3 now freezes six provider wire roles and a separate host-only committed
receipt: exact envelopes, tags, checked capacities, a six-argument execute ABI,
payload-bearing frames, digest DAG, and distinct receipt-key HMAC. Independent
compiler and host codecs intentionally replace the former provisional schema
identities, changing private v3 fingerprints, symbols, and known answers while
leaving v1, v2, and `SPXNPRF1` unchanged. V3 `CertifyOutcome` carries the
canonical ordinal/outcome witness and a
nonzero digest over the trace-certificate fingerprint plus that transcript. The
host recomputes this digest and rejects resealed witness/digest mutations, but
does not thereby accept or walk the trace-path DFA certificate independently.
Provider candidate evidence must never be relabeled as a host receipt,
and model `ReceiptCommitted` must not be treated as public ledger
`ReceiptCommit`.

The ordinary emitter derives its target from the compiler build and exposes no
public or general machine-code cross-target configuration. A hidden selector
emits complete target-bound evidence providers for five closed iOS targets and
two closed Android targets. Android dynamic guards now bind architecture,
Android/Bionic, ELF, pointer width, and byte order; the x86_64 Emulator gate is
green in hosted run 31320436726. The bounded
arm64-Simulator and Windows dynamic runtime paths are green. Future iOS device,
simulator, and Mac Catalyst/macabi
profiles must retain distinct target strings even though they share static
registration. No migration may infer physical finalizer success from
`Finalizing`; interruption remains uncertain and quarantined without retry.
Subsequent private additions provide graph-derived providers for all 14 normal
corpus scenarios, a desktop v3 loader, an OS-seeded receipt authority, and a
fixed-capacity atomic ledger/facade. One O0/O2 invocation now connects all 14
normal scenarios through those components and observes zero Rust heap growth
from immediately before `CallCommit` through `ReceiptCommit`; injected decode-
reserve failure quarantines exact evidence and pins. Seven joint failure
fixtures add returned/malformed/interruption/replay/conflict evidence, and
canonical pre-execute unwind reaches authenticated abort receipt commit without
entering provider execute. The private static-registration model has non-Apple
runtime evidence plus a mandatory five-target iOS compilation gate. That gate
is now configured to link and execute one exact arm64 Simulator provider through
the same receipt ledger at `-O0`/`-O2`; hosted observation belongs only to a
green job for the revision. Consumers must not generalize this single standalone
process to device/app lifecycle coverage, the remaining iOS corpus, fatal
allocator/process-crash recovery, or representative or general mobile
application/device execution. These
additions provide no public admission,
general physical-finalizer, or general mobile application/device execution
guarantee. They change no
native execution gate and leave `SPX-B104` closed.

## Private Android JNI handle and status projections v1

The private [Android JNI ownership adapter
v1](ANDROID-JNI-OWNERSHIP-V1.md) introduces two closed projections for its
Kotlin/native boundary. They are fixture protocols, not a stable public JNI or
resource ABI, and they do not replace `semaprax.status.v1`.

`SPXAJH01` is one positive `u64`: reserved sign bit zero, nonzero 15-bit
process-lifetime runtime tag, nonzero 24-bit generation, and nonzero 24-bit
slot. The independent Kotlin/native known answer for tag/generation/slot
`1/1/1` is `0x0001000001000001`. Consumers must reject zero, negative,
reserved, zero-field, stale, forged, cross-runtime, or exhausted-generation
values; they must never reinterpret a handle as a pointer or ownership payload.

`SPXAJS01` is one fixed `u64` status projection: nonzero `u32` code, closed
three-bit class, closed two-bit retryability, nonzero 16-bit manifest-domain
ordinal, and zero reserved high bits. Zero alone is success. Its base known
answer is `0x0000002d00000001`; the declared fixture exception is
`0x0000006b00000007`, and every undeclared JVM throwable maps to
`0x0000004500000001`. Consumers must validate all closed fields and may not
derive semantic meaning from exception class names, messages, stacks, or
objects.

The implementation, source-lock/packaging evidence, and exact API-35 x86_64
APK/Instrumentation path are green in [run 31324497016, job
93272580149](https://github.com/wavect/semaprax/actions/runs/31324497016/job/93272580149).
No migration may infer
GC collection or process-exit cleanup from deterministic
`cleanForTest()`/`cleanable.clean()` evidence, reinterpret non-throwing
`AutoCloseable.close()` as SEMAPRAX's general fallible explicit close, or open
public admission/`SPX-B104`. Since both projections are private pre-alpha
fixtures, future incompatible changes must use new magic/schema identities and
new independent known answers rather than silently accepting old values.

## Private Apple Swift ownership adapter v1

The feature-gated Swift adapter reuses generation-tagged handles and the
callable-v3 receipt ledger, but is not a public handle ABI or framework
compatibility promise. The generated zero-argument fixture is the sole open
entry; migrations must not restore caller-selected evidence hooks or expose the
hidden registration bridge. The bounded hosted Apple gate is green in
[run 31333469714, job
93295293995](https://github.com/wavect/semaprax/actions/runs/31333469714/job/93295293995);
that evidence must not be generalized to public frameworks, physical devices,
UI, or general lifecycle support.

## Private WIT boundary v1

`SPXWIT01` freezes one exact private WIT/schema/JavaScript bundle. Changes to
its WIT text, mapping JSON, adapter bytes, status constraints, or framing
require a new known answer and explicit migration note. This identity must not
be reinterpreted as a Component Model binary or public WIT package version.

The separate private scalar Component Model profile freezes a standards-valid
binary with digest
`sha256:3ed6bed8472eeae0ef17f96458622c9ae032dd7a13b115d2d7fea7fcfecde643`.
Its section order, component function type, canonical lift, export, and embedded
core module are part of the known answer. An incompatible change requires a new
profile/version and migration note. This scalar fixture must not be
reinterpreted as the `SPXWIT01` result/status interface or a public component
package.

## Private source-result component v4

Private Source-Result Component v4 is a new profile, not an in-place change to
the v1 scalar fixture, checked component v2, or Portable Result Component v3.
Its package/interface version is `semaprax:private@0.2.0`, and its sole admitted
export is:

```wit
type language-result = result<bool, bool>;
evaluate: func(value: s64, reject: bool, divisor: s64) -> result<language-result, status>;
```

Consumers must preserve the nested result distinction: source-language
`Ok`/`Err` occupy the inner result, while recognized compiler status occupies
the outer error. They must not flatten language `Err` into status, treat status
as a language payload, or reinterpret the compiler's internal variant bytes as
the WIT canonical representation. Invalid internal tags and unknown statuses
remain traps.

The v4 known answers are:

```text
source revision: sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254
generated core:  54fa2822c51a71cebfd88d379b45c37ffd3d0f0b2893cb4f2966f9e2db6d5e5f
component bytes: 3e7b9c2ddc8ca6fdfa801eb50ae3a21531fce44677345ddea68d20581c79b23b
artifact DAG:    f5fa5ae3905d30c998f783e9b77867986813b0e8b4412fa4afa98e932eda4d40
```

Any incompatible change to the exact source closure, selected signature,
prelude/layout bindings, component type graph, canonical lift, export name,
status mapping, or core/component topology requires another private profile
version and new independent known answers. V4 adds no compatibility fallback
from or to v1-v3 and creates no public component, aggregate, callable, FFI, or
resource ABI. Its exact Wasmtime execution is hosted green in [run 31356536123,
job 93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).

## Private generic-record component v7

Private Generic Record Component v7 is another separate default-off profile,
not an in-place change to any v1-v6 fixture. It freezes WIT package
`semaprax:private@0.5.0`, interface `generic-records`, world
`semaprax-private-v7`, and exactly four exports:

```wit
transform-i64-bool: func(input: duo-i64-bool, delta: s64) -> result<duo-i64-bool, status>;
transform-bool-i64: func(input: duo-bool-i64, delta: s64) -> result<duo-bool-i64, status>;
preserve-phantom-i64: func(input: phantom-i64) -> result<phantom-i64, status>;
invert-phantom-bool: func(input: phantom-bool) -> result<phantom-bool, status>;
```

The exact concrete types are `duo-i64-bool { left: s64, right: bool }`,
`duo-bool-i64 { left: bool, right: s64 }`, `phantom-i64 { marker: bool }`,
and `phantom-bool { marker: bool }`. The two Phantom types deliberately share
a physical field layout while retaining distinct semantic source instance,
layout-digest, component-type, and export identity. Every language record value
is nested in the unchanged outer `result<record,status>` so recognized
arithmetic or contract failure remains physical status rather than a record
value.

V7 authenticates the capability-free exact source table, ordered generic
arguments, concrete Native64/Wasm32 layouts, Graph v12 and cleanup-plan
bindings, source/core/profile/component mapping, and artifact DAG. Its primary
known answers are:

```text
source revision: sha256:2c2c38ae4a6400730bc6c91de659675074020651b9b58bb6a39d047630ef7303
generated core:  d218ff1eaff5f3f677fee58c7b2feb500e9efed8225800cfc3a6562f97d117d8
profile:         7b19f74ab185da90445a042dbd04b6f39f7f9eff3ffff34fc5f0a3bdfd4a9bbf
component bytes: 780a0ccfc35c7ff6d933483711e958d29cfd44c290762b05cd5183e6bf04b5b0
artifact DAG:    c3d1fd10501bfe8dcd4b5c8f24184d127e462b9ca4bc6b1f9422ad8fbcc0b26e
```

Local source-lock, hostile core/component, Node/core, default-consumer hiding,
strict quality, and independent security gates are green. The isolated Rust
1.97.1/Wasmtime 47 typed runtime is hosted green in [run 31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
V1-v6 bytes and known answers remain
unchanged. V7 does not establish general source selection or generic-record
mapping, empty/nested/resource/non-Copy records, imports or capabilities,
callbacks/reentrancy/async, callable/FFI/public ABI, browser/multi-engine
conformance, package/version negotiation, or `SPX-B104`/`SPX-W111` widening.

## Private record-pattern projection component v8

Private Component v8 is another separate default-off profile, not an in-place
change to v1-v7. It freezes WIT package `semaprax:private@0.6.0`, interface
`record-pattern-projections`, world `semaprax-private-v8`, and four ordered
monomorphic preserve/invert exports over exact `Phantom<i64>` and
`Phantom<bool>` records. The two named records deliberately share physical
fields while retaining distinct source-instance, layout, WIT-type, and export
identity. Its primary known answers are:

```text
source revision: sha256:2baac0c0920dbb153789767bf506a4a81713081586a81444d8e5f5a8f5a8516d
generated core:  b6e1dbf9522dbb98df9b6fcd370b562a9a722fcc672d44488aed80f13b7ad39e
Graph v13:      c587415819395e3d618b1e724d639d650e7c55b046f4b77b8bcb5de4ff95682b
plan:           c77c4060fb0b0051af125f4ca353df3a6f5dbd367cdc5ffd61347a7c22847059
Phantom<i64>:   d2ff6084bcfc95701b1dd59835d0ac3af96362e05e56dcadcbd4b8e5dc7d9d80
Phantom<bool>:  3e09cefc7d1ae9bc52ec827debdbcd0753d63bcca994ef776eadb66ba254e67a
profile:         79d4bade38dd3fff9c7145b406bb0bb265ff3ef7cf084edac83384c84610bce2
component bytes: d88590752ed7b08b0f0a32019ba8b4c5fc489d59f06b96986d7ad69e2554a10e
artifact DAG:    e32fe0a15a3458f16aa4da59d87683013dbeba03754966f35e0cb63600e613a3
```

Admission binds exact source, generated core, two concrete layouts, Graph v13,
scratch/result plan, profile, component, and DAG. The exact validator rejects
all six same-signature function-index swaps; only the four polarity-changing
swaps are claimed behaviorally observable. Local independent/upstream,
every-byte, invalid-value/poison, Node/core, source-lock, strict, and security
gates are green. The isolated pinned Rust 1.97.1/Wasmtime 47 zero-import,
empty-linker, no-WASI typed runner is hosted green in [run 31385406865, job
93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268).
V1-v7 bytes and KATs remain unchanged. V8 provides no generic-function
component, general source selection or record mapping, imports/capabilities/
resources, callbacks/async, callable/FFI/public ABI, browser/multi-engine
conformance, package negotiation, or `SPX-B104`/`SPX-W111` widening.

## Private generic-function instance component v9

Private Component v9 is another separate default-off profile, not an in-place
change to v1-v8. It freezes WIT package `semaprax:private@0.7.0`, interface
`generic-function-instances`, world `semaprax-private-v9`, and these six exports
in exact order:

```wit
preserve-i64: func(marker: bool, control: s64) -> result<bool, status>;
invert-i64: func(marker: bool, control: s64) -> result<bool, status>;
preserve-bool: func(marker: bool, control: s64) -> result<bool, status>;
invert-bool: func(marker: bool, control: s64) -> result<bool, status>;
ordered-i64-bool: func(marker: bool, control: s64) -> result<bool, status>;
ordered-bool-i64: func(marker: bool, control: s64) -> result<bool, status>;
```

Those exports select exact Graph-v14 instances of the three phantom Copy
templates `preserve<T>`, `invert<T>`, and `ordered<T,U>`. Consumers must bind
the complete ordered `FunctionInstanceId` sequence, not declaration IDs,
monomorphic wrappers, inferred arguments, or same-signature core indices. One
monomorphic materializer calls all six exact instances and `app.main` checks
their expected results. No authored type, record, or layout root migrates into
the profile. Its primary known answers are:

```text
source revision: sha256:218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c
Graph v14:      62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e
generated core: 9f178207a0406f740198ee8c71d5d008efdf4d995ff04e11e80ea73b79155d44
plan:           edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e
profile:        365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e
component bytes: 3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688
artifact DAG:    2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474
```

Admission binds exact source, Graph, generated core, plan, profile, raw
component, and DAG. Independent validation rejects every byte mutation,
noncanonical unsigned LEB encodings, truncation/trailing bytes, cross-version
confusion, and all 15 same-signature pair swaps; eight swaps change observable
polarity and seven are identity-only evidence. Local core 5/5, component 4/4,
CI-lock 4/4, full gates, and independent security review are green. The
zero-import, empty-linker, no-WASI pinned Rust 1.97.1/Wasmtime 47 typed runner
is hosted green in [run 31392541096, job
93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
V1-v8 bytes and KATs remain unchanged. V9 opens no
inference/constraints, general source selection/export or generic-function
Component mapping, aggregate/resource/non-Copy values, imports/capabilities,
callbacks/async, callable/FFI/public ABI, browser/multi-engine conformance,
package negotiation, or `SPX-B104`/`SPX-W111` widening.

## Private source-Option propagation component v10

Private Component v10 is another separate default-off profile, not an in-place
change to v1-v9. It freezes WIT package `semaprax:private@0.8.0`, interface
`option-propagation`, world `semaprax-private-v10`, and this sole export:

```wit
evaluate: func(input: option<s64>, divisor: s64) -> result<option<bool>, status>;
```

The export selects exact source function
`component.option-propagation.evaluate` plus `app.main`, mapping the
compiler-owned `Option<i64>` through postfix `?` to `Option<bool>`. Consumers
must authenticate Graph v11, CleanupPlan v3, prelude v1, both concrete
layout-v2 instances, and the selected source closure; they must not infer the
profile from a same-signature core function, declaration name, or caller-
provided self-consistent digest. No authored type, resource, template,
instance, import, or capability migrates into the profile. Primary known
answers are:

```text
source revision:      sha256:98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72
Graph v11:            96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d
prelude v1:           d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e
Option<i64> layout:   79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda
Option<bool> layout:  dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038
CleanupPlan v3:       d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f
generated core:       16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec
profile:              f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef
component bytes:      f5770bdfdbc862ea39640b2c706c1d9ea171164c220d18366e25b3219443ad0d
artifact DAG:         90ab80260c84abfe85d1edc666ab3750b81388e6e4cffd7ca21c301b9d0ee589
```

Independent validation rejects every byte mutation, truncation/trailing or
noncanonical encodings, caller-supplied source/core KAT substitution, and
v1-v9 confusion. Typed and raw gates cover `Some`/`None`, contracts, checked
arithmetic, sticky failure, status-first/tag-last publication, full poison,
invalid input/output tags and booleans, unknown status, repeated/fresh
instances, and out-of-band fuel exhaustion. Local core 5/5, component 4/4,
CI-lock 4/4, full, hostile, and security gates are green. The zero-import,
empty-linker, no-WASI pinned Rust 1.97.1/Wasmtime 47 v3-v10 runner is hosted
green in [run 31396483313, job
93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502).

V1-v9 bytes and KATs remain unchanged. V10 opens no general source
selection/export, general `Result`/`Option`/`?` or algebraic Component mapping,
nested/resource/non-Copy carriers, imports/capabilities, callbacks/async,
callable/FFI/public ABI, browser/multi-engine conformance, package negotiation,
or `SPX-B104`/`SPX-W111` widening.

## Revision token FNV-1a64 to SHA-256

Graph v3 and later, semantic patch bases, CLI output, and `semaprax.web.v2`/`semaprax.web.v3` manifests use one algorithm-tagged token:

```text
sha256:<64 lowercase hexadecimal digits>
```

The digest input is exactly `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. The domain separator and canonical projection are part of the protocol. Paths, comments, and formatting-only differences do not affect the token; semantic source changes do. This is collision-resistant content addressing and stale-base detection, not source authentication.

Legacy `fnv1a64:` patch bases, graph caches, snapshots, and web manifest expectations are incompatible. Regenerate them from the current source. SEMAPRAX deliberately does not accept an FNV fallback: an old patch fails with `SPX-G409` before modifying its source. Web consumers must reject `semaprax.web.v1` when they require the SHA-256 revision contract.

There is not yet a stable compatibility guarantee. Before 1.0, every breaking public syntax, CLI, diagnostics JSON, graph, patch, web manifest, package, component, or ABI change must add a section here and update the changelog.
