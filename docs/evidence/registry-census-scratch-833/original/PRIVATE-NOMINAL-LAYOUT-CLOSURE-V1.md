# Private Nominal Layout Closure v1

Status: implementation and owning regression definitions authored; owning executable qualification is pending.

Audience: internal library authors, compiler contributors, and reviewers.

An ordinary checked library caller may import an explicit nongeneric root type
or an exact provider function whose signature carries that root, without importing
every private record reached through its layout. For example, an explicitly imported `Decoded`
result can carry `Report`, containing `Config` and `Vec<Row>`, while source names
only `Decoded` and its cases. This is internal layout reconstruction, not a new
source import, public descriptor, capability, graph edge, or lifetime grant.

## Authority and visibility

The current source-authenticated provider supplies each persistent declaration,
case, and field identity. Authored type names, constructors, and patterns still require explicit ordinary
type imports. A function-only signature root permits inferred local values to be
forwarded and borrowed field projections to be checked against its actual layout;
it does not publish the provider type name in the caller. Descendants must independently satisfy the existing
resource-free, invariant-free, nongeneric record shape, cycle, depth,
field, owned-leaf, and exact compiler-Vec checks. A nested variant, resource,
generic descendant, borrowed leaf, altered Vec identity, or unsupported record
vector element remains refused. Existing 64-depth, 4096-field, 256-owned-leaf,
vector, and builder-byte limits remain unchanged. Inferred roots in this MVP are
records or the exact two-case record-payload plus scalar code/offset/field
refusal outcome. Other outcome payload families retain their existing explicit
import route.

One invocation-local borrowed closure resolves only descendants reached through
an admitted explicit type root or exact imported function-signature root. It prefers an existing explicit alias. Otherwise it
mints a normal source identifier fresh against all authored identifiers in the
caller: imports, declarations, parameters and generic parameters, all type
references, local bindings and reads, constructor and field names, match and
nested record patterns, closures, and traversal aliases/indices. The MVP census
covers plain record/variant/function modules. Interfaces, classes, resources,
protocols, implementations, sessions, Agent declarations, and session-following
functions refuse inferred private layouts until their full name census is
implemented. Their existing explicit-import route is unchanged.

The new names are used only in copied synthetic declarations, field types,
function prototypes, and checking-only defaults. Authored AST and `module_uses`
are never rewritten. An unimported authored `Report` annotation, `Config`
constructor, or child pattern remains unresolved; a local declaration with that
name cannot capture the provider's child. No lexer or `SPX-S106` exception
recognizes a private prefix. Original graph import/reference edges remain
source-derived and contain no invented child import.

Minted names are never diagnostic text. Before fresh or selective resolution
of a caller's synthetic program, the graph borrows that caller's already held
closure (no derivation or lookup grant), checks that the copies at
the end of that exact synthetic program are the declarations it appended, and
lends a borrowed view to that one verification through its type table. The
view holds only references, so binding it allocates no slot backing or string
copy and adds no builder charge; there is no thread-local scope to nest or
restore. A copy is identified by its exact address in that exact synthetic
program, and its provider module, persistent identity and provider-declared
name are read from the held provider source only while a diagnostic is
constructed. The source verifier consults this view only when rendering a type
it already resolved: an
`SPX-M101` missing case on a private variant names only the case plus the
provider variant's context, and record-pattern, scrutinee, call-argument,
return, binding and branch/arm-join type mismatches render a private type as
`Name [unimported @id("id") from module]`. Declaration checks also walk the
appended copies and import stubs, and generic validation specialization
substitutes `let` annotations, so declared binding types, type-argument and
copy-argument arity, string-bearing record and parameter-mode diagnostics use
the same rendering, and their spelling helps are withheld for a private type,
which has no source spelling. Codes and authored or explicitly
imported names are unchanged, and there is no prefix filter or message
rewriting, so an authored colliding spelling keeps its authored rendering. A
missing-case help offers `use type @id(..) from module as Name;` only when the
provider declaration has an explicit identity and the caller has no import,
type or function already spelling `Name`; otherwise it asks for an unused
alias, and without an explicit identity it offers no action. Whether `Name`
is free is decided from the authored caller only while that help is built.

## Storage and replay

