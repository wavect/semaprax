# Workspace Semantic Graph v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Workspace Semantic Graph v1 is a bounded, canonical, read-only view of one
authenticated Semantic Workspace v1 generation. It checks the complete managed
set before selecting the entry closure: the entry module and its transitive
providers, reached only through explicit direct `use function` and `use type`
edges. Reverse consumers, disconnected modules, implicit imports, reexports,
stubs, and synthetic main declarations are excluded.

## Public API and command

```rust
pub fn snapshot(
    root: &std::path::Path,
    entry_module: &str,
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>>
```

The public immutable types are exactly:

```text
WorkspaceSemanticGraph
WorkspaceSemanticGraphEntry
WorkspaceSemanticGraphModule
WorkspaceSemanticGraphDeclaration
WorkspaceSemanticGraphEdge
WorkspaceSemanticGraphLimits
WorkspaceSemanticGraphBudget
```

No fields or constructors are public. The graph exposes borrowed string
getters for schema, manifest schema, revision, digest, entry, modules,
declarations, edges, nonclaims, and canonical JSON. `limits()` and `budget()`
return their public `Clone + Copy + Debug + Eq + PartialEq` value types.
Modules expose path, module, source schema/revision/digest, dependency depth,
and permits. Declarations expose ID, wire kind, wire identity origin, and
nullable owner/path/module. Edges expose their ten wire fields. `to_json()`
borrows the exact compact JSON body.

```text
semaprax workspace-graph <root> <entry-module>
```

The API JSON has no terminal LF. CLI success writes the API JSON followed by
exactly one LF. Wrong arity exits 2 with
`workspace-graph requires exactly <root> <entry-module>` and no stdout.

## Entry and authenticated closure

`entry_module` is exact ASCII
`[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*`, is not normalized, is at
most 16,777,216 bytes, and must equal one authenticated module. It need not
define `main`. Malformed entry text is `SPX-G170`; an absent canonical module
is `SPX-G172`. The length check precedes grammar and never echoes an oversized
entry.

All 2–32 managed modules are parsed, resolved, cross-file checked, and bounded
before projection. Modules sort by UTF-8 path bytes; permits retain source
order. Declarations sort by stable ID and contain closure-owned source
declarations plus the compiler prelude exactly once. Compiler-owned prelude
declarations have null path/module. Synthetic declarations and stubs are never
emitted. Edges retain only closure-owned emitted facts and use the exact
Workspace edge tuple order.

Independent per-module Vec selection follows exact authored imported function
signatures and imported record/variant field paths, including nested type aliases.
Unrelated provider declarations or bodies, missing persistent IDs, wrong modules
and retained HIR metadata do not select Vec. The source-only cursor uses 514 fixed
pending/visited slots, 4,096 visited fields and 1,000,000 work units; its full
backing is charged before use. Exhaustion supplies no declaration authority.
This selection reconciles the compiler-owned inventory and admits no new type.

## Canonical wire

The schema is `semaprax.workspace-semantic-graph.v1`; the manifest schema is
`semaprax.workspace-semantic-manifest.v1`. JSON is compact canonical UTF-8 with
no BOM, whitespace, CRLF, or terminal LF. Top-level key order is:

```text
schema,workspace_manifest_schema,workspace_revision,graph_digest,entry,
modules,declarations,edges,limits,budget,nonclaims
```

Nested key order is:

```text
entry:       module,path
module:      path,module,source_graph_schema,source_revision,source_digest,
             dependency_depth,permits
declaration: id,kind,identity_origin,owner,path,module
edge:        caller_path,caller,target_path,target,kind,site,expression,
             ast_path,alias,ordinal
```

Declaration kinds are `resource`, `resource_drop`, `record`, `field`,
`variant`, `variant_case`, `case_field`, `interface`, `import`, and `function`.
Identity origins are `explicit`, `automatic`, and `compiler_owned`. Edge kinds
are `call`, `capability_authority`, `effect_requirement`, `function_import`,
`type_import`, and `type_reference`. Sites are `module`, `type`, `requires`,
`body`, and `ensures`. `alias` is an empty string when absent, never null;
nullable declaration owner/path/module values are explicit JSON null.

## Digest

`graph_digest` is lowercase `sha256:` plus 64 hex digits. Its domain is the
exact bytes `semaprax.workspace-semantic-graph.artifact-digest.v1\0`. The hash
input is:

```text
domain || u64_le(payload_byte_length) || payload
```

`payload` is the exact compact top object without `graph_digest`, but with the
final fixed-point `used_output_bytes`. A fixed-width digest placeholder sets
the final size. The implementation then hashes once, inserts the digest,
renders again, and checks the exact length and typed binding.

## Limits and budget

The `limits` object has this exact order and values:

| Key | Value |
| --- | ---: |
| `max_managed_files` | 32 |
| `max_reachable_modules` | 32 |
| `max_entry_module_bytes` | 16,777,216 |
| `max_total_source_bytes` | 16,777,216 |
| `max_declarations` | 4,096 |
| `max_callables` | 1,024 |
| `max_call_sites` | 65,536 |
| `max_uses` | 4,096 |
| `max_resolved_cross_file_edges` | 65,536 |
| `max_dependency_depth` | 16 |
| `max_builder_bytes` | 67,108,864 |
| `max_manifest_bytes` | 1,048,576 |
| `max_output_bytes` | 16,777,216 |
| `max_retained_generations` | 32 |
| `max_staging_attempts` | 32 |
| `max_unexpected_inventory_entries` | 0 |

