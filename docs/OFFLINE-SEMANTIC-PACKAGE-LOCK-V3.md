# Offline Semantic Package Lock v3

Status: **authored, locally unrun, unpublished, and unpromoted**.

Audience: package-tool authors and compiler contributors.

Lock v3 is an additive authority-free proof over one to four caller-supplied
Semantic Package Subject-v3 envelopes. A Subject v3 authenticates an exact
Package Report-v2 envelope and a strictly package-sorted list of dependency
requirements. Each requirement uses only `=x.y.z`, `~x.y.z`, or `^x.y.z`,
where every component is canonical `u32` decimal.

The lock admits one selected version per package identity. It independently
replays every subject and report, proves that each selected dependency version
satisfies its authenticated range, rejects missing identities and cycles, and
renders dependency-first package order. Edges bind the original requirement,
the selected coordinate, and the dependent coordinate. Capability closure and
the `available` / `unavailable` / `unproven` target lattice are unchanged from
Lock v2.

Frozen bounds are four packages, 17 MiB per subject, 64 MiB total subject
bytes, 64 dependencies per package, 256 edges, depth 32, 256 capabilities, and
a 16 MiB envelope. Diagnostics occupy `SPX-PL601` through `SPX-PL607`.

## Wire and replay

Schemas are `semaprax.offline-semantic-package-subject.v3` and
`semaprax.offline-semantic-package-lock.v3`. Each digest uses its exact schema
plus NUL as the domain, then little-endian `u64` payload byte length and exact
payload UTF-8 bytes. Envelopes order `schema,digest,bytes,payload`.

Subject payload order is
`schema,package,version,report_digest,report_bytes,report,dependencies,capabilities`.
The raw `report` is never parsed/re-rendered for embedding. Its existing
Report-v2 binding domain stays unchanged. Dependency rows order `package,range`;
package identities are unique and self-dependencies reject regardless of range.

Lock payload order is `schema,packages,edges,target_matrix,limits,budget,nonclaims`.
Package rows order
`package,version,subject_digest,subject_bytes,report_digest,report_bytes,revision,targets,dependencies,capabilities,capability_closure`.
Their dependency rows order `package,range,selected_version`. Edge rows order
`requirement,selected,dependent`, where requirements are `package,range` and
coordinates are `package,version`. Edges point from selected dependency to
dependent and sort by selected coordinate, dependent coordinate, then range.
There is one version per identity, so dependency-first tie order is package-byte
order. Target aggregation gives `unproven` precedence over `unavailable` over
`available`. Cycles and target-inventory disagreement reject.

JSON depth is at most 128; logical work is at most 8 Mi units and cumulative
rendered strings are bounded at 64 MiB. These are content and logical-work
bounds, not a process heap, CPU, or complete denial-of-service proof.

## Authored evidence

`src/package_lock_v3/tests.rs` adapts the exact-coordinate hostile fixtures as
exact-range cases. `tests/offline_package_ranges_v3.rs` adds source-replayed
numeric selection, intersection/backtracking, requirement/selection binding,
raw report embedding, catalog permutation, grammar, mismatch, mutation, and
cross-input replay cases. `src/package_range.rs` owns exact/tilde/caret boundary
fixtures. These tests are authored but have not been run for this batch; the
full quality gate and v1/v2 preservation evidence remain required.

The subject and lock are integrity evidence, not authority. This contract adds
no registry, discovery, acquisition, network, cache, build, script, execution,
publication, signature, publisher, provenance, licence, or SBOM claim. It does
not alter any v1/v2 schema, byte stream, API, diagnostic, or CLI route.
