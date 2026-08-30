# Offline Deterministic Package Resolver v2

Status: **authored, locally unrun, unpublished, and unpromoted**.

Audience: package-tool authors and compiler contributors.

Resolver v2 deterministically selects Subject-v3 packages from a complete,
caller-owned finite offline catalog. Roots and transitive dependencies use the
same closed exact/tilde/caret range grammar. Search selects the byte-lowest
unresolved identity and tries numeric versions in descending order. Every
constraint occurrence remains tagged and intersected; branch selection,
constraints, edges, and depth roll back together while decision/work charges
remain cumulative. The first complete graph wins.

The successful selected set is passed once through Lock-v3 generation and
exact verification. Evidence embeds that exact lock and binds canonical roots,
target, allowed capabilities, catalog digest, selections, limits, and budget.
Target availability and declared capabilities are admission facts, not target
execution or capability enforcement.

Frozen bounds are four roots, 64 catalog subjects, 32 versions per identity,
four selected identities, 128 MiB catalog bytes, 256 edges, depth 32, 4,096
decisions, 8 Mi work units, and a 16 MiB envelope. Diagnostics occupy
`SPX-PR601` through `SPX-PR607`.

## Canonical contract

The evidence schema is `semaprax.offline-package-resolution-evidence.v2`.
Evidence and catalog domains are respectively that schema plus NUL and
`semaprax.offline-package-resolution-catalog.v2` plus NUL. Lengths are
little-endian `u64` before exact bytes. The catalog preimage retains its frozen
count and per-subject length framing in numeric coordinate order, preserving
each subject's exact raw bytes.

Envelope order is `schema,digest,bytes,payload`. Payload order is
`schema,requirements,target,allowed_capabilities,catalog,selected,lock_digest,lock_bytes,lock,limits,budget,nonclaims`.
Root rows order `package,range`; selected rows order
`package,version,subject_digest,subject_bytes`. The embedded Lock-v3 bytes are
sliced structurally and replayed exactly, never normalized through a JSON value.

Every candidate decision, visited constraint, target lookup, capability check,
dependency-constraint insertion, edge insertion, and rejected-candidate
backtrack is charged in fixed order. Roots tag constraints first by row;
dependency tags follow by dependent coordinate then dependency row. Graph
state is identity-only until the successful Lock-v3 invocation binds versions.
That final lock failure is global, never another solver branch. The exact lock
is also rechecked for target and transitive declared-capability policy.

The requested envelope budget is 4 KiB through 16 MiB, each subject is at most
17 MiB, JSON depth is at most 128, and cumulative rendered strings are bounded
at 64 MiB. Nested Report and Lock bounds remain independent. Logical work is
not a complete CPU or heap bound.

## Authored evidence

`src/package_resolver_v2/tests.rs` retains the v1-shaped hostile wire, remint,
nested-bound, catalog, policy, and exact-coordinate fixtures using Subject-v3
exact ranges. `tests/offline_package_ranges_v3.rs` adds genuinely ranged
dependencies, numeric ordering, later-root intersection and rollback, exact
Lock-v3/raw-report binding, permutation, and cross-input rejection. These tests
are authored and locally unrun; no completion row is promoted by their presence.

This is not general SemVer and provides no prereleases, build metadata,
wildcards, unions, registry, network, fetch, cache, persistence, acquisition,
build, scripts, execution, publication, compatibility judgment, or migration.
It adds no CLI route and preserves Resolver-v1 and Lock-v1/v2 exactly.
