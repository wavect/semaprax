# Bounded Semantic Review v1

Status: implemented as a deterministic, read-only, single-file review. The
exact `2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in
[run 31423743369 attempt
1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1); all
12 jobs passed.

## Command and public surface

The CLI accepts one existing Semantic Patch v1, v2, or the sole canonical v3
file without applying it:

```text
semaprax review <file> <patch.spatch>
```

The public Rust surface is `review::preview`. The command has fixed arity and
no flags, target selection, output selection, Context query, approval action,
or verification mode. Extra arguments reject with exit status 2. Successful
CLI output is the exact API JSON plus one LF.

Patch v1/v2 review embeds one complete, nontruncated Semantic Impact v1 report
built from the same owned Patch preflight with fixed options `depth = 1024`,
`max_bytes = 16 MiB`, and `max_nodes = 1024`. Any Impact truncation, omitted or
deferred node, or nonempty frontier rejects as `SPX-G120`; Review never
describes an omitted caller closure as unchanged. Impact v1 itself remains
v1/v2-only.

Patch v3 review accepts only the exact canonical three-line
`assign-function-id` operation admitted by Diagnostic Repair v1. It carries the
same independently reconstructed identity-rebase evidence as the repair
preview and does not invoke, embed, or widen Semantic Impact v1.

## Canonical report

`semaprax.semantic-review.v1` is canonical compact UTF-8 JSON. Its top-level
keys have this exact order:

```text
schema, source_graph_schema, base_revision, candidate_revision,
source, patch, limits, budget, sections, evidence, nonclaims
```

Nested keys have the following exact order:

```text
source: digest
patch: schema, digest
limits:
  max_source_bytes, max_patch_bytes, max_operations, max_declarations,
  max_callables, max_call_sites, max_impact_depth, max_impact_nodes,
  max_impact_bytes, max_output_bytes
budget:
  used_source_bytes, used_patch_bytes, used_operations, used_declarations,
  used_callables, used_call_sites, used_impact_depth, used_impact_nodes,
  used_impact_bytes, used_output_bytes
section: kind, assessment, findings
finding:
  code, disposition, statement, operation_indices, evidence_ids
Impact evidence: id, kind, schema, digest, report
identity-rebase evidence:
  id, kind, schema, digest, identity_rebase
identity_rebase:
  before_id, after_id, name, direct_callers,
  derived_id_count, derived_id_digest
direct caller: id, identity_origin, site_count
```

`sections` always contains these seven keys in this exact wire order:

```text
behavior, api_identity, security_authority, memory_ownership,
target_artifact, migration, unsafe
```

These keys implement RFC 0001's conceptual behavior, API, security, memory,
target, migration, and unsafe-code review categories. The RFC concepts do not
rename or reorder the frozen protocol fields.

Every section repeats its key in `kind` and contains exactly one finding for
each authored patch operation, in operation order. `operation_indices` is the
single authored operation index and `evidence_ids` is exactly
`["evidence:0"]`. Closed finding dispositions are:

```text
change, bounded_no_change, migration_required, unknown, not_applicable
```

Closed section assessments are:

```text
change_proven, unchanged_within_admitted_domain, mixed,
unknown, not_applicable
```

A single `change` or `migration_required` disposition produces
`change_proven`; a single `bounded_no_change` produces
`unchanged_within_admitted_domain`; a single `unknown` produces `unknown`; and
a single `not_applicable` produces `not_applicable`. More than one disposition
produces `mixed`. Findings use the closed `SRV-B*`, `SRV-A*`, `SRV-S*`,
`SRV-M*`, `SRV-T*`, `SRV-G*`, `SRV-U*`, and policy `SRV-P103` codes. Their
canonical statements are explanatory evidence-bound text; consumers branch on
the code and closed disposition rather than parsing prose.

Rename findings prove only a name-normalized Graph change with stable identity,
exact unchanged security facts, and checked ownership/Cleanup preservation.
Generic-call findings prove the addressed call-instance change plus the
unchanged admitted scalar-Copy ownership, Cleanup, effect, and API-identity
facts. `require no-new-effects` is a verified policy requirement and makes no
source edit. Identity-rebase findings retain the exact
`breaking_identity_rebase`: runtime behavior is preserved only inside the
closed monomorphic scalar Graph-v10 domain, while Graph identity, derived IDs,
callee references, identity-bearing Cleanup facts, backend symbol/artifact
identity, and migration requirements intentionally change. Unknown target,
external-consumer, test/cache, generated/native, and unsafe consequences remain
unknown rather than being inferred from an empty section.

## Evidence variants

Patch v1/v2 uses one evidence object with fixed values:

```text
id = evidence:0
kind = semantic_impact_v1
schema = semaprax.semantic-impact.v1
```

`report` is the complete canonical Impact v1 object. Its digest is:

```text
SHA-256("semaprax.semantic-review.impact-digest.v1\0"
       || little_endian_u64(report_byte_length)
       || exact_canonical_impact_report_bytes)