Corrective byte compatibility: `max_builder_bytes` is 67,108,864 because that
is the `SPX-G171` enforcement limit. Earlier v1 output that encoded
16,777,216 was defective and is not canonical. Re-rendering a graph with the
corrected limit changes its digest, but does not change the schema or raise a
ceiling.

The `budget` object uses the corresponding `used_` names in the same order.
Full authenticated-work counters are `used_managed_files`,
`used_total_source_bytes`, `used_declarations`, `used_callables`,
`used_call_sites`, `used_uses`, `used_resolved_cross_file_edges`,
`used_dependency_depth`, `used_builder_bytes`, and `used_manifest_bytes`.
`used_entry_module_bytes` is the submitted entry scalar;
`used_reachable_modules` is the emitted provider closure;
`used_output_bytes` is the complete no-LF artifact; and
`used_retained_generations`, `used_staging_attempts`, and
`used_unexpected_inventory_entries` are authenticated storage state.
`used_builder_bytes` is the maximum sequential builder-phase debit:
`max(canonical_phase_bytes, core_phase_consumed_bytes)`.

The core phase charges a resolver pre-bound before it links anything, so a
program that cannot fit is refused with `SPX-G171` before mutation. The
pre-bound is computed from the authored program alone and is deliberately an
upper estimate, composed of three terms: structural source bytes (the AST
node footprints) times 24, which covers the sixteen footprints the
compile-time bundle assertions establish plus eight for map and tree
bookkeeping; string-content bytes times 64, the at-most 48 retained copies of
a source string plus bookkeeping; and, for every declaration identity slot a
resolved expression of that shape can hold, the longest identity in scope
times 16. A scalar expression holds three slots, a call or construction four,
a variant construction five, and `Try` eight. Before the split, every source
byte and every expression were charged at the `Try` and string rates, and a
4.9 KiB module of twenty scalar functions exhausted the budget.

The pre-bound can reject a build before linking, but it is not the only check.
Both the estimate and the structures actually retained by the core build are
reserved against the same `builder_bytes` budget. Either overflow produces
`SPX-G171`. A looser estimate rejects earlier and a tighter one later; neither
removes the retained-memory bound.

Independent edge replay compares the complete current source/HIR occurrence,
type-reference, effect and capability projections directly. It does not allocate
a second text rendering of those edges and discard it. Real replay scratch keeps
the sticky `workspace edge proof` reservation stage; removing the unused text
carrier neither refunds preceding reservations nor changes edge/output limits.
Owning executable qualification of this reduction is pending.

Authored expression-site reconstruction reuses one UTF-8 path buffer per root.
Its inline carrier and each complete replacement backing are charged before
allocation, including overlap with the previous backing; child return truncates
the buffer without refunding the cumulative debit. Callback path bytes, order,
lowered traversal suffixes, and separately charged owned edge copies stay exact.
This changes scratch construction, not parsed depth/call quotas or authority.

A final live resolved-core budget refusal may include additive help with the
first sticky reservation's requested bytes, remaining bytes, and reserved
floor, captured from that exact attempt before its budget is restored. Optional
floor refusals remain non-sticky and are not recorded. Failed earlier retries
and nested child phases cannot supply the final attempt's evidence. A scoped,
compiler-owned internal stage may identify the enclosing prebound, synthetic
module construction, cached HIR validation, fresh HIR resolution, or resolved
module retention operation. An unlabelled reservation reports stage `unknown`;
the stage never comes from source and does not identify the exact inner call.
Once required work has selected a refusal, later required requests, including
zero-sized authentication, remain refused without spending the balance.
Positive optional writes may still consume already reserved error-trailer
space; neither those writes nor clearing a trailer floor clears the refusal or
restores required-work admission.
Resolved-core construction provides an enclosing fallback label; compact
validation inventory, uncached synthetic AST peak reservations, authored module
signature validation, frontend lookup/resolution/retention and imported Vec
instance inventory have narrower operation labels. These labels neither alter
the cumulative ledger nor attribute a prior unlabelled refusal retrospectively.
Canonical shared-loan storage preparation installs its enclosing production
stage before capturing the existing CFG reservation owner. This fixed label
covers the preparation scope, including owner acquisition, plan rebuilding,
append and sealing; it does not identify a particular inner request. The same
budget, numeric first-sticky receipt, quota/work, restoration and paid help
omission rules remain authoritative. Test-only operation traces are not
production attribution, and the exact inner operation remains unknown.
These quantities describe the cumulative reservation ledger, not a retained
peak forecast, allocator measurement, or process RSS. The SPX-G171 code,
message, source immutability, and ceiling are unchanged. Missing evidence is
reported as unknown, never synthesized from the static forecast.

