# Offline Package Lock v1

- Status: versioned bounded reference; developer preview; unexecuted
- Audience: package tooling authors and compiler contributors

Offline Package Lock v1 is an additive, deterministic, read-only lock over an
explicit finite set of caller-supplied Interface Package Report v1 envelopes.
It records an exact package graph and independently replayable package facts.
It does not resolve versions, discover files, fetch packages, compile source,
run scripts, execute targets, contact a registry, mutate source, or publish a
lockfile.

## Commands and library

```text
semaprax package-lock <subject.json>... [--max-bytes N]
```

One to 64 subject paths are required. They are input handles chosen explicitly
by the caller; their spellings never enter the lock. From the already-open
handles, the CLI rejects non-regular inputs and repeated physical identities,
bounded-reads each supplied file exactly once, owns all bytes, performs no
later path rediscovery, and writes only the canonical lock plus one terminal LF
to stdout.
Wrong options or arity exit 2, domain failure exits 1 with no stdout, and no
route writes a file.

The authority-free library consumes owned subject strings rather than paths:

```rust
pub fn generate(subjects: &[String], options: &PackageLockOptions)
    -> Result<String, Vec<Diagnostic>>;

pub fn verify(
    lock: &str,
    subjects: &[String],
    options: &PackageLockOptions,
) -> Result<VerifiedPackageLock, Diagnostic>;
```

`generate` returns compact JSON without a terminal LF. `verify` strictly
parses and regenerates from the complete submitted subject set, exact-byte
compares the lock, and returns only package coordinates in canonical order.
The lock and its digest are evidence, never authority.

## Package subject

Each subject is compact canonical UTF-8 JSON with no BOM, CRLF, insignificant
whitespace, or terminal LF and JSON depth at most eight. Its wrapper schema is
`semaprax.offline-package-subject.v1`, with exact top-level order
`schema,digest,bytes,payload`. The digest is:

```text
sha256(
  "semaprax.offline-package-subject.payload.v1\0" ||
  u64_le(payload_bytes) || exact_payload_bytes
)
```

The payload has exact key order:

```text
schema,package,version,report,dependencies,capabilities,licenses,provenance
```

- `package` is a 1–255-byte canonical dotted SEMAPRAX module identity and must
  equal the authenticated Package Report `package.name` exactly.
- `version` is canonical numeric `MAJOR.MINOR.PATCH`, each component without
  leading zeroes and at most `u32::MAX`. There is no range negotiation.
- `report` has keys `schema,digest,bytes,envelope`. `envelope` is the exact
  compact Interface Package Report v1 envelope. The other three fields must
  equal that report's schema, outer payload digest, and exact envelope byte
  length. The report is independently verified, including its closed target
  matrix and embedded signature bindings.
- `dependencies` contains strict coordinate-sorted unique objects with keys
  `package,version`. Every coordinate must be present exactly once in the
  submitted set. Self edges, missing packages, version disagreement, duplicate
  edges, cycles, multiple submitted versions of one identity, and duplicate
  subjects are rejected.
- `capabilities` is a strict byte-sorted unique array. Tokens use the closed
  Build Capability Manifest v1 domains `filesystem`, `home`, `network`,
  `process`, and `secrets`, either exactly or followed by a nonempty dotted
  ASCII suffix. These are integrity-bound declared facts, not enforcement.
- `licenses` is a strict byte-sorted unique array of nonempty 1–128-byte
  printable ASCII identifiers. Empty means no license fact was supplied.
- `provenance` is strict `(kind,value)` sorted and unique. Keys are
  `kind,value`; kinds are `repository`, `revision`, `source`, or `vendor` and
  values are nonempty UTF-8 without NUL, at most 1,024 bytes. These are
  integrity-bound claims, not trusted or signed provenance.

Subjects are integrity envelopes. Re-minting a digest cannot change the
package identity away from the report module, alter the report target matrix,
introduce a foreign schema, duplicate/confuse coordinates, or make an
incomplete graph complete.

## Canonical lock

The lock wrapper schema is `semaprax.offline-package-lock.v1` and has exact
order `schema,digest,bytes,payload`. Its payload digest domain is
`semaprax.offline-package-lock.payload.v1\0`. The payload order is:

