# Semantic Patch Evidence v1

Semantic Patch Evidence v1 is the first bounded proof-carrying-patch slice. It
turns the already admitted single-file Patch v1/v2 operations and the sole
canonical Patch v3 `assign-function-id` operation into an independently
replayable evidence capsule, and provides a separate apply route that requires
that exact replay before A0 staging or commit. Here, “proof-carrying” means a
closed deterministic artifact whose admitted facts are independently rebuilt
and byte-compared; it does not mean a signature, authenticated provenance, a
general proof system, human approval, or target verification.

## Commands and scope

The three fixed-arity commands are:

```text
semaprax patch-evidence <file> <patch.spatch>
semaprax verify-patch-evidence <file> <patch.spatch> <evidence.json>
semaprax patch-with-evidence <file> <patch.spatch> <evidence.json>
```

`patch-evidence` emits one canonical `semaprax.semantic-patch-evidence.v1`
capsule without writing source. `verify-patch-evidence` independently rebuilds
the Review and supporting evidence, requires exact capsule replay, and emits
one canonical `semaprax.semantic-patch-evidence-verification.v1` receipt.
`patch-with-evidence` performs the same independent replay and, only after it
succeeds, prepares and commits the candidate through unchanged A0.

The public functions are
`semaprax::patch_evidence::{generate, verify, apply}`. Their exact signatures
inside that module are:

```rust
pub fn generate(source_path: &Path, patch_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn verify(source_path: &Path, patch_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn apply(source_path: &Path, patch_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>
```

`generate` and `verify` return the complete canonical artifact including its
terminal LF. `apply` returns only the candidate revision string; the CLI wraps
it in the success line below, including the shown terminal `\n`:

The apply CLI success line is exactly:

```text
applied semantic patch with exact evidence replay; graph is now <candidate_revision>\n
```

The ordinary `semaprax patch <file> <patch.spatch>` route is deliberately
unchanged and does not require an evidence file. A capsule's
`no_commit_authority` and `no_reusable_authorization_token` claims therefore
remain true: authority belongs to the live `patch-with-evidence` invocation and
unchanged A0 source/stage checks, not to possession of a capsule. The evidence
route owns and independently replays its bounded, single-read patch and
evidence bytes while holding the A0 lock; A0 does not authenticate those input
files.

## Canonical capsule

The capsule is exactly one UTF-8 JSON line followed by one LF. BOM, CR,
additional lines, duplicate keys, noncanonical JSON spelling or key order, and
nesting deeper than 8 reject. The exact top-level key order is:

```text
schema, source_graph_schema, base_revision, candidate_revision, source,
patch, review, assessments, supporting_evidence, limits, budget, nonclaims
```

Closed nested objects and their key order are:

```text
source: digest
patch: schema, digest
review: schema, digest
supporting_evidence: id, kind, schema, digest
assessments: behavior, api_identity, security_authority, memory_ownership,
             target_artifact, migration, unsafe
limits: max_source_bytes, max_patch_bytes, max_evidence_bytes,
        max_operations, max_declarations, max_callables, max_call_sites,
        max_impact_depth, max_impact_nodes, max_impact_bytes,
        max_review_bytes, max_receipt_bytes
budget: used_source_bytes, used_patch_bytes, used_operations,
        used_declarations, used_callables, used_call_sites,
        used_impact_depth, used_impact_nodes, used_impact_bytes,
        used_review_bytes, used_evidence_bytes
```

`schema` is exactly `semaprax.semantic-patch-evidence.v1`; `review.schema` is
exactly `semaprax.semantic-review.v1`; `supporting_evidence.id` is exactly
`evidence:0`. The source graph schema is one of Graph v10 through v14. The
patch schema is Patch v1, v2, or v3. Assessments are copied in the frozen Review
section order and each value is one of `change_proven`,
`unchanged_within_admitted_domain`, `mixed`, `unknown`, or `not_applicable`.

For Patch v1/v2, supporting evidence is exactly kind
`semantic_impact_v1`, schema `semaprax.semantic-impact.v1`; it binds the
complete nontruncated Impact v1 object rebuilt by Review. For the sole Patch v3
operation it is exactly kind `identity_rebase_v1`, schema
`semaprax.identity-rebase.v1`; it binds the shared repair identity rebase and
does not widen Impact v1 to v3.