The legacy estimate uses the longest authored identity in the whole workspace.
If that estimate would exceed the builder limit, an additive fallback repeats
the same arithmetic with each module's own transitive dependency closure and
the frozen prelude as its identity scope. Reverse-dependent test declarations
cannot occur in that module's synthetic HIR and therefore cannot enlarge its
identity bound. Every previously accepted estimate is retained exactly; the
fallback runs only after a builder-limit refusal. Structural and string factors,
the 64 MiB limit, and actual allocation charges remain unchanged.

If both estimates refuse, a third attempt accounts inline AST storage once:
embedded String headers, and the audited inline expression and pattern fields,
are already included in their enclosing values. Every separately visited
expression keeps its complete raw footprint. Primitive literal roots omit
only the fixed owned storage-slot, flag, place, and plan-slot structures that
their Value ownership and literal lowering never create; their state,
transition, and control-flow metadata and all bookkeeping remain charged.
Compile-time bundle assertions preserve the remaining fixed HIR bound.
String-literal and assignment-field payloads and nested declared/function types are explicitly
charged. Primitive literals and checked scalar unary/binary roots retain their
expression identity but need neither a nominal result-type identity nor an
owned cleanup temporary; their operands remain independently charged. This
also applies to scalar-valued blocks and uniform scalar control-flow results.
Ordinary nongeneric calls use their exact authored local/imported return type,
or the frozen byte-operation signature, to establish the same two absent
slots; expression and callee identities remain charged. Unknown, generic,
nominal, and potentially shadowed calls retain their conservative charges.
Unshadowed primitive value parameters and uniquely declared scalar locals omit
one unused nominal-type slot per read, retaining both the expression identity
and the binding identity of the resulting Place. Local facts begin only after
their initializer and end with their declaring block; duplicate, pattern, loop,
and closure bindings remain conservative. Assignments cannot establish a fact,
and native import precedence remains unchanged. All other identity slots and every expansion factor stay unchanged. Standalone
strings, synthetic runtime structures, generic-instance estimates, and transient
imported bodies retain their prior charges. Successful receipts from either
earlier attempt remain byte-for-byte unchanged.

If the raw-AST estimate also refuses, the final fallback sums retained
synthetic AST/HIR costs across modules and retains the historical charge for
one maximum transient full-function import clone across the sequential module
loop. This remains a conservative estimate after signature-only stub creation;
completed ordinary imports have zero-capacity contract vectors. Earlier
accepted estimates and receipts remain unchanged.

If that sequential-peak estimate also refuses, one further fallback omits
that historical transient-body peak. Ordinary imported stubs now clone only
their signature: their provider bodies and contracts are never allocated in
the consumer. Checked compiler wrappers still retain their full implementations
and their existing runtime charges. The new fallback keeps all retained
structure, string, identity, and runtime costs, and the same 64 MiB limit;
earlier successful receipts remain unchanged.

The final output-carrier profile reserves the complete compile-time asserted
expression HIR/cleanup bundle directly, retaining all eight bookkeeping
footprints. Its audited single-root resolver branches retain one identity
heap carrier; default bodies, generic expansions and traversal synthesis keep
their independent ceilings. Other profiles retain their historical receipts.
Independent call replay borrows the final core's immutable edges, reconstructs
all ten fields from authenticated AST occurrences, rejects missing, extra and
duplicate occurrences, then checks retained HIR and capability projections.
Every allocation still consumes the live ledger; the 64 MiB cap is unchanged.

[Prelude forecast accounting v1](WORKSPACE-GRAPH-PRELUDE-FORECAST-V1.md)
corrects the missing unconditional Option/Result retained-declaration term in
all private forecast modes. For that term only it supersedes the historical
fitting-receipt preservation clauses above and below. The complete original
forecaster and receipts remain historical controls; graph schemas, quotas,
expression discounts and live allocation charges are unchanged. Qualification
of the source correction is pending.

A present frontend has a separate mode-seven exact fallback after every older
fitting receipt has been considered. It sums the synthetic AST and runtime cost
of every module; it never selects the mode-five sequential peak or mode-six
filtered output lifetime. A parsed-only frontend retains one complete HIR bundle.
A semantic frontend stages a checked-cache HIR clone while core construction
retains the original, so this forecast charges two complete expression/cleanup
bundles. The original eight bookkeeping footprints remain charged once;
only the fixed HIR expression/cleanup bundle is doubled. Shared ExpressionId
Arc backing is charged by actual constructor occurrence, not twice merely for
a clone. Other string-copy, declaration, generic/default/import and identity
ceilings remain charged. The same live builder ledger and checked-module cache
limit still apply; an exact forecast is not evidence of physical allocation or
of an application fitting the unchanged limits.

Only an uncached core may use this final sequential-AST peak. The builder
receipt begins after the authored source programs have been parsed and retained,
and covers the core's synthetic AST/HIR construction; it does not re-charge
those pre-existing authored ASTs. Each synthetic AST, including its generated
default-expression bodies, is live only while its own HIR resolves. On the
final output-carrier fallback, that HIR yields exact declaration facts plus
function/type signatures (including generic parameters, effects, and checked
Vec/Box wrapper identity), then drops before the next module resolves; only
the filtered `WorkspaceResolvedModule` and those compact facts remain. The
mode-six receipt charges the accumulated filtered output and compact evidence
beside one current full synthetic AST/HIR, rather than summing full HIR for
every completed module. The cross-module checks compare that extracted evidence exactly. A semantic
frontend stages both synthetic AST and HIR in its checked-module cache, so it
stops at the earlier summed receipts and never selects this final fallback.

