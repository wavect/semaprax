# Offline Deterministic Package Resolver v1

Status: proposed specification; implementation and executable evidence are not
yet authored.

Offline Deterministic Package Resolver v1 selects one integrity-bound version
per package from a finite caller-owned catalog of source-replayed subjects. It is an
authority-free planning layer above Semantic Package Report v2 and Offline
Semantic Package Lock v2. It never discovers, fetches, builds, executes,
caches, or publishes a package.

## Public schemas and API

The evidence schema is:

```text
semaprax.offline-package-resolution-evidence.v1
```

The public Rust surface is:

```text
Requirement { package: String, range: String }
ResolutionInput {
    requirements: Vec<Requirement>,
    subjects: Vec<String>,
    target: String,
    allowed_capabilities: Vec<String>,
}
ResolutionOptions { max_bytes: usize }
ResolutionOptions::new(max_bytes: usize)
    -> Result<ResolutionOptions, Diagnostic>
ResolutionOptions::default()
    == ResolutionOptions { max_bytes: 16 * 1024 * 1024 }
VerifiedResolution {
    packages: Vec<package_lock_v2::Coordinate>,
    lock: String,
}
generate(&ResolutionInput, &ResolutionOptions)
    -> Result<String, Vec<Diagnostic>>
verify(&str, &ResolutionInput, &ResolutionOptions)
    -> Result<VerifiedResolution, Diagnostic>
```

`ResolutionOptions::new` accepts 4,096 through 16,777,216 bytes inclusive.
The core API performs no filesystem or process operation. Its caller already
owns every subject byte.

## Exact version and range grammar

Versions are canonical unsigned-decimal `major.minor.patch` triples. Every
component fits `u32`; zero is written `0`; other components have no leading
zero. Prerelease, build, wildcard, comparison-list, union, and whitespace
syntax is rejected.

The only range forms are:

```text
=major.minor.patch
^major.minor.patch
~major.minor.patch
```

Exact ranges select only the named version. Tilde ranges select from the lower
bound, inclusive, to the next minor version, exclusive. Caret upper bounds are
the next major for a nonzero major, the next minor for `0.minor.patch` with a
nonzero minor, and the next patch for `0.0.patch`. A required upper-bound
increment that overflows `u32` is rejected rather than treated as unbounded.

Requirements contain 1 through 4 strictly package-sorted unique rows. Package
identities are 1 through 255 ASCII bytes from `[A-Za-z0-9._-]`. The target is
exactly `native64` or `wasm32`. The capability allowlist contains at most 256
strictly byte-sorted unique values; an empty allowlist is valid. Each capability
is 1 through 255 ASCII bytes from `[A-Za-z0-9._-]`. Input grammar and ordering
are rejected before catalog authentication.

## Catalog authentication and normalization

The catalog contains 1 through 64 exact Semantic Package Subject v2 envelopes,
at most 17 MiB each and 128 MiB cumulatively. Every subject is independently
replayed through the existing Subject-v2 parser, including the embedded
Report-v2 source replay. The report and target projection are derived from the
embedded canonical source. Version, dependency, and capability rows are
caller-authored, canonical, and integrity-bound by the subject; they are not
source-derived, signed, provenance-authenticated, or trusted publisher facts.

Catalog input order has no meaning. Subjects normalize by package identity and
numeric semantic version. Every duplicate coordinate rejects, including a
byte-identical duplicate; this keeps supplied multiplicity unambiguous. At most
32 versions may exist for one package identity. After Subject-v2 replay, the
resolver re-parses every catalog and dependency version with this specification's
strict decimal grammar before tuple ordering or range matching.

The resolver does not reinterpret Report-v2 or Lock-v2. Lock v2 adds one
crate-private integration surface:

```text
pub(crate) ResolutionSubject {
    pub(crate) coordinate: Coordinate,
    pub(crate) subject_digest: String,
    pub(crate) subject_bytes: usize,
    pub(crate) dependencies: Vec<Coordinate>,
    pub(crate) capabilities: Vec<String>,
    pub(crate) targets: BTreeMap<String, String>,
}
pub(crate) authenticate_subject_for_resolution(bytes: &str, work: &mut usize)
    -> Result<ResolutionSubject, Diagnostic>
```

The snapshot is copied only from the already replayed internal Subject-v2
value. `subject_digest` is the existing Subject-v2 payload digest used by Lock
v2 package rows. The helper shares the existing subject parser, validators,
diagnostics, and logical-work counter. It is `pub(crate)`, not public package
API. Public Lock-v2 schemas, APIs, diagnostics, and bytes remain unchanged.