Every digest has the wire form `sha256:<64 lowercase hexadecimal digits>`.
The capsule binds the exact Graph schema, base and candidate revisions, source
digest, Patch schema and digest, domain-separated whole Review digest, seven
assessments, supporting-evidence kind/schema/digest, and complete work
accounting. The Review digest domain is the ASCII bytes
`semaprax.semantic-patch-evidence.review-digest.v1` plus NUL, then the
little-endian `u64` byte length and exact Review bytes.

The source and processed-Patch digests reuse Review's exact domains
`semaprax.semantic-review.source-digest.v1\0` and
`semaprax.semantic-review.patch-digest.v1\0`. The complete Impact support
digest reuses `semaprax.semantic-review.impact-digest.v1\0`; the shared
identity-rebase support digest reuses
`semaprax.semantic-review.identity-rebase-digest.v1\0`. Each of these, plus
the Evidence Review and artifact digests, is computed as
`SHA-256(domain || little_endian_u64(byte_length) || exact_bytes)`.

## Verification receipt

The receipt is also exactly one canonical JSON line plus LF. Its top-level key
order is:

```text
schema, result, source_graph_schema, base_revision, candidate_revision,
source, patch, patch_evidence, review, assessments, supporting_evidence,
limits, budget, nonclaims
```

`schema` is exactly
`semaprax.semantic-patch-evidence-verification.v1`; `result` is exactly
`exact_replay`; `patch_evidence` has key order `schema, digest`. Its budget
order is:

```text
used_source_bytes, used_patch_bytes, used_evidence_bytes, used_operations,
used_declarations, used_callables, used_call_sites, used_impact_depth,
used_impact_nodes, used_impact_bytes, used_review_bytes, used_receipt_bytes
```

All other nested receipt objects inherit the capsule's exact order:
`source: digest`; `patch: schema, digest`; `review: schema, digest`;
`assessments` in the seven-section order; `supporting_evidence: id, kind,
schema, digest`; the same twelve-key `limits` order; and the same ordered
`nonclaims`. The additional `patch_evidence` object is `schema, digest`.

The evidence-artifact digest domain is the ASCII bytes
`semaprax.semantic-patch-evidence.artifact-digest.v1` plus NUL, then the
little-endian `u64` byte length and exact capsule bytes. Exact canonical replay
requires both submitted bytes equal independently rendered bytes and all typed
bindings equal. A receipt is verification output, not an accepted capsule and
cannot be substituted into `patch-with-evidence`.

## Bounds and trusted inputs

The frozen limits are:

| Limit | Value |
| --- | ---: |
| Source bytes | 16 MiB |
| Patch bytes | 4 MiB |
| Evidence bytes | 65,536 |
| Operations | 4,096 |
| Parsed declarations | 4,096 |
| Parsed callables | 1,024 |
| Parsed call sites | 65,536 |
| Impact depth | 1,024 |
| Impact nodes | 1,024 |
| Impact bytes | 16 MiB |
| Review bytes | 32 MiB |
| Receipt bytes | 65,536 |
| Evidence JSON nesting depth | 8 |

Declaration, callable, and call-site bounds are checked on parsed AST before
HIR construction. Reads are bounded and owned. The source is a canonical
regular-file snapshot with exact final identity/bytes/revision/size checks;
patch and evidence paths are trusted inputs after their single bounded reads,
and later path mutation cannot change the owned bytes used for replay.

`patch-with-evidence` first acquires the ordinary create-new A0 sibling lock,
then reads the patch and capsule, authenticates the bounded source, rebuilds
Review and the expected capsule, and requires exact replay. Only then does it
prepare a staging file. Unchanged A0 checks revalidate exact source and staging
path/handle identity and bytes before rename, preserve source permissions and
sync staged bytes, and never remove a foreign stage replacement. Evidence is
therefore required before A0 staging and final commit, not before lock
acquisition.
Rejected evidence can therefore acquire and release the A0 lock, but it creates
no stage and the tool performs no source write.

## Diagnostics

The evidence layer adds these stable diagnostic families:

