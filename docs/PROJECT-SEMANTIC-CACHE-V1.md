# Project Semantic Cache v1

Status: implemented bounded checked semantic reuse; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
Full incremental checking remains separate; authenticated cross-process HIR
reuse is implemented in the separately versioned Persistent Semantic Cache v1.

Audience: compiler contributors, embedding hosts, and semantic workspace agents.

This opt-in cache belongs to one invocation and keeps compiler-checked module
HIR alongside source ASTs. A checked-module hit clones the HIR instead of
calling that module's `hir::resolve`. A changed module may also reuse exact
monomorphic free-function HIR when its complete non-body environment and every
field of that function except its body remain equal. Both lanes rerun source
verification, HIR validation, and the complete cross-file, import-stub,
linking, graph, and Project-profile admission gates. This reuses exact facts within fixed bounds. It is not a general incremental
compiler or proof that source edits preserve behavior.

Project finalization reuses the exact retained source AST for source Agent
extraction and prelude-bound source revision construction/replay. Each borrow
checks both source bytes and path; it neither clones the AST nor adds a frontend
cache hit. The revision hashing inputs and independent preflight fact checks
remain unchanged. A parser-call regression covers unchanged calculator builds
in both cache modes; this is local work-elimination evidence, not a measured
speedup or a claim that every Project profile performs zero parser calls.

## Host API and compatibility

`ProjectFrontendCache::new_with_semantic_cache()` selects checked-module reuse.
`is_semantic_cache_enabled()` reports that choice. The existing `build(manifest,
sources)` API consumes bounded canonical source proposals and returns the
admitted immutable Project revision plus descriptive work counters.

`ProjectFrontendCache::new()` and `Default` retain their existing AST-only
behavior, frontend report schema, and zero checked-HIR reuse. No caller silently
changes semantic admission mode. Both constructors are invocation-owned and
open no filesystem root or grant source, test, execution or publication
authority. The separate [persistent cache](PERSISTENT-SEMANTIC-CACHE-V1.md)
can now restore compiler-sealed HIR through a host-selected authenticated store;
there is still no public untrusted HIR constructor or implicit cache discovery.

`ImageWorkspace::with_semantic_cache(image)` independently primes a checked-module
cache against an already admitted image. `refresh_owned_sources` then admits
proposed canonical source directly without a preliminary cold changed-revision
build. An unchanged image can keep its existing `Arc`; invalid proposals preserve
both the previous image and cache. Host sessions and their startup policy are
separate adapters over this library behavior.

## Exact reuse and invalidation

The cache binds compiler package name/version, semantic-cache compatibility,
and the entire canonical Project manifest. Any context change resets all
entries. Compatibility is not a compiler binary fingerprint.
The separate persistent-store envelope additionally binds the compiler
executable bytes under its explicit trusted static-installation contract.

Changed, added, and removed source paths still seed the conservative reverse
import inventory. An exact synthetic program can reuse its complete checked HIR.
A nonexact module is retained only as a private function-reuse candidate and
cannot become a module hit. Independent unaffected modules can still reuse
checked HIR.

A hit additionally requires exact equality of the synthetic `Program` presented
to the resolver, including authored declarations, bodies, identities, spans,
and imported stubs. Source hashes or signature summaries alone cannot establish
that equality. A provider's signature or an importing module's alias/binding
change therefore cannot reuse stale resolved calls. Module body edits cannot
reuse their own previous checked HIR. Reuse does not authorize caller-supplied
AST/HIR or bypass the independent retained-HIR checks.

