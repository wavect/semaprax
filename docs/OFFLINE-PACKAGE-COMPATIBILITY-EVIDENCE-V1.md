# Offline Package Compatibility Evidence v1

Status: additive focused evidence authored, unrun and unpromoted.
Audience: package tooling authors and compiler contributors.

Exact base/candidate reports must be byte-identical to selected subjects in
independently replayed Lock-v2 graphs. The closed scope is
`stable_id_semantic_compatibility_only`: stable exports, recursive types,
ownership, effects, exact ordered contracts, shared-base reachable nominal
definitions, and ternary targets. Findings are breaking, nonbreaking, or
informational. Overall indeterminate has precedence over breaking.

Unproven facts or targets, contract calls without callee closure, imported
resource ABI closure, unknown facts, or an authenticated dependency/version/
edge/capability/target/integrity context drift force indeterminate. Exact
report/subject mismatch and source-association or integrity failure instead
reject with `SPX-PC502` and produce no capsule. Source spelling and general
consumer compatibility are nonclaims. Inputs, work, findings, render bytes,
and output are strictly bounded. No package authority, resolution, fetch,
build, execution, publication, or mutation is added.

Work counters are deterministic logical traversal/source-byte units, not
allocator-byte or wall-clock measurements.

## Public surface, wire, and classification

The exact public schema is
`semaprax.offline-package-compatibility-evidence.v1`. The public data/API
surface is:

```text
CompatibilityInput {
    coordinate: package_lock_v2::Coordinate,
    report: String,
    lock: String,
    lock_subjects: Vec<String>,
}
CompatibilityOptions { max_bytes: usize }
CompatibilityOptions::new(max_bytes: usize)
    -> Result<CompatibilityOptions, Diagnostic>
CompatibilityOptions::default()
    == CompatibilityOptions { max_bytes: 8 * 1024 * 1024 }
VerifiedEvidence { outcome: String, findings: usize }
generate(&CompatibilityInput, &CompatibilityInput, &CompatibilityOptions)
    -> Result<String, Vec<Diagnostic>>
verify(&str, &CompatibilityInput, &CompatibilityInput,
       &CompatibilityOptions) -> Result<VerifiedEvidence, Diagnostic>
```

Generation creates evidence. Verification independently replays both loose V2
reports, both Lock-v2 graphs and every exact subject before regenerating and
exact-comparing the capsule. `CompatibilityOptions::new` accepts 4,096 through
16,777,216 bytes inclusive.

The wrapper order is `schema,digest,bytes,payload`. Payload order is
`schema,scope,limits,base,candidate,outcome,findings,budget,nonclaims`.
Each input binding records exact report/lock/subject-set digests and bytes.
Input binding order is `package,version,report_digest,report_bytes,lock_digest,
lock_bytes,subjects_digest,subjects_bytes`. `limits` order is `max_findings,
max_work_units,max_input_bytes,max_output_bytes,requested_max_bytes`; `budget`
order is `used_input_bytes,used_work_units,used_findings`.
Findings have exact `classification,axis,subject,before,after,reason` order and
sort canonically. Domain-separated SHA-256 binds the exact payload.

Removed exports and reachable definitions, recursive shared type/ownership
changes, added effects, exact ordered contract-vector changes, and
available-to-unavailable targets are breaking. Added exports, removed effects,
and unavailable-to-available targets are nonbreaking. Identity-backed display
renames are informational. Indeterminate outranks breaking whenever reports,
targets, contract callees, imported-resource ABI closure, or authenticated
dependency context are semantically incomplete or drifted. Authentication,
integrity, and exact source-association failures reject before classification.

Limits: 160 MiB cumulative inputs, 10 Mi logical work units, 2,048 findings,
JSON depth 64, and 16 MiB output. Work is a partial logical/source-byte meter,
not total computation. Diagnostics are `SPX-PC501` options, `PC502`
authentication/association, `PC503` limits, `PC504` wire, and `PC505` replay.

The canonical `nonclaims` vector, in order, is:

```text
not_source_spelling_or_general_consumer_compatibility
no_resolver_registry_fetch_build_execution_or_publication
unproven_unknown_or_lock_context_drift_is_indeterminate
evidence_is_not_authority
```