The final output-carrier profile charges each compact signature heap clone and
declaration map entry before allocation. Synthetic AST clones receive a
monotonic maximum-peak reservation; the remaining HIR forecast is still
reserved before resolution. Forecasting excludes imports for the retained
local output without cloning an authored Program. A fixed-size index orders
modules by descending full-minus-retained HIR overhead, with a path tie-break;
the forecast and resolver use the same order. Completed outputs are restored
to canonical path order before validation and publication.

Exact authored declaration collections now lend a sealed compiler-only
`ProgramSource`; local presentations own only an optional scalar `main`.
A concrete presentation identity distinguishes overlays of the same source.
Imported presentations borrow the authored slices and own explicit aliases,
independently derived private layouts, rewritten signatures/checking defaults,
and a missing scalar main. Ordinary Vec/Box wrapper classifiers retain the same
complete imported body, including generic materialization. Class methods keep
the same exact copied imported declaration. Every ordinary import, ownership,
byte-helper, source, prelude and declaration-index check still runs.

Generated directories and the complete borrowed type-name index are paid before
allocation. Recursive generated roots retain the existing iterative `Program`
teardown owner on success, construction errors and refusal. Before roots are
copied, the producer pays each teardown worklist's minimum backing and a
fourfold undiscounted inline AST census, covering cumulative geometric worklist
requests; this is real new teardown allowance, with no cap increase or forecast
refund. Plain scalar/default overlays keep their allocation-free teardown.
The standalone cache-key receiver still pays both replacement directories
while recursive roots remain under their owner. The actual resolved-core import
producer instead acquires each real combined directory at its original
construction frontier: types before copying types, functions only after the
complete type-name verdict. It pays the affine owner and requested backing
before `try_reserve_exact`, settles any actual excess capacity before refusal,
and keeps only generated declarations in the borrowed view's logical length.
The structurally affine owner contains the actual Vec and immutable authored
slice plus the current payer. It lends element slices and bounded push only,
never a mutable Vec or detachable pointer ticket. Allocator address reuse cannot
transplant backing authority; this is ownership, not admission authority.
Semantic publication validates both owners and sticky state before cloning
payloads, appends authored roots into the paid spare backing, then rotates to
canonical authored-before-generated order and moves the same Vec once.
Original raw payload forecasts, counters and caps remain unchanged. Ordinary
and warm import construction retain additional spare capacity/control storage;
cold retention avoids a second overlapping directory allocation. This is an
explicit storage tradeoff, not an established efficiency or fit improvement.
AST-only frontend passes receive resolved HIR without constructing a discarded
cache key; original counters and the explicit legacy owning receiver remain.
No shared authored AST cache or forecast-based allocation credit is created.

Source-authored controls retain all original standalone and raw-graph gates.
`key_directory::tests` covers real zero/exact/short/sticky allocation/Drop,
source/detached-owner refusal, bounded no-growth mutation and terminal transfer; `borrowed_imports::tests::full::receiving`
covers complete old/current projections and full constructor/retention/Drop
costs separately, without subtracting the ordinary/warm spare storage. These
controls require executable qualification; semantic diagnostic materialization
and whole application funding remain separate residuals.

One actual resolved-core invocation may retain byte-helper proof Vec/cursor
backing across its sequential forecast and constructor phases. Reuse is bound
to the exact immutable program slice, authored map and captured live budget Rc,
with sticky required-zero checks before clearing or lending storage. Each new
logical phase clears all memo verdicts and cursor frames and repeats the full
original source/provider/body proof and original per-phase work allowance.
Failed or incomplete scopes poison the owner; ordinary Drop returns storage
without restoring allowance or replacing the first refusal. Additional owner
and simultaneous phase controls are prepaid, and growth still pays the full
original replacement acquisition. Pre-core/standalone proofs and every new
resolved-core attempt stay independent. Original forecasts, HIR bounds and
caps remain unchanged. The complete old receivers and focused physical/Drop
controls require executable qualification; this is no application-fit claim.

Record invariants run ordinary source verification first, then the same clause
inventory and desugaring passes. Writable AST is copied at those genuine
transformation boundaries; lookup no longer needs an owning synthesis copy.
The original owning transformation functions and full synthetic producer remain
independent saved oracles. Authored pointers, alias/default bytes, complete
canonical/graph/HIR diagnostics, real directory replacement layouts, and
constructor/resolver/cache-publication/Drop lifetimes have source-authored
owners, including generic wrappers, class methods and invariant negatives.
These owners are unexecuted at authoring time. No full #776 release qualification,
application-fit, reduced HIR forecast, or performance result is claimed here.
All original application inputs, fuel, caps and profile admission remain fixed.

