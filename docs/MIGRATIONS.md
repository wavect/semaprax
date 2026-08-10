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
resource ABI. Its hosted Wasmtime evidence is pending.

## Revision token FNV-1a64 to SHA-256

Graph v3 and later, semantic patch bases, CLI output, and `semaprax.web.v2`/`semaprax.web.v3` manifests use one algorithm-tagged token:

```text
sha256:<64 lowercase hexadecimal digits>
```

The digest input is exactly `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. The domain separator and canonical projection are part of the protocol. Paths, comments, and formatting-only differences do not affect the token; semantic source changes do. This is collision-resistant content addressing and stale-base detection, not source authentication.

Legacy `fnv1a64:` patch bases, graph caches, snapshots, and web manifest expectations are incompatible. Regenerate them from the current source. SEMAPRAX deliberately does not accept an FNV fallback: an old patch fails with `SPX-G409` before modifying its source. Web consumers must reject `semaprax.web.v1` when they require the SHA-256 revision contract.

There is not yet a stable compatibility guarantee. Before 1.0, every breaking public syntax, CLI, diagnostics JSON, graph, patch, web manifest, package, component, or ABI change must add a section here and update the changelog.