## Deterministic bounded solving

The selected graph contains at most four package identities and one version
per identity, matching Lock v2. Root requirements are ranges. Subject
dependencies remain exact coordinates integrity-bound by the selected subject.

Solving uses the following fixed order:

1. choose the byte-lexicographically smallest unresolved package identity;
2. visit its matching candidates in descending numeric `(u32,u32,u32)` order;
3. apply the exact candidate check-and-charge sequence below;
4. backtrack on version conflict, missing exact dependency, target rejection,
   capability rejection, cycle, or graph limit;
5. accept the first complete graph in that order.

This produces the deterministic first complete solution under the exact
decision trace above, not a global maximum over graphs with different
transitive identities. Input ordering cannot change that trace. A candidate
already selected for an identity must satisfy every accumulated root range and
exact transitive constraint.

DFS state is transactional per candidate branch. Constraints are source-tagged
occurrences. Root tags sort first as `(root, requirement_row_index)`.
Dependency tags sort next as `(dependency, dependent_coordinate,
dependency_row_index)`, where the row index is the position in the subject's
strictly coordinate-sorted dependency vector. Every root occurrence is charged
once when normalized. Every dependency occurrence is charged once before
insertion, including an equal range/version contributed by a different source
tag. Constraint inspection is a separate one-unit charge whenever a candidate
is checked. Selected coordinates, branch constraints, selected edges, and
derived depth roll back together on backtrack. Decisions and logical work are
cumulative across the complete search and never roll back.

The solver admits at most 4,096 decisions, 8 Mi resolver logical work units,
256 dependency edges, depth 32, and the unchanged Lock-v2 graph limits. Work
charges one unit per catalog row before Subject-v2 parsing; the existing shared
subject parser then charges dependency, capability, embedded-source-byte,
export, type, and target facts into the same counter. Catalog normalization
charges one unit for every `(subject,target-key)` row, including every row of
the first normalized subject used to establish the inventory, while requiring
every subject to have the same complete target-key inventory. Subjects use
normalized coordinate order and keys use byte order. Solving charges the exact
units below. Work or decision exhaustion
aborts the entire operation as `SPX-PR505`; it is never treated as an infeasible
candidate branch. Sorting comparisons, JSON parser internals, verifier/HIR
internals, and allocation bytes are not counted. Nested Report-v2 replay keeps
its own frozen source/projection/render bounds. Final Lock-v2 generation and
verification each keep their own frozen Lock-v2 work bounds and do not debit
the resolver counter. The input byte/count bounds and a 64 MiB cumulative
resolver render/intermediate String budget apply independently. These counters
are deterministic partial logical meters, not CPU-time, allocation-byte, or
denial-of-service completeness claims.

Per candidate, checks and charges occur in this exact order: one decision;
every accumulated constraint inspection in the total tag order above; one
requested-target lookup; one per direct capability in byte order; then for each
exact dependency in coordinate order, one source-tagged constraint insertion
followed by one selected-edge insertion. Immediately after those two charges,
the exact coordinate must exist in the authenticated catalog; if its identity
is already selected, it must equal that selected coordinate. Either failure
rejects the candidate before the next dependency. Cycle, selected-package,
edge, and depth limits follow without another work charge. Only the requested target row
is charged during solving because full target-key inventory equality was
already charged and enforced during catalog normalization. Each rejected
candidate charges one final backtrack unit. A candidate that fails before a
later step never charges that later step.

`used_subjects`, `used_subject_bytes`, and `used_allowed_capabilities` describe
the admitted input. `used_selected_packages`, `used_edges`, and `used_depth`
describe only the accepted graph. `used_decisions` and `used_work_units` are
cumulative across all explored branches. The solver checks every Lock-v2 graph
predicate from the replayed snapshots. It invokes Lock-v2 generation and
verification exactly once, after a complete graph is selected. Any final
Lock-v2 generation/replay or bound failure aborts globally; it is not a
backtrackable candidate rejection.

## Target and capability policy

The requested target must exist in every selected source-replayed subject and
must be exactly `available`. `unavailable` or `unproven` rejects that branch.
An unknown requested target rejects input grammar, and complete target-key
inventory disagreement rejects catalog normalization before solving.
Projection availability is not target execution evidence.