The borrowed provider references remain tied to the held immutable source set.
A bounded two-pass traversal reserves its real frame backing before allocation;
its single shared work budget covers both passes. The private entry directory
reserves its exact requested capacity, and each private name reserves complete
String capacity before writing. The identifier census uses precharged fixed
expression/type/pattern frame vectors and one shared work counter. It does not
serialize a whole Program or recursively format Debug output. Existing complete
provider-import sets bypass private-name and directory construction. If the
optional absence scan exhausts its bounded work, inference is disabled before
creating any aliases or directory; ordinary explicit-import validation still
runs and missing descendant aliases remain refused.

Synthetic clone forecasts include every copied private declaration, its field
and case metadata, string backing, identities, and rewritten aliases/defaults.
A growing synthetic type directory reserves the full replacement while the
previous directory is live. Private monomorphic field rewriting borrows field
types directly and does not build temporary field-reference vectors.

Each fresh build and source/cache comparison reconstructs the closure from its
current authenticated caller and providers. A stored private alias or closure
set is not authority. Actual provider bodies remain the linked implementation;
checking-only defaults cannot replace them. Existing independent HIR layout,
loan, cleanup, source-fact, cache, and selected-target validation remains required.
This does not widen public Project/package ABI admission or frozen source-less
`from_parts` authority.

## Owning definitions and qualification

`workspace_graph::expected_projection::private_layout::tests::` defines four
owning bodies: omitted private descendants with real provider evaluation and
exact Vec facts; full name hygiene and source visibility; provider identity,
layout and body drift; exact/one-short frame, directory and String reservations.
The `private_layout::authority::tests::` child adds two definitions: exact/one-short
borrowed authority storage and fresh/retained signature parity with missing,
stale and forged private fields. Proven complete provider type imports require
no new backing.
The original owned-function outcome gate retains complete HIR validation and
forged Vec-parameter refusal while checking descendant omission. The existing
text-collection hostile shapes remain refused. The workspace source-boundary
gate joins the new implementation and test children.

The `private_layout::function_tests::` child adds five definitions covering
function-only forwarding and borrowed nested String/Vec projections, exact root
and descendant identities, absent sibling/ancestor/source-name authority, stale
provider pointers, both signature parity paths, and exact/one-short storage.
The same linked source has interpreter, native O0/O2 and strict CoreWasm owning
success and failed-postcondition controls with repeated invocation and zero live
native/String/Vec storage; no production source or backend ABI is substituted.
The fourth definition compares complete imports with all three private descendant
imports omitted on the same `Decoded.Ready` provider. Both callers check exact
Unicode/NUL-containing encoded String bytes before and after forwarding the
Report; `Decoded.Error` has a distinct result. Interpreter, native O0/O2 and
strict Wasm repeat success, provider-body drift, and exact failed encoder
postconditions. Native retains the poisoned result cell; Wasm requires the exact
failure with no returned value. Both keep their zero-owner observers. Current source and
retained Ready-field type mutations remain independently refused. A fifth
definition removes every type import from the same Ready caller: the exact
private outcome layout is still retained, the unimported `Decoded` patterns
stay rejected, no diagnostic contains any alias the caller's own closure
minted, and applying the offered import is the succeeding explicit-root
control. A caller already declaring `Decoded` receives no fabricated alias, and
inferred record-pattern, call-argument and colliding authored-spelling callers
keep provider context without exposing aliases, as do a mismatched return and
a private descendant nested in a declared-binding payload. A registry
definition binds the held presentation under a zero-byte builder budget and
the allocation observer: zero allocations, frees and live bytes through drop,
no receipt change or re-derivation. Entry and identity comparisons debit the
same held invocation work ledger before reading their bytes; exact and
one-short work cases preserve sticky refusal. Short, renamed and
foreign-identity copies are refused. A verifier child binds a view to one
program and proves that an equal clone, an unbound pass after it, and a view
bound past the copy all keep ordinary text, and that an authored declaration
sharing the private prefix keeps its authored spelling. These are
authored gates; current-head execution is still required for qualification.

Function-root admission consumes an invocation-held, precharged ownership and
signature-site directory. A shared bounded postorder walk computes each reached
nominal declaration's ownedness once; exact current function/type pointers bind
its signature sites to the immutable caller, provider set, and authored index.
The compiler Vec wrapper retains its own ownership and flat element checks.
Binary lookups compare stable function identity and signature ordinal, then
authenticate exact held signature pointers,
debit the held work remainder and allocate no new frames, cloned IDs,
sets, or backing. Exhaustion never grants a private fallback. Primitive signature
leaves use the old scalar/resource classification without a pending Vec.

