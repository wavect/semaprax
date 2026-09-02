# Bounded Semantic Impact v1

Audience: agent and tool authors, plus compiler contributors.

Status: implemented as a deterministic, read-only, single-file patch preview.
The exact `1b3731a` full hosted matrix is green in [run 31408654657 attempt
2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
including [Ubuntu job
93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).

## Command and limits

The CLI accepts an existing Semantic Patch v1 or v2 file without applying it:

```text
semaprax impact <file> <patch.spatch>
  [--depth N] [--max-bytes N] [--max-nodes N]
```

The public Rust surface is `impact::preview` with `SemanticImpactOptions`.
Defaults are depth 1, 64 KiB, and 256 affected callable nodes. Depth is bounded
to 0..=1024, the whole JSON document to 1 KiB..=16 MiB, and nodes to
1..=65,536. Options are closed and single-use. Missing values, leading zeros,
negative or non-decimal values, duplicates, and unknown options reject before
semantic output; the CLI exits 2.

Semantic Patch v3 belongs to the separate bounded diagnostic-repair apply
path. Impact v1 rejects every syntactically valid, canonical v3 as `SPX-G110`
before semantic selector interpretation. Malformed or noncanonical v3 remains
`SPX-G101`; v1/v2 report bytes and behavior remain unchanged.

## Canonical report

`semaprax.semantic-impact.v1` is canonical compact UTF-8 JSON with this fixed
top-level order:

```text
schema, source_graph_schema, base_revision, candidate_revision,
patch, operations, changes, query, budget, truncation,
frontier, affected_functions
```

Nested objects and variant objects also have frozen key order. Consumers must
not infer a different order from generic JSON-map behavior:

```text
patch:
  schema, digest

operation kind "rename":
  index, kind, target, to
operation kind "rename_member":
  index, kind, owner, member, to
operation kind "rename_case":
  index, kind, owner, case, to
operation kind "replace_call_type_argument":
  index, kind, expression, template, old_instance,
  argument_index, from, to
operation kind "require_no_new_effects":
  index, kind

change kind "rename":
  kind, target, target_kind, before, after, classification,
  operation_indices, source_consumers
change kind "call_instance":
  kind, expression, containing_function, containing_kind, template,
  before_type_arguments, after_type_arguments,
  before_instance, after_instance, classification,
  operation_indices, source_consumers

source_consumer:
  id, kind, identity_origin, roles, site_count
query:
  direction, depth, max_bytes, max_nodes
budget:
  used_bytes, used_nodes, max_depth_used
truncation:
  truncated, reasons, omitted_known_nodes, deferred_known_nodes
frontier entry:
  id, kind, depth, reasons, operation_indices
affected_functions entry:
  id, kind, depth, operation_indices
```

`query.direction` is exactly `reverse`. Callable `kind` and
`containing_kind` values are `function` or `function_template`.
`classification` is exactly `source_projection` for a rename and
`behavioral_call_instance` for a call change. `identity_origin` is `explicit`
or `automatic`; compiler-owned consumers reject rather than appearing in the
report. Arrays of `operation_indices` are ascending.

The report binds the program-selected Graph v10/v11/v12/v13/v14 schema, exact
preflight base and candidate revisions, input patch schema, and exact processed
patch bytes. `operations` preserve authored instruction order, including
requirements. `changes` preserve the first contributing operation order while
coalescing exact duplicate renames and all selected argument changes for one
call expression. Every change carries its contributing `operation_indices`.

Within each change, `source_consumers` are ordered first by this closed kind
order and then by declaration ID:

```text
function, function_template, resource, record, field, variant,
variant_case, case_field, interface, import
```

Each consumer's `roles` use the closed order `declaration`, then `reference`.

Rename changes are classified `source_projection`. Their `source_consumers`
cover every exact planned edit and report the enclosing declaration ID/kind,
explicit or automatic identity origin, declaration/reference roles, and site
count. A rename alone does not populate `affected_functions`; source-consumer
coverage is not a claim of behavioral impact.

An exact generic-call instance change is classified
`behavioral_call_instance`. It records the expression, containing persistent
function or function template, template, before/after direct-scalar arguments,
and before/after instance IDs. Multiple argument operations on the same call
form one change and one seed.

## Finite reverse-call closure

Behavioral seeds traverse only the validated HIR's persistent authored
function/function-template call graph. Calls in `requires`, the body, and
`ensures` are indexed. The reverse closure is finite because it contains only
those callable declarations in the current single program; there is no lazy
repository discovery or unbounded external expansion.

The implementation computes that complete finite reverse closure before
applying `depth`, `max_nodes`, or `max_bytes`. These are output-selection and
truncation limits, not traversal-work limits. `operations`, `changes`, and
every change's complete `source_consumers` are mandatory and never truncated;
their bytes are part of the mandatory envelope. Only the known
`affected_functions` closure is prefix-selected. If the fixed envelope plus
the required first omitted-depth frontier cannot fit, the report fails as
`SPX-G109` instead of dropping provenance or consumer facts.

Traversal is breadth-first, globally ordered by stable ID at each depth. A
callable appears once at its minimum depth. Operation provenance is the sorted
union of paths that reach it at that same minimum depth; longer cycle or
diamond paths do not add provenance. Depth 0 contains the directly changed
call owner.

`affected_functions` is the largest canonical prefix that fits all requested
limits. If a limit cuts the closure, `frontier` contains the remaining known
nodes at the first omitted depth; deeper known nodes are counted as deferred.
Closed truncation reasons are ordered `depth`, `max_nodes`, `max_bytes`.
`omitted_known_nodes` counts the whole known suffix and
`deferred_known_nodes` excludes the emitted frontier. `used_bytes` is exactly
the returned JSON byte length and excludes the CLI newline. `used_nodes` is
exactly the number of emitted `affected_functions`; it excludes operations,
changes, source consumers, and frontier entries. `max_depth_used` is the depth
of the last emitted affected function, or 0 when none is emitted. If the
mandatory envelope cannot fit, the query fails as `SPX-G109`; it never emits
oversized or invalid JSON.

## Snapshot and digest boundary

Preview canonicalizes and snapshots one regular source leaf, parses and
verifies the patch against owned source/patch buffers, builds the report, then
rechecks exact source path identity, bytes, and revision before returning. It
does not acquire the patch commit lock, create staging siblings, rename, or
write source. A concurrent source byte or identity replacement fails closed as
`SPX-I207` and produces no report.

Unix source identity is exact device/inode. Windows holds same-file handles and
compares volume plus the available 64-bit file index; this does not claim ReFS
128-bit or hostile non-unique-index uniqueness.

The patch file remains trusted input. It is read once; the report digest binds
the exact owned bytes that were processed, not continuing patch-path identity
or provenance. The digest is:

```text
SHA-256("semaprax.semantic-impact.patch-digest.v1\0"
       || little_endian_u64(byte_length)
       || exact_patch_bytes)
```

A patch-path mutation after that read cannot change the report's operations or
digest, but callers that require authenticated patch provenance must snapshot
or authenticate it externally. Preview/apply candidate-revision equality is
tested for the same starting bytes; preview does not reserve or commit that
candidate after returning.

## Diagnostics and closed domains

- `SPX-G109` reports invalid Impact options or a mandatory envelope that cannot
  fit `max_bytes`.
- `SPX-G110` reports an Impact invariant or identity-domain failure, including
  incomplete source-consumer coverage, base/candidate Graph-schema drift, an
  inexact call selector/owner, or an automatic callable entering behavioral
  reverse closure. It also rejects every syntactically valid, canonical
  Semantic Patch v3 before any v3 semantic selector is interpreted; malformed
  or noncanonical v3 remains `SPX-G101`. Automatic declarations may still appear as exact
  rename source consumers; they are not admitted as behavioral closure nodes.
- Existing Patch v1/v2 parse, stale-selector, verification, and requirement
  diagnostics pass through unchanged. In particular, `SPX-T226` continues to
  reject generic composition, recursion, effects, or other generic-function
  execution outside the admitted monomorphic slice before any Impact report.

Consequently, an exact generic-call change inside a function template cannot
currently become an Impact seed: that source shape requires generic
template-to-template composition and fails as `SPX-T226` before reporting.
Function-template nodes are nevertheless indexed and may appear as persistent
reverse callers of an admitted monomorphic call-owner seed.

Impact v1 does not add Graph v15 or a CleanupPlan schema. It is not
repository-wide or multi-file analysis, a persistent or incremental index, a
general reverse semantic graph, or a computation of type, ownership, data,
capability, cleanup, import, target, diagnostic, test, schema, migration, or
artifact consumers. It does not rank relevance, generate repairs, prove patch
claims, summarize semantic review, authenticate patch provenance, or commit a
change. Call edges found in contracts are ordinary call-graph edges, not a
general contract-dependency analysis.

The separate [Semantic Review v1](SEMANTIC-REVIEW-V1.md) layer embeds the
complete, nontruncated canonical Impact v1 report for Patch v1/v2 under fixed
Review limits. That composition does not change Impact's schema, bytes, public
options, v1/v2-only domain, or canonical-v3 `SPX-G110` rejection; Impact itself
still does not emit the seven review sections.

The separate [Semantic Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md)
capsule may bind Review's domain digest of that complete embedded Impact v1
object. Impact itself remains byte-identical, v1/v2-only, read-only, and not a
target projection. Additive [Evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md)
retains that exact Impact binding and separately binds
[Target Evidence v1](SEMANTIC-TARGET-EVIDENCE-V1.md); it does not widen Impact
to Patch v3, targets, repositories, or multi-file analysis. Impact is not a
proof carrier, verifier, authorization token, or apply/commit authority.

Project candidates use a distinct retained multi-source impact artifact. Its
additive [candidate impact navigation](PROJECT-CANDIDATE-IMPACT-NAVIGATION-V1.md)
exposes compact metadata and exact pages over that artifact's existing arrays.
It does not change this single-file Semantic Patch Impact v1 schema, bytes,
traversal or evidence claims.

## Evidence

The canonical rename-report SHA-256 KAT is
`94bbe5dcfe02f4b80b12ba5c8faf0889ddf11a96598072e539490c71a09518e9`.
The local focused integration suite is 12/12 and the internal Impact/call-index
suite is 4/4. The same exact `1b3731a` full matrix is hosted green in [run
31408654657 attempt
2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
including [Ubuntu job
93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).
Focused integration evidence covers exact patch-byte digest binding,
read-only inventory, v1/v2 operation/change provenance, preview/apply revision
parity, CLI confusion, explicit-owner closure, automatic-owner fail-closed
behavior, cycles and diamonds, requires/body/ensures calls, all Graph
v10-v14 source schemas, all admitted rename domains, deterministic
high-cardinality patches and closures, and exact byte/node/depth frontiers.
Internal tests cover exact call-expression lookup plus source byte/identity
final checks and patch mutation after its single read.

Focused gates:

```text
cargo test --locked -p semaprax --all-features --test semantic impact::
cargo test --locked -p semaprax --all-features --lib impact::tests::
cargo test --locked -p semaprax --all-features --lib call_index::tests::
cargo test --locked -p semaprax --all-features --test semantic patch_v2::
cargo test --locked -p semaprax --all-features --test agent_context_v2
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

The separate [Semantic Workspace Transaction
v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md) performs no Impact-v1 or repository
analysis. Impact remains a single-file Patch v1/v2 preview and gains no
multi-file closure, persistence, ranking, repair, or commit authority.

[Semantic Workspace Patch Evidence
v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) independently rebuilds that exact
complete single-file Impact object for each Patch-v1/v2 child and binds only
its existing supporting-evidence digest. It performs no cross-file Impact
closure or repository reasoning and changes no Impact bytes, KATs, limits,
diagnostics, or nonclaims.