Function reuse is limited to top-level monomorphic free functions. Module path,
module identity, imports, permits, types, interfaces, protocols,
implementations, function order, every signature field, contracts and spans
must be exact. The selected function's complete AST must also be equal. Generic
top-level declarations participate in that complete ordered signature proof,
including exact scoped type parameters; class methods resolve afresh. The reuse
capability is bound to the exact current source receiver. Only monomorphic
function HIR is cloned: generic templates always resolve from current source,
and calls in both reused and fresh functions bind their derived instance IDs
and ordered type arguments to those current templates. The complete transitive
instance closure is freshly materialized in the original structural order;
previous template and instance payloads confer no authority. Any
environment, contract, signature or span drift conservatively resolves all
functions in that module. Persistent snapshots do not restore invocation-local
function work costs, so the first changed-module build after restoration also
resolves afresh. An exact whole-module persistent hit remains eligible.

The freshly validated checked owner is charged the resolver's retained bounded
output consumption so hits preserve cold-equivalent construction accounting.
Loan-proof capacity census and its original residual debit remain required;
sharing does not authorize removing either charge. Contiguous and noncontiguous retained selections
keep the original checked owner and its vector/string capacities. Sparse local
selections retain separately prepaid authored ordinals, not cloned HIR bodies.
Read-only graph, signature and operation receivers traverse those exact references;
indices supply access, never validation authority. Final owning link/projection
products still materialize at their original boundary. The separately owned
imported-instance attachment overlay remains unchanged. The cached two-bundle
forecast and original cold allowance remain conservative and unchanged.
Immutable expression-identity backing remains shared. The allocation-free census
does not repeat the full physical-storage census's scratch allocation inside the
graph budget. Full census and required scratch reservations remain unchanged for
retained proof storage and authenticated snapshot reconstruction. The existing
nonempty-loan gate retains exact cold/warm graph/image and accounting comparisons.
In-process expression-identity clones now share immutable backing. Authenticated
snapshot restoration independently reconstructs nonempty loan plans and requires
complete value equality before restoring that sharing; wire bytes, full validation
and zero source-resolution requirements remain unchanged. Its separately bounded
physical-storage replay is specified in [Semantic Cache Store v1](SEMANTIC-CACHE-STORE-V1.md).
The shared-backing repair's current-source gate is pending; the preceding hosted
evidence does not establish that gate.
This charge is construction accounting, not a measurement
of allocator use or peak process memory. Cold and warm routes still rebuild
the Project's linked representation and graph; neither promises constant-time
refresh or unchanged target artifacts after a source edit.

Staged entries become reusable only after the whole build and bounded work
report succeed. A failure after some modules have resolved does not commit
those partial entries. Restoring the original sources after a rejected proposal
can reuse the previous successful cache.

## Work report and limits

Semantic mode emits `semaprax.project-semantic-cache-work.v1`, with compatibility
`semaprax.project-checked-module-hir.v9`. The v9 binding owns the combined
Row Loan v1 and [Borrowed Byte Slice For Traversal v1](BORROWED-BYTE-SLICE-FOR-TRAVERSAL-V1.md)
wire: `SourceForKind` tag `3` (`BorrowBytes`), single-arm byte receivers and
`TraversalFreeze` loans. The v8 binding introduced Row Loan v1; v7 bound scoped
Vec field reads to canonical selectors. Older payloads are never decoded under
v9; decoded HIR still undergoes complete independent validation. The v6 binding
invalidates stored full-capacity metadata before it can be interpreted as mutable
clone capacity. Earlier compatibility bumps included `.v3` when a function
gained the endpoint typestate `follows` clause, issue #297 follow-on, and
before that from `.v1` when the sealed source carrier gained declared session
protocols, issue #297, so a persisted payload from an older layout is never
decoded as the new one. It preserves the frontend report's
field structure but uses this separate schema so the old frontend contract's
`checked_HIR_reused: 0` remains true.

`modules_resolved` counts modules that enter semantic resolution.
`checked_HIR_reused` counts actual complete checked-module hits.
`monomorphic_functions_resolved` and `monomorphic_function_HIR_reused` split
fresh and exact function work; `selective_function_HIR_resolution` identifies
the semantic mode. `full_source_verification`, `full_HIR_validation`,
`full_cross_file_checks`, and `full_link_and_profile_admission` remain true.
Parser, canonicalizer, and AST reuse counters retain their existing meanings.
Counters describe work, not benchmark results, behavioral equivalence, or
independently executed tests.