`private_layout::owner_index::tests::` adds three owning definitions: differential
comparison with the independent old walk, wide logarithmic probes, and
allocation-free repeated queries;
exact/one-short complete construction, shared lookup work, and observed absence
of legacy fallback after refusal; and duplicate
provider names/aliases plus cold/reconstructed layout and retained-HIR drift.
One lazy invocation-local registry now holds each selected caller directory across
import validation, retention and cached/uncached forecasts, synthetic stub
construction, and core retries. Exact caller, provider-slice and authored-index
pointers bind every lookup. All caller directories share one work ledger and
sticky refusal; a failed query poisons subsequent derivation and queries.
Preparation reserves actual directory, frame, name and shared-ledger backing
before allocation. Its monotonic receipt is carried into every core attempt and
retention forecast, including retained overlap. The no-inference route creates
no registry backing. There is no global cache or serialized validation lease.

Entry, nominal-fact and signature-site ordering uses allocation-free fallible
heapsort. Identity-byte comparisons and swaps debit the same remaining work
before execution; refusal never publishes an incomplete directory. The four
`private_layout::registry::tests::` definitions compare held and independent
projections, exercise a real invocation with 64 signature sites, exact/one-short
preparation and overlap, independent ordering, and interleaved callers sharing
one sticky ledger. Direct forecast controls cover cached and retry modes;
existing owning cache/retry gates remain required for actual phase qualification.

Current private type authority (#765) no longer rebuilds an owned top-level
function/type map per validation caller. Each ordinary, compact and declaration
fact caller passes the invocation's own `index_authored` map for the same
source slice. The `authority` child's borrowed view first proves that the caller
is a current member of that slice by address. One exact lookup per old top-level
member, charged by the old per-member debit, then requires the entry's key,
function or type pointer, path, module, kind and explicit-identity bit to match
that member. The held index size must equal the slice's complete authored
census. The census also counts fields, cases, case fields, lifecycles, class
methods, interfaces, interface imports, protocols and protocol methods. A
separately charged pass over the whole index then requires its function/type
entry count to equal the old member count, so a same-size replacement of a
child entry by a foreign function or type refuses. Each lookup admits only a
function or type entry keyed by its own identity, so the wider index answers
exactly the old map. A slice holding class methods
keeps the old owned map, because a method shares the function kind in the held
index.
The class scan, variant cases, lifecycles, interfaces, protocols and the
whole-index kind census are newly charged work on the unchanged per-invocation
work limit. The legacy path's
authority-index request report is unchanged. A borrowed derivation reports the
old byte formula only through a separate test-only label, never as a request.
Root/cursor reservations, sticky `contains` and signature work, duplicate and
foreign refusal are unchanged. No digest, layout witness, cache, refund or new
quota is involved.

The `private_layout::authority::view_tests::` child adds five owning
definitions. The first runs a differential against the saved pre-#765 owned-map
oracle, comparing verdicts, root pointers and the exact extra proof work. Its
cases include explicit aliases and held field, case, lifecycle, interface and
interface-import identities that the old map omitted. The second keeps the class
method fallback differential-equal. The third refuses stale, wider, narrower,
detached-caller, relabelled and duplicate bindings. The fourth drives the proof
directly with a current member caller. It covers inner-lookup parity, same-size
foreign function/type replacement of child entries, and current-slice module,
path, explicit-bit, kind, declaration-pointer, key-pointer and missing-member
forgeries. The fifth checks exact and
one-short builder bytes on both paths. Its allocator observations cover borrowed
frame/root backing only, allocation-free queries, complete release through Drop
and the saved map's larger backing. Storage elimination is a source property;
any CPU, application fit or agent cost effect needs actual observation.

These definitions have not been executed for this tranche. No current-head
interpreter/native/Wasm, cache, benchmark, source-size saving, or matrix promotion
is established by this document. The owning grouped qualification must run the
new bodies, existing imported-library and cache/replay controls, and selected
Project/backend gates before a support claim.

Related contracts: [Project v32 Nested-Outcome Command v1](PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md),
[Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md),
[Semantic Kernel v1](SEMANTIC-KERNEL-V1.md), and
[Architecture](ARCHITECTURE.md#internal-owned-data-libraries).