```text
schema,roots,packages,edges,target_matrix,capability_closure,
limits,budget,nonclaims
```

Roots are packages that no other submitted package depends on—equivalently,
coordinates that never appear in any subject's `dependencies` array—sorted by
coordinate. Every rendered edge is oriented `dependency -> dependent` and has
exact key order `dependency,dependent`; roots therefore have no outgoing edge
under this wire orientation.
Packages are in deterministic dependency-first topological order with
coordinate order breaking every tie. Each package carries exact coordinate,
subject digest, exact subject bytes, report schema/digest/exact-envelope digest
and bytes, exact report target matrix, direct dependencies, direct
capabilities, transitive capability closure, licenses, and provenance. A
package's capability closure is the sorted set union of its direct facts and
the complete closure of every dependency. The top-level closure is the union
of every package closure. The top-level target matrix is the exact intersection
of authenticated package-report availability facts in the frozen target order.
Edges sort by `(dependency,dependent)` and carry exact coordinates.

Independent replay rebuilds all typed facts and the complete canonical wire
from the original subject bytes. No submitted lock field is trusted as an
input to reconstruction.

## Limits

| Limit | Value |
| --- | ---: |
| subjects/packages | 64 |
| subject bytes each | 262,144 |
| total subject bytes | 4,194,304 |
| dependencies per package | 64 |
| total dependency edges | 256 |
| dependency depth | 32 |
| capabilities per package / closure | 256 |
| licenses per package | 64 |
| provenance facts per package | 64 |
| JSON depth | 8 |
| builder work units | 16,384 |
| builder bytes | 16,777,216 |
| output bytes | 8,388,608 |

Every lock carries frozen `limits` and exact `budget` objects. The limits also
record the invocation's `requested_max_bytes`, which may only narrow the frozen
8 MiB output ceiling. Count, depth, byte, and work arithmetic is checked before
growth. An exact limit succeeds; one more fails closed without partial output.

## Diagnostics

- `SPX-L401`: CLI/options/input grammar.
- `SPX-L402`: subject or lock canonical JSON/schema/shape failure.
- `SPX-L403`: subject/report digest, byte count, or exact replay failure.
- `SPX-L404`: package identity, version, duplicate, foreign dependency,
  target, capability, license, or provenance confusion.
- `SPX-L405`: dependency cycle.
- `SPX-L406`: count, depth, byte, output, builder, or work limit.
- `SPX-L407`: submitted lock does not exact-replay the subjects.
- `SPX-I215`: explicit subject open, metadata, nonregular, alias, read, or
  UTF-8 failure. Diagnostics never echo a subject path or its bytes.

## Ordered nonclaims

1. `offline_read_only_lock_evidence`
2. `not_resolver_version_negotiation_or_compatibility_engine`
3. `no_registry_network_fetch_or_filesystem_discovery`
4. `no_dependency_source_archive_or_artifact_acquisition`
5. `no_build_script_compilation_linking_or_target_execution`
6. `no_sandbox_or_capability_enforcement`
7. `capabilities_are_integrity_bound_declared_facts_only`
8. `licenses_and_provenance_are_optional_integrity_bound_claims_only`
9. `not_signature_trusted_provenance_sbom_approval_or_policy`
10. `no_source_mutation_lockfile_publication_or_commit_authority`
11. `no_path_facts_raw_tree_git_editor_or_workspace_authority`
12. `no_reusable_authorization_token`
13. `no_incremental_cache_persistence_recovery_cleanup_or_gc`
14. `no_external_consumer_compatibility_or_conformance_claim`
15. `no_new_language_graph_cleanup_backend_or_runtime_semantics`

## Evidence and status

Authored evidence covers canonical KAT/determinism, exact replay, report and
subject tamper, digest re-mint, legacy Package Report preservation, diamond and
tie ordering, cycle/self-edge, duplicate identity/version/edge, foreign and
version-mismatched dependencies, target confusion, capability closure,
optional fact preservation, and every frozen limit. This implementation and
evidence are unexecuted in this tranche. No completion-matrix status is
promoted and no hosted, supported, resolver, registry, or enforcement claim is
made.
