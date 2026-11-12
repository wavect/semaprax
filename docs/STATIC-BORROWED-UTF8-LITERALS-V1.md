# Static borrowed UTF-8 literals v1

Audience: compiler contributors and reviewers.

Status: focused local qualification for the direct-source and retained Project
profiles in OPT #740. The later retained composition profiles and remaining
OPT #745 obligations are described below.

The adjacent form `str"priority"` denotes immutable UTF-8 with type `borrow str`.
An ordinary `"priority"` continues to construct an owning String. The contextual
identifier `str` remains an identifier elsewhere; `str "priority"` is not a
literal. Both literal forms use the existing decoding and canonical escapes.
Empty input, NUL, Unicode scalars, quotes, and backslashes retain their exact
bytes. There is no interpolation, normalization, implicit String conversion,
new builtin identity, or runtime allocation operation.

A direct literal can initialize an immutable borrowed-text local, be retained
through existing immutable local aliases, and be passed to an admitted
synchronous borrowed-text parameter. Named `str_as_bytes` views use the existing
checked range and loan rules. These descriptor-only operations can occur inside
an otherwise admitted loop. Ordinary borrowed results, aggregate storage,
mutable aliases, callbacks, suspension, and view escape remain closed.

HIR stores a distinct `BorrowedUtf8Literal` leaf with immutable, module-held
`StaticUtf8` backing. Its type is Str and ownership is Borrow. `SourceExprStatic`
provenance binds the actual literal expression and exact local descriptor,
byte length and range. The descriptor does not fabricate owning storage or
acquire a cleanup finalizer. Cache decoding reconstructs the immutable payload;
independent validation rechecks the type, ownership, backing and view extent.
A graph schema, literal-looking name, hash, or attached plan cannot grant root
or backend authority. Stale source graphs are replayed against current source.

Selected programs use prelude v16 and Graph v77. The graph's
`semaprax.static-borrowed-utf8.v1` inventory is descriptive, with `authority:false`.
The current full-Program graph route composes existing helper-renewal facts.
Legacy and detached/from-parts graph entry points refuse the new root with
SPX-G410, including uninstantiated templates. The retained Workspace/Project route constructs a rare source-site inventory
while the authenticated AST is still held. Each fact binds the source owner,
identity origin, span, canonical expression path and independent immutable
source bytes. Later selection independently matches current retained functions,
generic templates and concrete instance execution identities against these
facts and the current namespace. Missing, duplicated, changed or misplaced
sites fail SPX-G410. The selector still checks ordinary renewal profiles; no
stored bool, schema or signature grants literal authority. This route includes
an authored normal authenticated command Project case; public scalar/text/data
provider profiles remain closed.

Selection comes from the mandatory HIR validation walk. Unselected modules
create no literal-site inventory or selected-module vector backing. A selected
module reserves its source cursor, borrowed scratch, site carrier, source-byte
backing and cloned identity strings before construction, including spare
capacities. The rare optional module field is counted in the existing module
carrier size. Source and typed-tree walks and site matching have separately
bounded work domains; this is not a claim of zero extra work in selected modes.
The existing helper census now also covers contracts and templates so removing
a source inventory cannot hide static sites behind the helper schema.

One literal may contain at most 65536 decoded bytes (source SPX-T314).
Independent mandatory HIR validation also sums every represented decoded static
literal, ordinary String literal and literal-format template, including generic
templates and concrete instances. If any static literal is present, this
conservative occurrence sum must be at most 65536; excess or forged HIR is
SPX-H006. It takes no guessed deduplication discount. The source verifier checks
the per-literal bound; aggregate pool admission is currently the independent
HIR boundary. Old programs do not select the new prelude or pool merely because
they contain owning Strings. The mandatory HIR walk gathers selection without
an additional absence cursor or heap inventory. Detached graph refusal performs
its own bounded, charged census; it has no current full-source authority.

The compiler reserves the actual immutable carrier and String backing before
allocation. The reference interpreter clones the held immutable reference when
evaluating a literal; it does not construct an owning String or copy the literal
payload on every evaluation. Static module bytes consume no invocation-input
budget. An actual host argument remains foreign input even if its bytes or
address happen to coincide with a literal. No compiler-side reservation or
logical allocation count is a physical runtime measurement.

Native lowering emits read-only constant storage and a length-aware descriptor,
with no owning constructor or drop. Its decoded occurrence bound is not a claim
about C terminators, alignment, linker deduplication, or physical constant bytes.
Internal aggregate Core-Wasm uses the immutable interval starting at 196608 in
four fixed pages; command lowering uses 393216 in seven fixed pages, disjoint
from output and transcript storage. An empty literal points at the interval's
base, including at the exact full-pool boundary. Internal reads authenticate the
selected interval; foreign-entry fences remain unchanged. Public scalar/text/
data adapters, opaque standalone String arenas, and selected-final-UTF8
certificates explicitly refuse static literals until their own profiles support
the root. No host permission, ABI, raw input limit, runtime counter or memory
cap is increased.

Focused owning gates:

- `hir::static_text::tests::`: source/canonical/current graph/cache round trip,
  stale graph, exact/+1 pool, backing exact/one-short reservation, forged view,
  immutable-local refusal, interpreter reentry and frozen-profile refusal.
- `static_utf8::` in the owned-data harness: native O0/O2 allocation interception,
  Core-Wasm immutable-pool reads, public-profile refusals, and the shared
  `static_utf8_writer.spx` helper before and inside loops.
- `wasm::aggregate::tests::static_text::`: exact pool and empty-literal placement.
- `workspace_graph::source_schema::static_text::tests::`: retained writer,
  stale source content/identity/namespace/presence, exact per-instance site
  bijection, and the explicit retained lexical-renewal composition gap.
- `workspace_graph::static_text::tests::`: exact/one-short cursor reservation
  and absence without selected backing.
- The `static_utf8::` Project definition authenticates the held source,
  executes the existing writer, and refuses drift at final recheck.
- `collection_response_generated_static_keys_match_direct_and_indexed_renderers`
  derives and verifies a collection-response codec, then uses explicit static
  roots for three generated key sites. The root encoder calls the direct
  metrics renderer once and the indexed row renderer twice. The original
  codec and the explicit-root fixture each match the independent JSON oracle
  on the interpreter, native C11 O0/O2, and strict Core Wasm.

A separate matched two-row run changed two generated sites for a 64-byte
field name. Each canonical key fragment went from 532 source bytes to 139
and carried the same 67 decoded bytes. The complete generated source went
from 19,569 to 18,783 bytes. Native execution made exactly 303 output byte
writes for the 303-byte JSON result in each of the four runs. The emitted C
source went from 420,389 to 419,276 bytes, and the Core Wasm module went from
42,753 to 44,855 bytes. Agent token and cost qualification requires fresh
matched accepted-task cohorts.

A selected retained composition now has authored implementation and regression
definitions for module-local primitive signatures, static literals and lexical
Bytes renewal. Its private inventory is captured only at the checked AST-to-HIR
boundary, binds exact authenticated source bytes, and stores a domain-separated
SHA-256 projection of every function's complete typed requires/body/ensures,
identities, calls, operands, types and spans. Later replay compares current
namespace/signature/body facts, rebuilds a minimal declaration index, and
independently validates the complete cleanup, status, loan and ownership route.
Attached certificates and graph versions grant no authority. The legacy route
without that inventory still refuses the composition with SPX-G410.

The selected replay cumulatively reserves constructor backing before growth;
work-budget resets cannot refund the byte reservation. The tree-node upper
bound is tied to the pinned Rust standard-library layout and Global allocation
request contract, with separate test-only allocator observations; it is not a
universal Rust layout or RSS guarantee. Refusal diagnostics allocate separately
and are reported separately in the owning observation definitions. Exact and
one-short tests establish preconstruction refusal, not a claim that diagnostics
fit inside a tiny failed replay budget.

An additive selected route has authored implementation and owning regression
controls for imported pure monomorphic Copy-scalar helpers. The defining module
captures its own checked source revision, explicit function identity, exact
parameter modes and scalar-result signature and complete requires/body/ensures guard before HIR retention, including
providers without static literals. Import stubs and caller aliases grant no body
authority. Replay authenticates each demanded provider against the current
namespace, exact defining module/path, signature and body, rejects cycles, and
borrows the actual defining functions through one indexed context. Equal names in
different providers remain distinct by persistent ID and authored alias.

One non-clone preparation owner pays authentication hashes, borrowed comparisons,
closure frontiers and declaration reconstruction before transferring its remaining
work and cumulative backing into the module replay session. Each helper obtains a
fresh function-bound lease and undergoes complete structural, status, path-state
and exact observation replay. Checked arithmetic and pre/postcondition failures
remain part of the proof; purity is not infallibility. Failure prevents restarting
the preparation or session. Independent saved local controls, provider-body and
plan mutations, aliases, stale/missing/duplicate providers and exact/one-short
whole-entry-to-Drop ledgers are authored but unexecuted in this batch.

The imported consumer route admits primitive signatures and no local nominal type
inventory; the existing no-import route retains its separately specified record
profile. A further authored tranche admits Borrow Str/Slice parameters and Value scalar
parameters with scalar results. View expressions must name the exact current
borrowed parameter; StrAsBytes lowers to its authenticated BorrowPlace. Only
byte length and Str length/empty/prefix/contains reads are permitted intrinsic
calls, with exact argument and result shapes. Borrowed aliases, ranges, owned
roots and borrowed results remain closed, and the helper must retain its exact
empty v1 loan plan. Ordinary scalar While statements have no lexical exits.
The cumulative preparation ledger also pays every root-identity comparison.

Helper-local lexical control, templates/instances, owning helper parameters,
nominal/Vec/Option carriers, other intrinsics and dynamic callable providers
remain closed. Exact scalar `i64_from_u8` is also admitted by its compiler operation identity,
argument mode/type and result type. Independent exact Copy Option<u8> predicates,
canonical case/field/type-parameter reconstruction, source-digest pattern coverage
and ordinary While reader controls are authored groundwork. Their receiving
profile remains explicitly closed until cumulative schema and inventory lookup
work is metered; neither these predicates nor their test-only index constructor
grant production Option admission.