### What a hit is worth

`modules_parsed: 0` records that parsing was skipped, not that the work was.
Two properties bound what any hit can save, and neither is visible in a
counter.

An AST hit does not avoid work proportional to module size. `lookup` returns
`entry.program.as_ref().clone()`, a deep clone of the whole `Program`, so a hit
trades parsing a module for cloning its AST. The two costs are of the same
order, and the clone grows with the AST while the parse grows with the source,
so a hit is roughly a wash on small modules and can be *more* expensive than a
cold parse on large ones. The stored `Arc<Program>` is a retention device, not
a sharing one; nothing downstream holds it.

The four phases named above run in full on every rebuild. Whatever a hit
saves, it cannot exceed the share of a cold build spent in parsing, and the
measured share is small. Issue #85 records the benchmark shape that follows
from this — `rebuild-unchanged` at parity with `cold` on a 1x fixture and
slower on a 4x one — and owns the decision of whether to make hits O(1) by
threading the `Arc` through the workspace consumers, which today all take
`&[Program]`.

The cache retains one successful context under the existing source count,
source byte, and compiler-construction bounds. Checked retention additionally
uses `MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND`, tied to the Workspace Semantic
Graph's 128 MiB `MAX_BUILDER_BYTES`, and enforced against the whole synthetic
AST/HIR construction estimate before checked clones are made. Sharing the
ceiling removes the smaller independent 16 MiB cache limit; it does not combine
the phases, equate their estimates, or turn either estimate into allocator/RSS
accounting.
The synthetic program is also preflighted against the exact 8,192-function
inventory bound before function-cost inventory is created.
This is separate from the existing 16 MiB source and AST-construction limits;
it is not an allocator byte limit. Reports retain the 65,536-byte
bound and deterministic sorted-key compact JSON with one final LF. Source ASTs,
synthetic resolver inputs, checked module clones, newly linked HIR, and staged
generations may coexist. No aggregate process-heap or RSS bound is claimed.
Ordinary frontend capacity/grammar and compiler semantic diagnostics propagate;
cache policy cannot make an inadmissible program valid.

## Authored evidence

`tests/project/semantic_cache.rs` covers unchanged warm counters and exact cold
source/graph/image identity; AST-only compatibility; leaf, provider, and private
body invalidation; exact sibling-function reuse after a body-only edit;
whole-function-lane invalidation after contract drift; changed import
signatures/bindings; matching cold rejection diagnostics; failed-build rollback;
manifest reset; and owned-source image refresh with failed-proposal rollback.

The focused Project semantic-cache corpus, including nonempty combined
owned-variant/loan Graph v32 cloning, has hosted-green release evidence.
Historical local execution remains a separate witness. Broader incremental
verification and measured time/memory improvements remain required before
stronger completion or speed claims. The separate
[Persistent Semantic Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md) implements its
own authenticated cross-process checked-HIR reuse rather than widening this
in-process profile.

### Immutable checked HIR retention (OPT777 contiguous tranche)

Cold semantic insertion moves the resolved program into the existing checked
module owner. A warm exact-synthetic hit borrows that same immutable owner after
current source-association replay. Full current HIR validation, source/environment
checks, cold-equivalent resolver debit and loan-proof accounting remain mandatory.
The owner is storage, never a validation lease.

Contiguous local function, type, template and agent selections retain actual
borrowed slices of that owner through core retention. Noncontiguous selections
use an owned compatibility fallback; permits, interfaces and mutable imported
instance overlays also remain owned. The full synthetic AST, declaration index
and imported stubs stay alive with the checked owner. The existing full cached
two-copy forecast remains conservative and unchanged. No uncached discount is
granted by a filtered view. Final public owning projections and linker inputs
materialize selected bodies at their existing owning boundary. Temporary generic
closure discovery now borrows functions, templates and instances.

