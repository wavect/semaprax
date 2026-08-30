# Source-backed Candidate Archive v1

Audience: embedding hosts, compiler maintainers, and agents recovering candidates.

Status: implementation and executable regression cases authored, **unrun**.
The user explicitly skips tests, compiler/interpreter execution, and long local
quality gates. This is not verified completion, performance, or publication
evidence.

`ProjectCandidateArchive::prepare(candidate, expected_candidate)` creates a
self-contained source-backed archive for one complete candidate. `to_json()`
returns canonical sorted compact JSON with terminal LF. `archive_digest()`,
`candidate_digest()`, and `base_revision()` expose exact identities.
`ProjectCandidateArchive::restore(bytes, expected_archive, expected_candidate)`
returns a fully reconstructed `ProjectCandidate` without opening any files.
The existing [recovery capsule](PROJECT-CANDIDATE-RECOVERY-V1.md) and its APIs
remain unchanged; this archive adds the original source bytes needed when raw
project files have changed or disappeared.

The closed `semaprax.project-candidate-archive.v1` object contains:

- compiler package/version and explicit archive compatibility identity;
- `canonical_manifest`, `base_revision`, `base_workspace_revision`, and
  `base_graph_digest` for the candidate's actual original base;
- ordered `sources`, each containing manifest-relative `path`, exact canonical
  `source`, `source_digest`, `source_revision`, and `source_graph_schema`;
- `recovery_capsule`, as the exact existing canonical capsule **string**;
- `candidate_digest`, `candidate_project_revision`, and `archive_digest`;
- `source_authority:false`, `approval_authority:false`, and `trusted_hir:false`.

No absolute project root, held file handle, serialized HIR, host permission,
approval, or live-source freshness claim is imported. Logical source paths are
validated against the canonical manifest and used only for in-memory compiler
admission; they do not select filesystem operations. Restoring a rebased
candidate uses that candidate's new base. The archive preserves the recovery
history represented by the candidate; it does not invent separate provenance
for earlier reconciliation reports or prove historic publication.

The archive digest hashes `semaprax.project-candidate-archive.payload.v1\0`,
the little-endian u64 payload byte length, and the canonical payload including
terminal LF with `archive_digest` omitted. It is content addressing, not a
signature or approval. Both expected archive and candidate digests are required.
A newly self-hashed object still requires full compiler admission and exact
replay; hashes alone never establish that a source or candidate is valid.

Restore validates selectors, raw resource bounds, closed schema/compiler facts,
canonical byte spelling, and content digest. It validates manifest source order
and bounds, then runs the ordinary complete source-backed Project builder.
Independently derived base graph, Workspace, Project, and source identities must
match. The existing recovery API then validates and sequentially applies every
change, rechecking target/profile admission and final candidate identity. Finally
the complete archive is regenerated and compared byte for byte. Wrong source
facts, changed identities, missing/extra keys, duplicate fields, alternate JSON
spelling, extra LF, unsupported compiler compatibility, and claimed authority
fail closed. Ordinary compiler/recovery diagnostics remain authoritative.

The input/output limit is 128 MiB. Before allocating a JSON Value, a raw scanner
bounds outer nesting at 16 and potential JSON nodes at 1,024. The capsule string
is separately limited to 64 MiB and enters the existing recovery preflight and
32-change limit. Source inventory inherits 16 modules and 16 MiB aggregate source
bytes; manifest and path bounds remain the ordinary Project bounds. Canonical
JSON escaping contributes to the outer cap, so a near-limit source/capsule pair
can fail archive admission even if each component fits its own limit. Serialization
uses the existing bounded writer. These are logical byte/construction bounds,
not total-memory, replay-time, or RSS promises; bounded values and replay state
may coexist during validation.

G296 covers archive grammar, canonical form, and compatibility; G297 covers
archive capacity; G298 covers selectors, digest/content, or reconstructed
identity disagreements. The archive has no store, process, network, source-write,
or commit authority. Any host-selected persistence or live-session import route
must impose its own filesystem and current-source authentication boundaries.

`tests/project_candidate_archive_v1.rs` authors deleted-source exact restoration
without file recreation, rebased history restoration after raw edits, continued
candidate editing, wrong selectors/canonical spelling/authority rejection,
self-rehashed false base/source/candidate rejection, and raw structural limits.
All are unrun in this change.