```

Patch v3 instead uses:

```text
id = evidence:0
kind = identity_rebase_v1
schema = semaprax.identity-rebase.v1
```

Its `identity_rebase` object is byte-order-equivalent to the shared Diagnostic
Repair v1 reconstruction. Its digest is:

```text
SHA-256("semaprax.semantic-review.identity-rebase-digest.v1\0"
       || little_endian_u64(identity_rebase_byte_length)
       || exact_canonical_identity_rebase_bytes)
```

V3 has `used_impact_depth = 0`, `used_impact_nodes = 0`, and
`used_impact_bytes = 0`; its evidence object has no `report` key.

The source and processed-patch digests use the same length-prefixed construction
with domains `semaprax.semantic-review.source-digest.v1\0` and
`semaprax.semantic-review.patch-digest.v1\0`. All protocol digest fields have
wire form `sha256:<64 lowercase hexadecimal digits>`.

## Hard work and output bounds

Review fails closed and never truncates:

| Limit | Value |
| --- | ---: |
| Source bytes | 16 MiB |
| Patch bytes | 4 MiB |
| Patch operations | 4,096 |
| Parsed declarations | 4,096 |
| Authored callables | 1,024 |
| Parsed call sites | 65,536 |
| Impact depth | 1,024 |
| Impact nodes | 1,024 |
| Embedded Impact bytes | 16 MiB |
| Review output bytes | 32 MiB |

Declaration, callable, and complete structural call-site bounds are enforced on
the parsed AST before HIR construction. Patch operation count is checked before
owned semantic preflight. Exact output byte accounting is fixed-point computed;
`budget.used_output_bytes` equals the returned JSON byte length and excludes the
CLI LF. Limit failures are `SPX-G120`; internal classification, Graph-schema,
or accounting failures are `SPX-G121`. Existing patch parse, stale-selector,
repair, verification, and I/O diagnostics otherwise pass through unchanged.

## Snapshot and authority boundary

Review authenticates one canonical regular source snapshot, owns the exact
bounded source and patch bytes used by preflight, constructs the complete
report, and then rechecks source identity, bytes, revision, and the same 16-MiB
bound before return. Concurrent byte, identity, revision, or over-bound growth
fails as `SPX-I207` and produces no report. Unix identity is exact device/inode;
Windows uses held same-file volume plus the available 64-bit file index and
does not claim ReFS 128-bit or hostile non-unique-index uniqueness.

The patch file is read once and its owned bytes are digest-bound, but its path
remains trusted input and is not re-authenticated. Review performs no A0 lock,
stage, rename, source write, apply, commit, reservation, or approval action.
The library exposes report construction, not a public report-verification API.

## Explicit nonclaims

Every report carries these exact ordered nonclaim strings:

```text
not_proof_carrying_patch
no_authenticated_provenance_or_signature
no_human_approval_ui_or_policy
no_public_verify_api_or_proof_artifact
no_lock_stage_apply_or_commit_authority
no_repository_or_multi_file_analysis
no_agent_context_generation_or_embedding
no_test_or_target_execution
no_general_capability_security_unsafe_or_abi_analysis
no_semantic_impact_v3
no_persistence_or_incrementality
no_external_consumer_compatibility
```

The seven mandatory sections are bounded classifications, not general security
analysis, a memory-safety proof, an unsafe audit, target compatibility or
artifact-byte evidence, external migration completeness, or human approval.
Review adds no Context contract, target execution, proof-carrying patch,
signature, verifier, authenticated provenance, general v3 Impact, new patch or
repair operation, Graph/CleanupPlan schema widening, backend/runtime semantic
change, repository index, multi-file transaction, or commit authority.

The separate [Semantic Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md)
capsule domain-digest-binds an independently rebuilt Review and carries the
bounded proof artifact. This does not change any Review byte, KAT, schema,
nonclaim, or public API: Review still exposes only read-only `review::preview`,
has no `review::verify`, and its report itself remains non-proof. Evidence's
opt-in apply authority comes from `patch-with-evidence`, not from Review.

## Evidence

The exact whole-report SHA-256 KATs are:

```text
Patch v1  054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946
Patch v2  37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421
Patch v3  081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544
```

Local Review integration is 10/10 and internal hook/limit units are 4/4.
Library 408/408, the full workspace, release, doctest, rustdoc, strict Clippy,
format, diff, preservation, and independent security gates are green. The exact
`2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in [run
31423743369 attempt
1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
including [Ubuntu job
93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170),
[Windows job
93570423172](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423172),
[macOS job
93570423226](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423226),
[MSRV job
93570423203](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423203),
and [dependency-policy job
93570423175](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423175);
all 12 jobs passed. Hosted platform/backend jobs are preservation evidence and
do not constitute target or test execution by Review.

Focused gates:

```text
cargo test --locked -p semaprax --all-features --test semantic_review_v1
cargo test --locked -p semaprax --all-features --lib review::tests::
cargo test --locked -p semaprax --all-features --test semantic_impact_v1
cargo test --locked -p semaprax --all-features --test diagnostic_repair_v1
cargo test --locked -p semaprax --all-features --test semantic_patch_v3
```