Snapshot encoding walks the authenticated held state without building a second
owned snapshot tree; the wire and full fresh decode/replay contract are unchanged.
This tranche does not establish optimal noncontiguous retention, reduced forecasts,
whole-compiler allocation fit, or completion of OPT777. New owner and wire controls
are source-authored and require focused execution with the existing persisted
application and semantic-cache controls.

The additional lifecycle owners compare an actual three-source cold and warm
Project build with the independent AST-only owned receiver, including fresh
current cross-file checks, final owning publication, source-signature drift and
restoration. Requested Rust Layout allocation/count/high-water/Drop observations
separate cold acquisition, warm replay with a pre-held cache, and final owned
materialization. Raw inputs, the saved oracle, common process-static initialization
and fixed observer metadata are outside those scopes; in-scope equality and wire
checks are included. Final publication remains a real deep clone. These authored
controls require execution; they establish no quota reduction, process-memory
bound, application fit or complete OPT777 acceptance by themselves.


## Sparse checked-owner retention (OPT777)

The private `Retained` carrier has explicit range and ordinal forms. Sparse
selection reserves the local index Vec header plus `count * size_of::<usize>()`
before allocation; any allocator excess is debited before publication. The
actual module-carrier layout pays its enlarged inline retained fields. Operation
sidecar views similarly prepay the actual increase over their previous slice
carrier before constructing the directory. These charges use the current
builder ledger; refusal stays sticky.

The new sparse owner gate is
`workspace_graph::checked_retention::tests::ordinals::sparse_checked_ordinals_preserve_cold_warm_body_identity_old_wire_and_paid_drop`.
It uses real cold insertion and warm checked lookup, compares exact body
addresses and saved old owning wire, retains both owner drop orders, and
observes only additional ordinal allocation and Drop under literal exact and
one-short requests. The prior noncontiguous copying controls remain saved old
oracles. Existing complete Project/cache wire, source/environment drift and
forged replay controls remain required. Authored source changes and static
review do not establish execution, full pipeline allocation or application fit.

### Full-input retention counters with shared selections

Normal checked and owned retention preserve the same established fixed-carrier
and loan-sidecar debit for every input type, function, template and agent,
including functions filtered out of local output. This conservative counter is
not a claim that checked retention physically clones those carriers. The shared
receiver pays it on the exact immutable input before selecting; its fixed portion
covers the smaller sparse ordinal header/backing in that same receiving call.
Only checked excess is separately reserved. Loan-sidecar charges never supply
ordinal credit, and no allowance is transferred, refunded or reset. Standalone
ordinal constructors continue to reserve their exact actual layouts. Final
uncached output-only retention remains its separate existing payer. Forecasts
and the mutable instance overlay remain unchanged.

The two `checked_retention::tests::input_accounting` owners retain the literal
six-function counter witness, actual ordinal allocation/Drop scope and unchanged
all-input owned sidecar oracle with exact/one-short/sticky controls. These are
source controls pending qualification; they do not establish total diagnostic
funding or an application fit/performance result.

### Package scalar publication discovery (source-only followup)

`WorkspaceGraphBuild::linked_package_scalar_exports` borrows current retained
function bodies while building its available-function map. Discovery still checks
every body against exact declaration path/module/kind facts and refuses duplicates
before checking exports. Reachability and byte-sorted publication are unchanged.
Only the reachable functions are cloned at the final owning linker boundary;
previously discovery also deep-cloned every available function, including unused
bodies, before cloning the reachable functions a second time.

The two `workspace_graph::tests::package_link_borrowed` owners compare the saved
owned implementation's complete wire/errors, declaration and duplicate forgeries,
and actual discovery through owning publication and Drop for 1/8/32 unused bodies.
The physical observer excludes workspace preparation and does not establish cache
acquisition funding or application fit. No forecast, cap or persisted wire changes
follow from this subset. Qualification remains pending.