Within one resolved-core validation attempt, private type authority retains one
borrowed owner bound to the exact immutable Program slice and authored map.
Its lazy complete top-level proof is charged to the first caller's existing
work allowance; callers retain their original independent work/depth ceilings
for import, root, contains and current-HIR signature checks. The owner reuses
one precharged DFS frame allocation and, where class methods require the old
lookup universe, one precharged legacy function/type map. Roots remain fresh
per caller and charged to the same cumulative builder account. Every caller
and map binding is checked again, failed acquisition/traversal is sticky, and
scratch frames are cleared on failure. No source or HIR verdict, allocation
credit or mutable cache survives the attempt. Source-only controls require
saved-old roots/verdicts, exact and one-short repeated backing through Drop;
application recovery and performance remain subject to execution.

At the production default builder limit, an unnested core phase that exceeds
its budget after accepting an earlier estimate may retry with the raw-AST
estimate, then with the final transient-peak estimate, only when each is
strictly tighter. The final output-carrier profile is a different retention
strategy and may be selected without that numeric comparison; its complete
charges must still fit the unchanged cap. The failed partial core is dropped and private frontend
attempt state is rolled back before each retry. The recorded sequential-phase
debit is the maximum of all attempts; no enclosing budget is reset or refunded.
Explicit smaller-limit invocations and nested budgets retain their original
core-refusal behavior, and a successful earlier attempt retains its receipt.

The resolver allocates the ordinary-function carrier at the exact count of
nongeneric source functions and class methods. In final uncached retention, a
fully selected carrier with capacity equal to that count moves directly into
the retained module. Resolution already covers that physical allocation; no
second output allocation or output-carrier charge is invented. Nonempty loan
sidecars still receive their full charge. Partial selections and carriers with
spare capacity keep the existing compact-copy path and its complete reservation,
and earlier attempts retain their established receipts. No source prebound,
capacity factor or retained-closure forecast is reduced.

In that final uncached retention attempt, the sharing census starts with
distinct loan-plan identity backing keys. An allocation-free comparison census
counts the exact distinct candidates before reserving either carrier. It sorts
those candidates, then walks
and validates every retained HIR identity and marks only exact backing-key
matches. The final uncached census reuses these same carriers to visit every
proof backing: state 1 records HIR sharing, state 0 identifies independent
storage and state 2 records its complete physical debit once. Every proof key
must occur in the sorted candidate inventory, and every backing is validated.
The independent debit includes the full String capacity and Arc carrier; equal
text never establishes sharing. Only state 1 keys survive the final compaction.
This eliminates the second unmatched-reference vector in this final attempt.
Earlier attempts and standalone capacity APIs retain their original reference
inventory and exact reservations. Unreferenced HIR identities consume no entry.

One key vector and one byte-per-candidate match vector are reused across
functions and modules. Before consuming a module, both carriers reserve the
largest selected ordinary-function candidate census once, avoiding intermediate
replacement allocations. Before consuming selected generic instances, this
attempt also reserves the largest selected instance census once; authored
templates and exactly authenticated imported Vec/Box wrappers use the same
selector for census and retention. Unselected instances grant no census
authority. Earlier attempt receipts remain unchanged. Every replacement reserves both entire requested
capacities before allocation and charges any allocator capacity excess; earlier
allocations remain charged in the cumulative ledger. Both vectors are cleared
before each function, so a previous function's keys cannot exclude independent
or foreign proof storage. Missing HIR or proof backing refuses before scratch
allocation. The successor HIR walk retains the existing depth bound and uses
the existing one-million expression-work bound, rather than the 4096 loan
endpoint bound: HIR-only nodes are not loan points. The optional distinct-key
census uses a fixed 4096-bit stack bitmap to skip a prior-key scan only when
that bucket has never been set. Every bitmap hit, including collisions, uses
exact prior-key comparison; the bitmap grants no sharing or exclusion authority.
It allocates no heap storage and cannot change the distinct-key count. The
census still falls back after 64 million actual key comparisons. A refused
uncovered-proof reference inventory names the authenticated internal stage
`loan identity inventory`; this is the actual enclosing reservation operation,
not an inference from requested bytes or a change to the sticky refusal. Full-census fallbacks, proof carriers, unmatched
identities and earlier-attempt per-HIR-key receipts are unchanged. Reuse and
candidate intersection remove actual temporary storage; they do not discount
any allocation that still occurs or raise the builder limit. The full pattern
bundle and exact-limit/one-short hostile inventory gates remain required.

Cross-file edge reconstruction does not traverse an import-free module to
materialize paths whose output set is necessarily empty. Capability edges
are still emitted before that fast path, and the complete expected-versus-HIR
edge comparisons still reject missing, extra, or forged evidence. Type-site
replay borrows retained HIR expression identities; the authored projection
derives an identity lazily only when a type site needs it. Emitted identities,
paths, ordinals and canonical ordering are unchanged. The saved allocations
reduce actual builder work without changing any pre-charge or budget limit.

Canonical HIR identity validation compares the retained bytes directly with the
length-delimited expression/value identity encoding, without allocating a second
identity string solely for comparison. It checks every UTF-8 byte, execution
kind, owner, path and length delimiter, plus the value identity's cached hash.
The ordinary allocating encoder and the recursive validation reference remain
independent regression oracles. This removes actual temporary allocations and
their builder debit; it does not change any pre-bound, cap, canonical source
revision, identity spelling or source-graph JSON. Workspace/Project usage
receipts (and digests incorporating those receipts) may decrease accordingly.
Owned identity construction and all remaining allocations keep their existing
budget checks, including fail-closed overflow and nested parent debit.

