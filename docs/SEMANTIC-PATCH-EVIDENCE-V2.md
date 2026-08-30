# Semantic Patch Evidence v2

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, plus compiler contributors.

Semantic Patch Evidence v2 additively binds the unchanged Semantic Review v1
facts and complete supporting evidence to one independently rebuilt Semantic
Target Evidence v1 report. It remains a bounded single-file proof carrier:
exact replay proves only the frozen capsule bindings, not execution, safety,
compatibility, provenance, approval, or authority.

The authored [ordinary native String cleanup correction](NATIVE-INLINE-STRING-SETTLEMENT-V1.md)
changes native target bindings for String-bearing subjects. A capsule binding
older compiler output does not silently migrate: it must match complete
current replay or reject with `SPX-G132`. Schema, digest domains, Evidence v1,
and String-free known answers are unchanged. This is not evidence that the new
native runtime or its regressions have executed.

## Commands, API, and apply order

```text
semaprax patch-evidence-v2 <file> <patch.spatch>
semaprax verify-patch-evidence-v2 <file> <patch.spatch> <evidence.json>
semaprax patch-with-evidence-v2 <file> <patch.spatch> <evidence.json>
```

The public functions are
`semaprax::patch_evidence::{generate_v2, verify_v2, apply_v2}`. Their exact
signatures inside that module are:

```rust
pub fn generate_v2(source_path: &Path, patch_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn verify_v2(source_path: &Path, patch_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn apply_v2(source_path: &Path, patch_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>
```

`patch_evidence::{generate_v2, verify_v2}` return their complete canonical
artifacts including one terminal LF. `apply_v2` returns only the candidate
revision. Its CLI prints exactly:

```text
applied semantic patch with exact evidence replay; graph is now <candidate_revision>\n
```

The v2 apply route acquires the unchanged A0 lock first, then owns bounded
patch and evidence reads, authenticates the source, independently rebuilds
Review v1, supporting evidence, and Target Evidence v1, and requires exact
typed and byte replay before staging. Rejection may acquire/release the lock
but creates no stage and performs no source write. A0 alone owns source/stage
checks and commit authority. Ordinary `patch`, all Evidence v1 APIs/commands/
bytes/KATs, and Review v1 remain unchanged.

## Canonical capsule and receipt

Both artifacts are one canonical UTF-8 JSON line plus one LF. BOM, CR,
additional lines, duplicate keys, noncanonical spelling/order, and nesting
deeper than 8 reject. Capsule top-level key order is:

```text
schema, source_graph_schema, base_revision, candidate_revision, source, patch,
review, assessments, supporting_evidence, target_evidence, limits, budget,
nonclaims
```

Nested orders are:

```text
source: digest
patch: schema, digest
review: schema, digest
assessments: behavior, api_identity, security_authority, memory_ownership,
             target_artifact, migration, unsafe
supporting_evidence: id, kind, schema, digest
target_evidence: id, kind, schema, digest
limits: max_source_bytes, max_patch_bytes, max_evidence_bytes,
        max_operations, max_declarations, max_callables, max_call_sites,
        max_impact_depth, max_impact_nodes, max_impact_bytes,
        max_review_bytes, max_target_evidence_bytes, max_graph_bytes,
        max_native_c11_bytes, max_wasm_core_bytes, max_receipt_bytes
budget: used_source_bytes, used_patch_bytes, used_operations,
        used_declarations, used_callables, used_call_sites,
        used_impact_depth, used_impact_nodes, used_impact_bytes,
        used_review_bytes, used_target_evidence_bytes, used_base_graph_bytes,
        used_candidate_graph_bytes, used_base_native_c11_bytes,
        used_candidate_native_c11_bytes, used_base_wasm_core_bytes,
        used_candidate_wasm_core_bytes, used_evidence_bytes
```

The capsule schema is `semaprax.semantic-patch-evidence.v2`.
`target_evidence` is exactly id `evidence:1`, kind
`semantic_target_evidence_v1`, schema
`semaprax.semantic-target-evidence.v1`, and the domain-separated report digest.
`supporting_evidence` remains id `evidence:0`: complete Impact v1 for Patch
v1/v2 or the shared identity rebase for the sole Patch v3 operation.

The receipt schema is
`semaprax.semantic-patch-evidence-verification.v2`, result is `exact_replay`,
and top-level order is:

```text
schema, result, source_graph_schema, base_revision, candidate_revision,
source, patch, patch_evidence, review, assessments, supporting_evidence,
target_evidence, limits, budget, nonclaims
```

`patch_evidence` order is `schema, digest`. Other nested orders inherit the
capsule. Receipt budget order is:

```text
used_source_bytes, used_patch_bytes, used_evidence_bytes, used_operations,
used_declarations, used_callables, used_call_sites, used_impact_depth,
used_impact_nodes, used_impact_bytes, used_review_bytes,
used_target_evidence_bytes, used_base_graph_bytes, used_candidate_graph_bytes,
used_base_native_c11_bytes, used_candidate_native_c11_bytes,
used_base_wasm_core_bytes, used_candidate_wasm_core_bytes, used_receipt_bytes
```

## Assessments, digests, and limits

Assessment values retain Review's closed enum. Evidence v2 independently
forces `security_authority` to `unchanged_within_admitted_domain` after exact
zero capability delta. `target_artifact` is `change_proven` when either
production target projection digest changes, otherwise
`unchanged_within_admitted_domain`. The sole canonical Patch v3 identity
rebase is exactly `change_proven`; ordinary rename-only cases may remain
unchanged, while an admitted generic call-argument change is change-proven.

