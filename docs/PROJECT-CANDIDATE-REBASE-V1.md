# Project Candidate Semantic Rebase and Merge v1

Status: authored, unrun; a conservative slice of the
[graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md). This is not
arbitrary source merging, behavioral equivalence, or publication authority.

Audience: agent builders, compiler contributors, and reviewers.

## Public API and source bases

`ProjectCandidate::rebase(expected_candidate, new_base, expected_new_base)`
replays the candidate's typed intentions on an independently admitted
`Arc<ProjectRevision>`. Its resulting candidate diff is based on `new_base`.
Both selectors must match exactly, and old/new canonical Project manifest
bytes must be identical. Filesystem freshness belongs to the admitting host;
a retained revision is not current-path authentication.

`ProjectCandidate::merge(expected_candidate, other, expected_other)` requires
both candidates to share their original Project base revision. It finds their
longest exact common intention-history prefix, replays the other candidate's
complete history from that original base, and then replays this candidate's
remaining suffix. The common prefix is not duplicated. The resulting source
diff and complete intention evidence retain the original shared base, so
changes from the other parent are not lost by treating its revision as a new
empty base. Merge order is explicit: right suffix, then left suffix.

Both return `ProjectCandidateRebase`, exposing `candidate()` by reference,
`into_candidate()` by ownership, and `to_json()` for its separate ancestry and
classification report. Existing Candidate and Image wire schemas are unchanged.
The report binds both parent candidate digests for a merge; a rebase binds its
source candidate and the exact admitted destination Project revision. Keep
this report alongside the resulting candidate when retaining merge ancestry.

The additive [Typed-draft Rebase](PROJECT-CANDIDATE-DRAFT-REBASE-V1.md) first
uses this checked-history rebase, then separately guards and remaps pending
body, expression and contract holes. It returns only a draft and its report;
these complete-candidate APIs and their wire reports remain unchanged.
The separate [Draft Merge](PROJECT-CANDIDATE-DRAFT-MERGE-V1.md) uses one checked
history merge and independently reconciles both pending inventories; it cannot
release a candidate while any merged hole remains unresolved.

## Stable-ID conflict selection

Conflict analysis uses only the candidates' already admitted canonical
source snapshots and retained checked HIR. It indexes explicit top-level functions by stable ID and
separately fingerprints signature, body, contracts and effects/module permits.
Signatures include parameter names/types/modes and return type, excluding the
function display name. When a parameter or return type is nominal, the
fingerprint also binds the retained HIR's complete ordered type identities.
An unchanged type spelling or import alias cannot hide a different declaration
identity or concrete type arguments on the concurrent base. Scalar fingerprints
retain their previous representation. Canonical expression formatting excludes spans. Local
function calls and authenticated import aliases normalize to tokens derived
from their resolved stable-ID bindings before body/contract fingerprinting.
Those tokens exist only in the conflict calculation and are never materialized
as source. There is no whole-file conflict rule.

[Nominal declaration renames](PROJECT-NOMINAL-RENAME-V1.md) additionally use
separate source record/variant display, shape and origin fingerprints. Competing
renames or shape/origin drift reject; unrelated function edits can replay.
Existing function/aggregate hashes retain their conservative source-spelling
checks, so this does not normalize every merge across a type display rename.

[Member renames](PROJECT-MEMBER-RENAME-V1.md) use the same source fingerprint
route with explicit owner/case ancestry, selected member kind/name and the
complete owner shape. Competing member renames, including net-zero histories,
reject. Earlier member additions are resolved against each intermediate source
revision before replay; unrelated function edits still require full admission.

Typed record and variant intentions additionally bind their referenced checked
aggregate shapes before each history step is replayed. The comparison uses that
step's original and rebased intermediate revisions, so earlier successful
intentions remain part of the dependency context. Missing targets or changed
ordered member identities/types reject with `SPX-G235`; names and source
locations may also conservatively conflict. Generic template fingerprints bind
ordered parameter identities even for phantom parameters; prelude fingerprints
also bind the compiler-owned schema/digest provenance. This protects aggregate operands
even when the changed function has a scalar signature. It does not prove
transitive shape or behavioral equivalence; see
[Aggregate Constructors v1](PROJECT-AGGREGATE-CONSTRUCTORS-V1.md).

`project` operands additionally bind the selected explicit record field and
its complete checked owner descriptor. A deleted or reidentified field cannot
be recovered by matching its old name. These dependency comparisons also use
each step's original and rebased intermediate revisions; surviving projections
still reparse and check the generated exact-owner value binding.

`match` operands bind the whole checked variant owner and its ordered complete
case/payload inventory, including generic parameter and compiler-prelude facts.
Reidentifying a unit case or payload field conflicts even if names, source
types and the owning variant identity are unchanged. These guards run for
recursive match operands at each intermediate revision before ordinary replay.

`update` operands bind the complete checked record descriptor, including fields
not mentioned by the request. A concurrent change to an untouched field's
identity, type or order conflicts with `SPX-G235`. Subset and empty updates
follow this same guard and complete candidate replay.

Declaration `nominal` type objects bind the whole checked record or variant
owner, including ordered member and generic parameter inventories and prelude
provenance. This guard runs even when the added function body merely forwards
a parameter and contains no aggregate expression operand. A changed field or
unit-case identity rejects with `SPX-G235` before replay; concrete type arguments
remain bound by the exact intention. Each intermediate revision must retain
the same selected shape, and the rebuilt declaration must still pass its
checked Copy-signature gate.

Record and variant declaration creation contributes every planned owner, case
and field ID to collision checks. Nested IDs cannot collide with declarations
in the destination revision or earlier history steps. A record created earlier
in the same history can subsequently receive an `add_record_field` intention;
full admission checks its shape at that intermediate revision. Later nominal
type use and aggregate construction retain the ordinary whole-owner dependency
guards. No new type is considered present in the original source base.