The [shared-loan planner](SHARED-LOAN-PLAN-V1.md) uses bounded transient
forward-reachability rows and a fixed point queue. Its maximum 74 bytes per
expression (two points, each with four `u64` words, two `u16` links and one
membership byte) is included in `HIR_EXPR_FIXED_BUNDLE`; the boxed carriers
and queue indices are included in `HIR_FUNCTION_FIXED_BUNDLE`. Existing
compile-time assertions keep both bundles under the unchanged sixteen-footprint
fixed structural allowance. The forward table and queue do not survive a
canonical build or replay, and retained loan-plan capacity stays independently
charged only for nonempty own-root proof sidecars. This is an allocation bound,
not a measured change to the historical corpus or receipt values.

The identity factor was re-derived from measurement. A declaration identity at
one resolved occurrence is retained by the HIR node, the declaration, type and
call indexes, the validation sets, the cleanup inventory, the cleanup-plan
projection, and the retained edge set: eight structures, each able to hold it
as a map key and as a value, which also covers the transient resolver maps
live beside them. The previous factor of 64 additionally charged the copies
that the identity-slot count already enumerates, so every slot was billed for
the whole resolver twice. Measured with a counting global allocator around the
core build, lengthening every identity in a package by one byte raises the
heap that build retains by 0.87 to 1.11 bytes per identity slot — `std.test`
460 bytes over 416 slots, `std.core` 1,378 over 1,385, `std.bytes` 2,080 over
2,378, and `std.data.json.token` 2,409 over 2,755 — against 64 charged, so
sixteen keeps a margin above measurement wider than the structural factor's.
The structural and string factors were measured the same way and left alone:
adding a function to `std.core` charges 24 structural bytes per structural
source byte where the core build retains 3.8, and the string term is 1.2% to
2.5% of the estimate on the `std.data.json.*` packages. The tracked
`bounded_output` debit is not an under-count worth re-deriving against: it is
66% to 79% of the retained heap the allocator measures on the same builds.

An imported declaration is charged for what the synthetic projection retains,
not for the provider's source. A function import becomes a stub: the rewritten
signature, no preconditions or postconditions, and the return type's default
expression as the body. Its pre-bound is therefore the signature at the
resolved-structure rates plus that default expression, and the provider's
contract and body are charged once, in the module that declares them. Building
the stub clones one provider function transiently before replacing its body, so
the largest single imported contract is charged once per module in the earlier
estimates, or once across the sequential module loop in the final fallback,
at the raw AST rate with no structural expansion: no node of it becomes HIR. A type import is
retained in full and keeps its full charge. Before this split, every importing
module re-charged each imported body as resolved structure, so a conformance
module that imports every function its library exports cost a second complete
copy of that library — measured at 142,043 of the 226,999 raw pre-bound bytes
of `std.data.json.tests`, expanded by 24, which is why two packages of about
4.5 KiB could not be linked together. With that charge corrected, the budget
is spent on the whole package - library, examples, and conformance modules
plus any vendored `[dependencies]` source. Measured by padding the six JSON
packages with trivial `i64` helpers until `SPX-G171` fires, one package admits
between 19.7 KB and 22.1 KB of total source, varying with declaration and
expression structure rather than byte count alone. The same measurement before
the identity term was re-derived gave 12.3 KB to 14.2 KB.

Output is written through a hard sink before allocation. Exactly 16,777,216
bytes succeeds; one more reports `SPX-G171` for `output_bytes` with no partial
JSON. Typed render/digest disagreement is `SPX-G173`.

## Ordered nonclaims

The exact `nonclaims` array is:

1. `no_exclusive_lock_stage_publish_apply_or_commit_authority`
2. `not_patch_evidence_signature_provenance_approval_or_reusable_authorization`
3. `not_general_formal_proof_or_behavioral_equivalence_certificate`
4. `no_target_codegen_artifact_project_test_or_external_execution`
5. `no_cross_file_impact_review_context_repair_or_patch_generation`
6. `no_cross_file_agent_context_embedding_or_search`
7. `no_generic_cross_file_composition`
8. `no_cross_file_resource_interface_ownership_borrowing_or_lifetime_composition`
9. `no_reexport_wildcard_implicit_or_ambiguous_imports`
10. `no_dynamic_linking_package_registry_network_or_dependency_fetch`
11. `no_raw_working_tree_git_editor_or_unmanaged_file_analysis`
12. `no_create_delete_move_or_flat_materialization`
13. `no_incremental_cache_persistence_or_repository_index`
14. `no_automatic_recovery_rollback_cleanup_or_gc`
15. `no_power_loss_network_nfs_or_overlay_guarantee`
16. `no_acl_xattr_ads_preservation`
17. `no_new_patch_source_graph_cleanup_backend_or_runtime_semantics`
18. `no_external_consumer_compatibility`

## Authority and evidence status

