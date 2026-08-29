# Offline Semantic Package Lock v2

Status: additive implementation and evidence authored, unrun and unpromoted.
Audience: package tooling authors and compiler contributors.

Lock v2 is a distinct authority-free graph over at most four exact
caller-owned Semantic Package Report v2 subjects. Each subject embeds the
report as raw canonical JSON with exact byte count and domain-separated
digest; admission independently source-replays it. Generation rejects graph,
identity, version, cycle, depth, edge, byte, work, capability, and target
confusion. Capability closure is derived only from exact subjects. Ternary
target intersection preserves any `unproven` fact.

Work counters are deterministic logical traversal units, not allocator-byte or
wall-clock measurements.

The lock grants no resolver, registry, network, fetch, build, script, target
execution, enforcement, persistence, publication, or mutation authority.
Lock v1, Report v1/v2, and Graph bytes remain unchanged. Evidence is unrun.

## Public surface and canonical wire

The exact public schemas are
`semaprax.offline-semantic-package-subject.v2` and
`semaprax.offline-semantic-package-lock.v2`. The public data/API surface is:

```text
Coordinate { package: String, version: String }
LockOptions { max_bytes: usize }
LockOptions::new(max_bytes: usize) -> Result<LockOptions, Diagnostic>
LockOptions::default() == LockOptions { max_bytes: 16 * 1024 * 1024 }
VerifiedLock { packages: Vec<Coordinate> }
create_subject(&Coordinate, &str, &[Coordinate], &[String])
    -> Result<String, Vec<Diagnostic>>
generate(&[String], &LockOptions) -> Result<String, Vec<Diagnostic>>
verify(&str, &[String], &LockOptions) -> Result<VerifiedLock, Diagnostic>
```

`LockOptions::new` accepts 4,096 through 16,777,216 bytes inclusive.

Both wrappers have exact `schema,digest,bytes,payload` order. Subject payload
order is `schema,package,version,report_digest,report_bytes,report,
dependencies,capabilities`; `report` is raw canonical V2 JSON. Lock payload
order is `schema,packages,edges,target_matrix,limits,budget,nonclaims`.
Subject dependency rows have `package,version` order. Package rows have
`package,version,subject_digest,subject_bytes,report_digest,report_bytes,
revision,targets,dependencies,capabilities,capability_closure` order. Edge rows
have `dependency,dependent`, each coordinate in `package,version` order. Target
rows have `target,status`. `limits` has `max_packages,max_subject_bytes,
max_total_subject_bytes,max_dependencies,max_edges,max_depth,max_capabilities,
max_work_units,max_output_bytes,requested_max_bytes`; `budget` has
`used_packages,used_subject_bytes,used_edges,used_depth,used_work_units`.
Digests are SHA-256 over a schema-specific domain, little-endian payload
length, and exact payload bytes. Replay verifies every embedded V2 source,
rebuilds the graph/topological order, derives closures and ternary target
intersection, regenerates all bytes, then exact-compares.

Limits: 4 packages; 17 MiB per subject and 64 MiB total; 64 dependencies per
package; 256 edges and capabilities; depth 32; JSON depth 128; 8 Mi logical
work units; 16 MiB output. Work units count source bytes and selected logical
traversals, not total CPU, allocation, or every verifier-internal operation.

Diagnostics are `SPX-PL501` options, `PL502` wire, `PL503` authentication,
`PL504` graph/identity confusion, `PL505` cycle, `PL506` bounds, and `PL507`
exact replay. Capabilities are integrity-bound declarations, never enforcement.

The canonical `nonclaims` vector, in order, is:

```text
offline_source_authenticated_lock
no_resolver_registry_fetch_build_execution_or_publication
capabilities_are_integrity_bound_claims_not_enforcement
lock_is_evidence_not_authority
```