### First checked cold source view

An import-free synthetic input without type invariants can lend its exact
authored declarations to the first checked resolution. The existing complete
source verifier, current private-name subject, and checked function ceiling
still run. Resolution retains its per-function costs for later body reuse.
An existing checked candidate, an imported overlay, or a type invariant uses
the original owned input path and its exact association checks.

The cache key remains an owning `Program`. Its construction occurs after the
resolver-debit snapshot and before checked publication: retained authored
declarations are copied once, generated main storage moves, and discarded
module-use/implementation sidecars are not copied. This delays the owning
boundary; it does not establish a shared AST cache owner. Cached forecasts and
cold-equivalent accounting remain conservative and unchanged. The narrow
allocation witness measures only removed sidecar clone payloads through Drop;
it does not establish whole Project funding or application fit.

The current v10 compatibility adds CopyDiscard codec tag 5; its exact no-result
semantics and source revalidation are owned by
[Finite owned-record discard loops v1](FINITE-OWNED-RECORD-DISCARD-LOOPS-V1.md).

### Exact generic environment selective reuse: source-only followup

The blanket generic-environment exclusion is removed from the private
function-granular guard. All original source/provider authentication,
non-body environment and complete ordered signature equality remain required,
including every yields/follows clause field and span. Hostile direct-AST clause
mutations retain full fresh/nonreuse program or ordered diagnostic parity.
The unchanged full ordinary nonreuse resolver remains the independent oracle.
`hir::generic_environment_reuse_tests` authors call-bearing helper reuse,
current generic template body and nested callee closure drift, type-argument
drift, stale previous template/instance payload exclusion, malformed reused call
identities/arguments, detached-current capability rejection, complete current
HIR equality, and actual allocation/Drop plus exact/one-short reservations.
The existing canonical Vec/Box selected-import receiver retains its genuine
`reused > 0` assertion and full fresh-owned comparison without modification.

These controls await coordinated execution. This source-only change makes no
cached generic-instance reuse, fit, runtime efficiency or agent-token claim;
current instance construction, cleanup/loan replay, builder limits and ordinary
unauthenticated wrapper refusals remain required.

### Current byte-helper and Agent side effects in function reuse

A resolved caller with any call to a possible source byte-helper target resolves
fresh. The shared `SourceIndex::may_require_body_proof` signature prerequisite
is exactly the one used before source body proofs: a Bytes result and exactly
one own Bytes parameter. The existing complete borrowed call visitor checks
requires, body and ensures, including control flow and nested children. This
retains current transitive nonallocation proofs, source diagnostic selection,
phase-owned proof memo/failure and builder reservations. A transitive changed
helper cannot hide: the direct caller crosses this prerequisite and runs the
current full source proof. A scalar wrapper's own internal proof-dependent
calls likewise force its fresh resolution; callers whose admission depends only
on its unchanged scalar signature remain reusable. This restriction is local to
body-dependent byte-helper callers, and leaves genuine scalar generic reuse.

The ordinary source verifier still runs before the current reuse capability;
Agent execution metadata and resolved declarations are rebuilt unconditionally.
New generic-environment controls compare whole ordinary/nonreuse programs or
complete ordered errors for valid and invalid byte-helper body drift and direct
Agent origin/target/kind/model-wait metadata drift. Corrupted prior Agent payloads
provide no current authority. Actual paid allocation/Drop and exact/one-short
byte-caller controls are authored. Original generic and Vec/Box owners remain;
all these additions await coordinated execution and make no efficiency claim.

The source-only [Scoped Vec Field Source Forms v1](SCOPED-VEC-FIELD-SOURCE-FORMS-V1.md)
successor selects `semaprax.project-checked-module-hir.v11`: it retains the
Explicit/Traversal form of each scoped field read and independently replays it
against canonical source. v10 payloads are not decoded under v11; frontend AST
wire is unchanged. Actual owning cache qualification remains pending.