Snapshot holds one shared semantic-workspace lock while acquiring the
authenticated snapshot, performing the retained unified build, projecting and
rendering the graph, rechecking held objects and inventory, and performing a
checked unlock. No raw graph or authority escapes. The module exposes no
parser, verifier, source constructor, write, stage, `ACTIVE` pivot, backend, or
runtime authority.

Local canonical-wire, digest, mutation, cap, API/CLI, and preservation gates are
present. The internal literal whole-document fixture pins raw SHA-256
`sha256:6639d985e25d4d33a72e37034c6e3f116940d3598bbf46162a6baaeb547da972`;
the distinct public managed-workspace fixture pins raw SHA-256
`sha256:64dddc0c2046766640ec93b7a7249214d099f683a2b6f26f43cdc22073764a6c`.
Exact-head release evidence is HOSTED GREEN for v0.4.0; this document makes no status
promotion.

The additive Project-v7 linker admits one narrow exception to the former
value-only function boundary: a monomorphic imported function may accept exact
`borrow Slice<u8>` parameters only when its return is a non-borrowing scalar.
`Slice<u8>` is not globally added to signature-type admission, so borrowed or
owned byte returns, strings, arrays, named aggregates, resources, generics,
and other non-Value modes remain rejected. The one build that widens this is
the package-source workspace, which additionally admits a whole `own Bytes`
imported parameter under the same non-borrowing-scalar-result condition; see
[Multi-Package Source Capsule v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md).
That admission is scoped to that build and changes no Project, draft, or
candidate boundary. Linked HIR revalidates lexical
borrowing, exact symbolic root provenance, call closure, cycles, and capacity
before backend selection. This grants no persistence, mutation, lifetime
erasure, or general cross-file ownership composition.
The frozen Workspace Semantic Graph v1 `nonclaims` bytes above remain
unchanged: the Project-v7 linked-execution admission does not grant this
read-only carrier general borrowing or lifetime authority.

The internal owned-data source-call lane retains its existing authenticated
monomorphic record trees with owned Bytes and scalar leaves. It admits a
zero-argument or scalar-value factory, consuming record updates, and borrowed
record observers without selecting a public host descriptor. Every nominal
signature identity, including exposed nested records, must have an explicit
direct `use type` edge in the caller. Inference of a factory's result grants
no type-import authority. A refusal keeps `SPX-G172` and its original import
span; bounded help names the first missing nominal identity, provider module,
and exact type-import syntax from authenticated declaration metadata.

An unknown persistent import identity keeps its original `SPX-G172` message,
path, and import span. Advisory help names bounded, escaped identity/module
labels and a quoted `semaprax query` command for the provider path already in
the validated module inventory. It neither guesses a replacement nor adds
import, source-repair, or filesystem authority.

Non-executable import prototypes for exact one-Bytes-plus-usize owner renewal
forward their existing owning parameter when the remaining operands are the
admitted scalar values, borrowed records, or borrowed `Slice<u8>`/`str` views.
The same internal source lane admits a monomorphic, effect-free Bytes writer
with exactly one `own Bytes` parameter, a Bytes result, and only scalar value
or borrowed `str`/`Slice<u8>` companions. Import validation independently proves
the actual current provider's nonallocating transitive body before admission;
the projection repeats that proof before forwarding the existing owner. A
matching signature alone is insufficient. Allocating, generic, ambiguous or
foreign-origin providers retain the ordinary refusal, and public scalar/host
export roots gain no Bytes result authority.
Factories retain their bounded default prototypes. Linked execution replaces
every prototype with the real provider body and independently revalidates
allocation reachability, ownership, loans and canonical cleanup. Forwarding
therefore grants no exemption for an allocating provider or aliased input.
No workspace graph, cleanup, loan or public ABI schema changes here. The
source-only regression additions await the shared qualification batch.

## Additive source Agent execution metadata (v070 candidate)