Every selected direct and transitive subject-declared capability must appear in
the strictly sorted caller allowlist. After Lock-v2 replay, the resolver
rechecks the exact requested target row and every emitted capability closure
against that policy. The allowlist is resolution admission policy only. A
declared capability is not proof that package code uses only that authority,
and the allowlist creates no runtime, filesystem, process, network, home,
secret, tool, or build permission.

## Lock and evidence replay

After selection, the resolver delegates the exact selected subjects to
unchanged `package_lock_v2::generate` with `LockOptions::default()` (16 MiB),
then independently verifies the resulting lock with the same fixed options.
The outer `ResolutionOptions::max_bytes` never changes embedded Lock-v2 bytes.
The evidence binds:

- canonical requirements;
- target and capability allowlist;
- the sorted duplicate-free catalog set count, exact bytes, and domain-separated
  digest;
- the selected coordinate/subject digest/byte rows;
- the exact embedded Lock-v2 bytes and digest;
- frozen limits and used logical budget;
- the canonical nonclaim vector.

The wrapper order is `schema,digest,bytes,payload`. The outer digest domain is
`semaprax.offline-package-resolution-evidence.v1\0`. It hashes the domain,
little-endian `u64` payload length, and exact payload bytes. The catalog digest
domain is `semaprax.offline-package-resolution-catalog.v1\0`; its transcript is
the domain followed by the row count as little-endian `u64`, then for each
subject sorted by package plus numeric version: its exact envelope length as
little-endian `u64` and exact envelope bytes. `lock_digest` is the existing
Lock-v2 payload digest from the canonical embedded wrapper. `lock` is embedded
as raw canonical Lock-v2 JSON, not a quoted JSON string.

Payload order is
`schema,requirements,target,allowed_capabilities,catalog,selected,lock_digest,
lock_bytes,lock,limits,budget,nonclaims`. Requirement and coordinate rows use
`package,range` and `package,version` order respectively. Selected rows use
`package,version,subject_digest,subject_bytes` order and are sorted by byte-
lexicographic package identity. `VerifiedResolution.packages` uses that same
order. The embedded Lock-v2 retains its independently canonical dependency-
first package order. Catalog binding order is
`subjects,bytes,digest`. Limits order is `max_requirements,max_subjects,
max_versions_per_package,max_selected_packages,max_allowed_capabilities,max_subject_bytes,
max_total_subject_bytes,max_edges,max_depth,max_decisions,max_work_units,
max_json_depth,max_render_bytes,max_output_bytes,requested_max_bytes`. Budget order is `used_subjects,
used_subject_bytes,used_selected_packages,used_edges,used_depth,
used_decisions,used_allowed_capabilities,used_work_units`.

All objects reject missing, duplicate, and unknown keys. Integers are canonical
nonnegative decimal JSON numbers. Strings use the repository canonical JSON
quoting. Arrays use the orders above. The wrapper is compact UTF-8 with no BOM,
insignificant whitespace, CR, terminal LF, or trailing data. Parsing checks the
outer byte bound before JSON parsing and rejects nesting above 128. Rendering
uses the 64 MiB cumulative String budget before the outer output bound.

Verification authenticates and normalizes the complete supplied catalog,
re-runs the exact solver and Lock-v2 generation/replay, regenerates the entire
evidence wrapper, and byte-compares it. Self-consistent re-digest/remint or a substituted
catalog cannot bypass source replay or association checks.

The canonical nonclaims are, in order:

```text
offline_deterministic_resolution_evidence
no_registry_network_fetch_build_script_execution_cache_or_publication
capability_allowlist_is_resolution_admission_not_runtime_enforcement
target_availability_is_projection_not_execution
evidence_is_not_authority
```

Diagnostics are `SPX-PR501` options/input grammar, `PR502` subject/report
authentication, `PR503` resolution/conflict, `PR504` target/capability policy,
`PR505` bounds, `PR506` wire grammar, and `PR507` exact replay.

Nested diagnostics never escape this public surface. Subject/Report replay
failures map to `PR502`; target/capability policy failures map to `PR504`;
resolver and final Lock-v2 structural/confusion/cycle failures map to `PR503`;
all resolver or nested bounds failures map to `PR505`; malformed outer evidence
maps to `PR506`; and a final or verification-time exact replay mismatch maps to
`PR507`. The original nested message is not copied into the public diagnostic.
CLI current-directory, no-follow open, metadata, read, and file-identity
failures use the existing stable host-I/O diagnostic `SPX-I215`; CLI declared
or actual byte-bound failures use `SPX-PR505`, and invalid UTF-8 uses
`SPX-PR501`.