Full OPT #745 remains open for these provider,
loan/path, storage/instance and exact intrinsic metadata obligations. No wider
production or executable qualification is claimed.
A separate private inventory constructor profile supports exact current
nongeneric flat records with 1..8 direct Copy/String/Bytes fields, under a
32-declaration lookup bound. It compares declaration and field identities,
owners, names, types, order and ordinals to the current checked declaration
index before cloning. Field carriers, identities, lifecycle strings, flag and
projection vectors are reserved before construction. This adds constructor
support only. A further private factory captures exact module-local record and
field identities, names, spans, types, order and identity origins while checked
authored source and typed HIR are held. Later context construction independently
compares the full current layout and namespace before reserving its minimal
declaration index. The source-bound typed digest now includes nongeneric nominal
types, record construction and field projection. Structural leaves and typed
constructor/projection skeleton accounting must still be complete before
retained record replay can be admitted. Record census, field comparisons and
named signature lookups use bounded cumulative work; retained carriers and
index backing are reserved before allocation. No imported or generic record
authority is captured. The selected retained entry now also admits authenticated module-local,
nongeneric, invariant-free direct records with 1..8 Copy/String/Bytes fields.
One bounded borrowed current-declaration classifier is shared by inventory,
whole-record call epochs, typed drop/call/result replay and structural field
leaves. It checks exact nominal identity, namespace ownership, field IDs,
indices, order and types without materializing generic type facts. All field
shape, flag, lifecycle, projection, map and clone backing is reserved before
construction. Ordinary partial-constructor failure, staged argument commit,
branch/loop exit and owned provisional-result publication retain their complete
canonical cleanup order; submitted plans are never sorted or repaired.
Source-layout and full current-body replay remain prerequisites, not evidence
capsules or cached verdicts. Nominal updates/destructuring, variants, imports,
generics, nested records and collection carriers stay closed. A direct record
result is admitted only through the same owned provisional-result route.

Authored definitions include genuine nominal make/identity/read lexical
functions, exact/one-short physical closure and independent stale layout/body,
field, call epoch, flag, projection and finalizer-order attacks. Ordinary String
allocation failure remains its existing fail-stop contract; this route does not
claim production recoverable String OOM or completion of OPT #724.

All new definitions are unexecuted in this implementation tranche.

The 31 retained static-text checks passed on the compiler sources at
`18142c8d0`, including exact and one-byte-short replay, imported provider and
layout drift, and borrowed-record source-for controls. The authenticated
command Project with static text and lexical Bytes renewal passed at Graph v77.

The fixture from `06e2ca7f3` reproduced a remaining #745 gap. Direct source
emitted and independently verified Graph v78 for the module combining static
lexical renewal and finite owned iterator scratch. The authenticated Project
returned `SPX-G410` from retained static source-site authority.

The static source-body encoder now binds the Bytes iterator's typed calls,
Done and Yield cases, generated field bindings, and all four owning-latch
identities. These entries use the existing bounded digest and work debit.
Earlier static body encodings keep their exact bytes. The authenticated
module-local static receiver now checks the exact Bytes iterator call types,
argument ownership, Done and Yield patterns, and the independently replayed
latch under its existing paid preflight. The cleanup inventory independently
rebuilds the iterator drop leaf, the case-qualified Yield item and remainder
flags, and the staged owned argument slots for `vec_into_iter` and `iter_next`.
The selected cleanup leaf index verifies the exact Done and Yield layout and
retains the case-qualified paths. Structural block and edge replay checks the
whole typed Bytes step places for variant transitions and the owned iterator
call arguments. Selected exit replay derives the exact leave prefix for an
authored Continue from the current region ancestry and the remainder's Yield
scope. Generated latch endings use their ordinary loop boundary. Conditional
finalizers bind the exact Yield case and field. Selected path replay now
tracks the conditional Done/Yield state through initialization, transfer and
case authentication. It compares the exact guarded cleanup order at exits and
execution boundaries. The independent typed-HIR path comparison authenticates
the closed Bytes iterator call parameters and match arms on the same ledger.
The original Graph v78 Project passes. A Project entry that runs both lexical
Bytes renewal and the owned iterator returns 10, matching direct interpreter
execution. Direct Graph v78 bytes and the retained Graph v77 check and
interpreter receipt match their earlier baselines.


The OPT #887 failure-37/109 source-only correction keeps generic lexical exits
outside the admitted template profile (the corrected source is SPX-T226),
while testing an admitted scalar/static template beside a concrete lexical
function. A separate hostile typed-HIR donor case isolates the uninstantiated
template census with no lexical concrete member; it grants no source admission.
The complete old selector is independently compiled from authentic root-level
bytes in `src/workspace_graph/source_schema/static_text/tests/before_static_lexical_selector.rs`,
with only named visibility, function-name and old-helper glue. Its exact source
check also retains the entire original test prefix and production owner check.
This child belongs in the physical/source inventory union. These corrections
are authored and await owning runtime qualification; no new support is claimed.