| Concurrent change | Decision for replayed intent |
| --- | --- |
| Body edit and unrelated display rename, including the same source file | Replay permitted, then full admission. |
| Same-target body edit plus function display rename | Replay permitted; stable identity selects the renamed declaration. |
| Callee display rename changes a caller's source spelling | Stable-ID call normalization avoids a false caller-body conflict. |
| Target contracts changed while body/signature/display intent is replayed | Potentially compatible; full candidate rebuild required. This does not prove predicate truth or behavioral equivalence. |
| Contract append while target body changed | Potentially compatible; full rebuild required. |
| Independent contract appends with unchanged signature/effects | Append in merge order and fully rebuild. No duplicate-elimination or logical simplification is inferred. |
| Same-target competing signature intentions | Conflict, including net-zero signature histories in the merge suffixes. |
| Nominal type keeps its spelling but resolves to another identity | Signature conflict before applying a dependent signature/body/contract change. |
| Body/expression replacement versus changed target body, signature or effects | Conflict. Disjoint expression editing is not inferred. |
| Signature evolution versus changed target body, signature or effects | Conservative conflict. More permissive compatibility analysis remains future work. |
| Contract append versus changed target signature or effects | Conflict. |
| Two display renames of the same target | Conflict, even if they happen to choose equal display names. |
| Deleted target/lost explicit identity | Conflict before replay. |
| Typed constructor calls a concurrently deleted declaration or changed signature | Conflict before replay when the referenced declaration existed in the original base. |

Unrecognized future intention kinds fail closed until their conflict contract
is defined. New declarations, declaration deletion/movement, general package
changes and semantic schema migrations outside the separately specified
additive intention routes are not admitted by this v1 merge policy. Existing
direct caller migration still runs through its own authenticated source
transformation and full verifier.

## Revalidation and expression identities

Calls and nominal constructors nested in a typed `let` initializer or body
remain ordinary semantic dependencies. Scoped binding does not hide a changed
callee signature or grant permission to reuse a stale selection. Source replay
and recovery reconstruct the canonical block and binding through the shared
constructor. See [Lexical Binding Constructor](PROJECT-LEXICAL-BINDING-CONSTRUCTOR-V1.md).

The same nested-dependency checks apply to explicit signature
`argument_expression` templates. A callee display rename can be rebound through
its stable identity; a changed callee signature remains a conflict before
computed-argument reconstruction. A computed parameter's nominal type selector
also binds its complete selected owner/member inventory, independently of any
constructor used by its argument expression; a concurrent shape change rejects.
These checks use the corresponding intermediate revisions during history replay.
See
[Signature Argument Expressions](PROJECT-SIGNATURE-ARGUMENT-EXPRESSIONS-V1.md).

Conflict selection only decides whether to attempt replay. Every surviving
intention is reconstructed against the exact current destination revision and
passed to ordinary `ProjectCandidate::apply`. Canonical source is reformatted,
reparsed, independently rebuilt, and subjected to existing identity, contract,
effect, ownership/loan/cleanup, profile and core-target preservation checks.
Failures return diagnostics without changing either parent or source files.

For `replace_expression`, expression IDs are revision-scoped. After the target
body/signature/effect conflict guard passes, the expression operation maps its
original authenticated HIR origin through the compiler-derived body AST path
to the new unique HIR origin, requiring identical resolved type and ownership.
Each history step uses its own original intermediate revision and current
rebased intermediate revision; IDs are not remapped once against an obsolete
root snapshot. No guessed span/text replacement or retained stale expression
ID bypasses this check.

The candidate pipeline may emit C11 and structurally validate Wasm projections
as admission evidence. It does not run native/Wasm programs or project tests.
Neither a compatible fingerprint nor successful source admission is a proof
of behavioral equivalence or of newly appended predicates at runtime.

## Report, bounds and diagnostics

The canonical LF-terminated report schema is
`semaprax.project-candidate-rebase.v1`. It contains operation kind, left/right
parent digests, original/onto/result base revisions, result source revision
and candidate digest, shared-prefix count, and per-intention concurrent-change
classification. Its validation field describes complete candidate source
replay; `source_authority` remains false.

A merged history retains Candidate's maximum of 32 intentions. Existing
Project/source and AST traversal bounds constrain fingerprint work; individual
body/contract/fingerprint renderers use a 16 MiB limit, and the final report
uses a 1 MiB limit. These are not incremental-performance or aggregate-heap
claims. Normalization and replay do not add persistent caches or ambient I/O.

| Diagnostic | Meaning |
| --- | --- |
| `SPX-G233` | Incompatible canonical manifest, malformed/unsupported intent, or ambiguous identity. |
| `SPX-G234` | Selector, merged-history, fingerprint or report capacity. |
| `SPX-G235` | Stale selector, incompatible base, deleted target/dependency, or semantic conflict. |

Source, constructor, expression-mapping and candidate replay diagnostics keep
their owning codes. No output report is a commit token, approval or lock. A
separate source-publication authority remains necessary after any rebase.

## Authored evidence and remaining work

[Integration evidence](../tests/project_candidate_rebase_v1.rs) covers same-file
independent changes, same-target body/display compatibility, stable-ID callee
rename normalization, body/contract revalidation, competing signatures/bodies,
deleted targets, stale selectors, manifest rejection, original-base preservation
and exact shared-prefix handling. Tests and compiler/quality gates were not
run at the user's request; no local or hosted completion is claimed.

General semantic conflict reasoning, source-publication race integration,
parallel mutation scheduling, candidate persistence/recovery, cross-package
consumer migration and measured multi-agent productivity remain open.