Source Agents with embedded deterministic operations or `model_wait_v1` metadata select `semaprax.workspace-semantic-graph.v4`. The existing graph body is preserved over its actual v1/v2/v3 base; the appended `agent_execution` object records `base_schema`, `authority: "none"`, and module/path-qualified checked Agent rows. Source rows follow the [owning syntax contract](LANGUAGE-NATIVE-AGENT-SYNTAX-V1.md#additive-embedded-execution-metadata-v070-candidate). Facts come only from the selected reachable closure after complete authenticated-source validation. No metadata means unchanged legacy schema and bytes. These descriptive facts grant no execution, storage, or publication authority.

### Selected native Rust imports in Project frontend projections

Explicit indexed Project frontend admission selects
`semaprax.workspace-semantic-graph.v5` and
`semaprax.project-semantic-graph.v5` when retained modules contain selected Rust
imports. The additive `indexed_rust_imports` object has `authority: "none"`
and an `imports` array ordered by module path and persistent import ID. Rows
record `id`, `path`, `rust_path`, `selected_index_digest`, `receiver`, `result`,
`effects`, and `failure_domain` (`null` for infallible imports). The existing
module/declaration/edge projection remains intact. Source and selected index
facts are checked by the [indexed native Rust Project route](NATIVE-RUST-INTEROP-V1.md).
No selected import means unchanged graph schema and bytes. This addition does
not add selected-index inputs to managed `ACTIVE` snapshot admission.

### Explicit native law sources in Project graphs

A Project with one or more manifest v2 `law_sources` selects
`semaprax.project-semantic-graph.v6`. The graph retains its ordinary checked
module, declaration, and call-edge projection and adds `law_modules` from the
explicitly selected native source modules. Each typed law row includes its
persistent ID, closed selector, canonical scalar proposition, evidence
requirement, and source owner. `law_dependencies` records the exact persistent
function identity and clause kind selected by contract laws; independent
scalar relations have no function dependency. Both arrays are derived from
authenticated Project bytes and included in the graph digest. A Project
without native laws keeps its previous graph schema and bytes. These rows are
declarations, not proof or publication authority.

### Nested fresh-resolution attribution (OPT778 diagnostic tranche)

Fresh resolution now holds a fixed compiler-owned lowering label and nests labels
for function and instance loan plans, function and instance cleanup inventories,
canonical cleanup plans, yield profiles and final HIR validation. Instance discovery,
type/expression lowering and provenance construction remain within lowering. The
first refused reservation keeps its innermost label after return or unwind. Nested
budgets retain their existing independent unlabelled start and parent restoration.

Stage handles retain the existing budget Rc without allocating another control
block. Labels are enum values and static strings; there are no per-node formatted
labels, source-controlled names or hot-path logs. Staging changes neither budget
layout, debits, floors, ceilings nor failure selection. It identifies the operation
enclosing a refusal; it does not identify an allocator call or establish retained
heap/RSS, application recovery or a path-arena saving. OPT778 path representation
and accounting changes remain outside this diagnostic-only tranche.

### Fixed-arity lowering result extraction

Binary and conditional lowering retain their existing continuation scheduling.
Their finishing receivers precheck that every child result exists, then move a
pair or triple directly from the result stack into authored order. They allocate
no temporary result Vec and clone no child. Short stacks refuse before mutation
with the existing continuation invariant diagnostic. Variable-arity splicing and
all final HIR/identity, loan, cleanup and status construction remain unchanged.
Existing conservative forecasts remain unchanged. The saved split-off test oracle
compares full HIR/wire, canonical plans and reference outcomes, while a separate
requested-layout control isolates the eliminated two/three-element buffer. This
does not attribute any existing application reservation refusal to that buffer.

### Resolver path arena (OPT778 representation tranche)

Iterative expression lowering no longer owns or copies a complete path string per
continuation. Each frame retains a four-byte private coordinate into a per-call
arena. A node retains only its parent, its own edge (a static segment from one
closed spelling table and/or one checked decimal index), its checked accumulated
UTF-8 length and its creation height; no node repeats an ancestor prefix. Root,
contract, generic and synthesized spellings are borrowed unchanged. Coordinates
are construction state only: they are never source-membership evidence, never
intern or hash a public identity and never skip source, HIR, loan or cleanup
validation.

Final `ExpressionId` and local `ValueId` text is streamed back to front into one
buffer of the exact `scoped_identity` length: the same owner (including the
generic execution spelling), domain, decimal length and path bytes. The identity
bytes are charged with the formatting route's non-required reservation before
allocation, the shared carrier keeps its existing required charge, and the exact
buffer needs no capacity normalization. A refusal leaves the empty text the
formatting route left. The parent walk uses no scratch and visits at most one
node per path byte. `scoped_identity` remains the construction route elsewhere
and the independent oracle; the test-only recursive reference keeps text paths
through that encoder. Closure lowering, record-pattern traversal and synthesized
`for`/`for own` lowering are the receivers that require owned text; each
materializes once with an exact pre-allocation charge.

Node backing is required work: every allocation is debited before it is made, so
the cumulative debit covers the old/new overlap of growth; a refusal is sticky
and later coordinates spell empty text. Nodes above the frame-stack height are
released at each pop. Every edge is created after its event's first push and is
pushed at once, so at most one node is live per frame-stack slot and the backing
never exceeds the frame vector capacity. `RESOLVER_FRAME_BYTES` is therefore the
physical frame size plus one 20-byte node: a per-slot bound for frame and arena
storage together. Forecast formulas, budgets, floors, ceilings, the 64 MiB cap
and failure selection are unchanged; this constant is the only forecast input
that moves, by the node size minus the removed inline `String` slot. Its exact
numeric value, and every pinned builder phase peak derived from it, must be
re-derived from a compiled build before they are re-pinned; this source-only
tranche asserts neither. No retained HIR identity storage is removed and no
application recovery, token, runtime, memory or RSS gain is claimed; Catalog23
and Shift15/MAX evidence requires actual owning execution.

The unchanged literal 1 MiB identity receiver keeps all three invocations and
full old HIR/work/static parity on success; physical refusal timing follows
actual requested backing. Each owned and borrowed receiver additionally has
measured full payment exact/one-short controls with first receipt, sticky zero
and one probes, allocation-free retry, unchanged debits and complete Drop.
`identity_resolution/tests/refusal_reference.rs` preserves the full historical
threshold body verbatim as source provenance, not a current execution claim.