All v1 domains are reused unchanged: source
`semaprax.semantic-review.source-digest.v1\0`, Patch
`semaprax.semantic-review.patch-digest.v1\0`, Review
`semaprax.semantic-patch-evidence.review-digest.v1\0`, Impact support
`semaprax.semantic-review.impact-digest.v1\0`, and identity-rebase support
`semaprax.semantic-review.identity-rebase-digest.v1\0`. Target report binding
uses `semaprax.semantic-target-evidence.report-digest.v1\0`. The v2 artifact
domain is `semaprax.semantic-patch-evidence.artifact-digest.v2\0`. Every digest uses
`SHA-256(domain || little_endian_u64(byte_length) || exact_bytes)` and wire form
`sha256:<64 lowercase hexadecimal digits>`.

Limits are source 16 MiB, patch 4 MiB, evidence and receipt 65,536 bytes,
operations 4,096, parsed declarations 4,096, callables 1,024, call sites
65,536, Impact depth/nodes 1,024 and bytes 16 MiB, Review 32 MiB, Target report
65,536 bytes, each Graph and native C11 source 32 MiB, each Wasm core module
16 MiB, and JSON depth 8. Accounting is exact and fixed-point rendered.

Diagnostics retain Evidence v1's stable families: `SPX-G130` format/schema/
capsule-receipt confusion, `SPX-G131` bounds, `SPX-G132` replay mismatch,
`SPX-G133` typed invariant or authenticated snapshot/preflight disagreement,
and `SPX-I208` evidence-file I/O/UTF-8. Nested Review `SPX-G120` and Target
Evidence `SPX-G140` are normalized to Evidence `SPX-G131`; nested Review
`SPX-G121` and Target Evidence `SPX-G141` are normalized to Evidence
`SPX-G133` and do not escape through the v2 API. Existing Patch, source-
snapshot, and A0 families remain in force, including `SPX-I202` for patch
input and `SPX-I207` for source/final-check failures.

## KATs and evidence

Raw whole-artifact SHA-256 KATs are:

| Patch schema | Capsule SHA-256 | Receipt SHA-256 |
| --- | --- | --- |
| v1 | `f57ed8fe1f9b97c7626e15d2a3376381a27e6349a9b74ff4f74583aeecdcf2ad` | `6d8c78110542bfdaa4c07cd2e0f164bfab61d461c3e1b2ad916a9689adcde46b` |
| v2 | `a8f9a3898c9caf777dffefc0ac615e8e9afd37eced90e456373c2e4cb668b667` | `2502b809859550c67570f44e6201924516568b8af8a18d2186de55f65ef86344` |
| v3 | `a479473e5e2b973516f23c43b9d68cec1c77c32779eab2b1558a25a1bd6fbb78` | `0144d1dfc21e08b057c972d861b48aefa3e61e1a0c4d259a25cc010941739ddd` |

Evidence v2 integration is 8/8. Target Evidence integration is 9/9 and its
units are 4/4. Root library 439/439; full workspace/all-target/all-feature,
release, host 11/11 and loader 26/26 doctests, rustdoc with warnings denied,
strict Clippy, formatting, diff, preservation, and security gates are locally
green. The exact `fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted
green in [run
31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793), with
[dependency job
93624123614](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123614),
[Ubuntu job
93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631),
[macOS job
93624123633](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123633),
[Windows job
93624123715](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123715),
[component job
93624123698](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123698),
and [MSRV job
93624123711](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123711);
all 12 jobs passed.

Hosted integration compiles/runs the exact candidate C at O0/O2 and exact
candidate Wasm through Node, but those gates validate emitted artifacts only.
No execution result or project-test status enters the report, capsule, or
receipt and none becomes runtime authority.

## Exact nonclaims

The capsule and receipt carry this ordered array verbatim:

```text
not_signature_or_authenticated_provenance
not_human_approval_or_policy
not_safe_compatible_or_abi_verified
no_commit_authority
no_reusable_authorization_token
no_project_test_discovery_or_execution
no_native_toolchain_or_runtime_execution
native_evidence_is_deterministic_c11_source_only
wasm_evidence_is_deterministic_core_module_only
no_agent_context_or_repository_analysis
no_multi_file_transaction
no_general_proof_system_or_capability_flow_theorem
no_persistence_or_incrementality
no_external_consumer_compatibility
no_new_patch_repair_graph_cleanup_or_runtime_semantics
```

Evidence v2 is not a signature, provenance, approval, token, target verifier,
test runner, native compilation/machine-code/ABI record, Wasm runtime or
multi-engine conformance record, Context/repository analysis, multi-file
transaction, consumer compatibility proof, general theorem, or new semantic
operation. It grants no authority. This additive target-bound slice changes no
completion status and does not replace the next strategic multi-file tranche.

[Semantic Workspace Transaction v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md) is
the later separate managed-generation publication tranche. It neither embeds
nor widens Evidence v2; `no_multi_file_transaction` remains exact for every v2
capsule, receipt, and A0 apply route.

The additive [Semantic Workspace Patch Evidence
v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) admits child Evidence v1 only and
explicitly excludes Target Evidence and Evidence v2 aggregation. No v2
artifact, KAT, command, API, or nonclaim changes.