## CLI boundary

The additive command is:

```text
semaprax package-resolve <subject.json>... \
  --require <package>:<range> [--require ...] \
  --target <target> \
  [--allow-capability <capability>]... \
  [--max-bytes N]
```

The exact CLI grammar is 1 through 64 subject tokens, followed by 1 through 4
contiguous `--require VALUE` pairs, one `--target VALUE`, zero or more
contiguous `--allow-capability VALUE` pairs, and an optional final
`--max-bytes N`. Requirements and capabilities must already be strictly sorted
and unique. `--target` occurs exactly once; optional `--max-bytes` occurs at
most once. The repeatable options are accepted only in their named groups.
Empty tokens, subject tokens beginning with `-`, `--`, stdin, response files,
environment expansion, late flags, and unknown flags reject before opening a
subject.

An explicit relative path resolves once against the caller's current working
directory. Intermediate components follow the host's ordinary resolution and
are not authenticated. The leaf is opened atomically without following a
symlink/reparse point: Unix uses safe `rustix` open flags `NOFOLLOW|CLOEXEC`,
and Windows opens the reparse point itself and rejects its reparse attribute
before admitting the held file. Other hosts fail closed. The CLI requires a
held regular-file handle with a supported platform file identity.
Alias paths to one held identity reject. It accounts pre-read held size before
allocation and checked-adds it to the 128 MiB cumulative declared-size total;
crossing the total fails before allocating or reading that file. Each file read
is capped by the smaller of its per-file remaining limit and the cumulative
actual-byte remainder, plus one sentinel byte. Actual bytes are checked-added
to the cumulative total immediately after each read and before another file is
allocated or read. It then rechecks
held type, identity, and size after the read; short reads, growth, oversize,
identity drift, and invalid UTF-8 reject. Unsupported identity acquisition
fails closed. The held handle authenticates only the exact bytes read. It is no
publisher identity or signature and cannot prevent an uncooperative
same-principal writer from changing bytes before or during open/read. Same-size
mutation is not detected by metadata rechecks, but the resulting exact bytes
still undergo complete Subject-v2 and Report-v2 replay.

The CLI passes owned bytes to the authority-free core, performs no path
discovery beyond those explicit resolutions, and publishes no file. Success
writes one canonical evidence line plus terminal LF to stdout; stdout is not an
atomic or durable publication boundary. Usage failures exit 2 and domain
failures exit 1. V1 exposes verification only through the Rust `verify` API; it
has no evidence-verification CLI.

## Required evidence and preservation

Focused evidence must cover exact/caret/tilde boundaries (especially `0.x`),
malformed and overflowing ranges, deterministic catalog permutations,
deterministic first-feasible backtracking under the frozen trace, multi-root convergence, exact transitive
closure, missing/conflicting dependencies, duplicate-coordinate handling,
cycles, every exact/plus-one bound, available/unavailable/unproven targets,
capability allowlists, report/subject mutation and re-digested forgery, wire
mutation/truncation/insertion, exact replay, decision/work exhaustion as a
whole-operation bounds failure, and malformed CLI inputs.

CLI hostility covers atomic symlink/reparse leaf rejection, same-file aliases,
non-regular files, pre-sized oversize input, short read/growth, invalid UTF-8,
repeated/late/unknown flags, zero and 65 subjects, duplicate options, and
non-atomic stdout failure behavior. Focused preservation owners are
`src/package_lock/tests.rs`, `src/package_lock_v2/tests.rs`,
`src/package_report_v2/tests.rs`, `src/package_compatibility/tests.rs`,
`tests/offline_package_lock_v1.rs`, `tests/package_report_v1.rs`,
`tests/documentation.rs`, and existing CLI dispatch/usage evidence.

Report v1/v2, Subject/Lock v2, Lock v1, Compatibility Evidence v1, Graph
v10-v24, Project v1-v10, and every pre-existing command's accepted invocation,
stdout/stderr, and exit behavior are frozen. The additive root help/usage
listing may add only the new command line and receives its own golden.
No dependency acquisition/execution/provenance, unsafe code, build authority,
hosted promotion, registry,
publisher identity, signature, provenance, SBOM, compatibility promotion,
artifact conformance, or capability-enforcement claim follows from this
resolver.