| Code | Meaning |
| --- | --- |
| `SPX-G130` | malformed, noncanonical, closed-schema, or receipt/capsule confusion |
| `SPX-G131` | source, patch, evidence, Review, Impact, AST-work, or output bound exceeded |
| `SPX-G132` | submitted capsule differs from independent canonical replay |
| `SPX-G133` | typed evidence invariant or authenticated snapshot/preflight source disagreement before staging |
| `SPX-I208` | evidence-file open, inspect, read, or UTF-8 failure |

Existing Patch, Review, source-snapshot, A0 lock/stage, and final-check
diagnostics remain in force. The tool performs no source write and creates no
stage before successful replay; any noncooperating external source mutation is
preserved rather than overwritten. Failures after staging retain A0's cleanup
and foreign-replacement rules.

## Frozen KATs and evidence

Raw whole-artifact SHA-256 KATs (without the `sha256:` wire prefix and without
the domain-separated digest framing above) are:

| Patch schema | Capsule SHA-256 | Verification receipt SHA-256 |
| --- | --- | --- |
| v1 | `03befad24157620b56138e84d4495b1973d141275ee728493d5fbe4f0f6f09aa` | `1f2733743aaf2f9d2b9ad6bf2709a6867f169f596be01a9d53e92daecb8730a1` |
| v2 | `23742f9b8a323003237106d7a800cc8fb98f53a68bd72f5e0961cf47c63f7bba` | `6d8b13b3f54277e66a1ee501e1e71d6fe959a2ebcdbaa158a7ece20dde054e48` |
| v3 | `d682e08b125451af3ed49dce03a0814e83ca5e665224fc3bc7ab7b314827f62c` | `13a99674a4c014d9f7f315d8108c3e5c870dcac2c5950ff3035ca1a1c155361b` |

The frozen A+B generation/verification slice has 11/11 integration cases and
5/5 internal limit/invariant units. Phase C evidence-gated apply has 16/16
integration cases and 11/11 hook/limit units. Library 420/420, doctest 37/37,
full workspace, release, rustdoc, strict Clippy, formatting, diff, preservation,
and independent security gates are locally green. The exact
`34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted green
in [run
31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
including [Ubuntu job
93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
all 12 jobs passed. The earlier `e04c2c9` run failed only the Rust 1.97
`collapsible_match` lint and is not green evidence. Earlier Review, Impact,
Repair, and Patch hosted runs prove preservation only; the replacement run is
the hosted evidence for this new evidence-gated route.

## Exact nonclaims

The capsule and receipt carry this ordered array verbatim:

```text
not_signature_or_authenticated_provenance
not_human_approval_or_policy
not_safe_compatible_or_target_verified
no_commit_authority
no_reusable_authorization_token
no_test_or_target_execution
no_agent_context_or_repository_analysis
no_multi_file_transaction
no_general_proof_system
no_semantic_impact_v3
no_persistence_or_incrementality
no_external_consumer_compatibility
no_new_patch_repair_graph_cleanup_or_runtime_semantics
```

This slice adds no signature, MAC, theorem prover, SMT proof, trusted
provenance, human approval, test/target execution, general capability/unsafe/
ABI proof, Agent Context, repository analysis, multi-file transaction,
incremental or persistent index, consumer-compatibility guarantee, repair or
Patch operation, Graph/CleanupPlan schema or semantic-shape widening, or
backend/runtime semantic change. It does not remove A0's nonclaims for
predictable-name collision or stale-lock denial of service, crash-left locks,
the trusted final-directory window, parent-directory sync/power-loss
durability, or platform file-identity limits.

Review v1 is unchanged byte-for-byte: its public surface remains read-only
`review::preview`, it has no `review::verify`, and its v1/v2/v3 whole-report
KATs remain
`054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946`,
`37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421`, and
`081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544`.
The Review report itself remains non-proof; only the separate Evidence v1
capsule is the bounded proof carrier.

The additive [Target Evidence v1](SEMANTIC-TARGET-EVIDENCE-V1.md) and
[Semantic Patch Evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md) contracts do not
change any Evidence v1 artifact, KAT, command, API, or nonclaim. V2 opts into a
separate target-report binding; v1 remains target-execution-free and immutable.

The separate [Semantic Workspace Transaction
v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md) does not accept or emit Evidence v1
capsules and adds no workspace proof/provenance binding. This document's
single-file, no-multi-file artifact claim remains exact.
