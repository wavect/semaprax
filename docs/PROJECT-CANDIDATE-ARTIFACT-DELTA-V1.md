# Project Candidate Artifact Delta v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors and agents reviewing generated package changes.

This additive report compares actual pathless Web or npm carriers from a
candidate's original base and final admitted Project revision. It connects
candidate review to emitted file bindings and manifest-selected export
identities. It does not infer installed consumers, package compatibility or
runtime behavior from those facts.

## API and replay

```rust
pub fn artifact_delta(&self, expected_candidate: &str, kind: ImageArtifactKind)
    -> Result<String, Vec<Diagnostic>>;
pub fn verify_artifact_delta(&self, expected_candidate: &str,
    kind: ImageArtifactKind, bytes: &[u8]) -> Result<String, Vec<Diagnostic>>;
```

`ImageArtifactKind` selects `Web` or `Npm`. The report schema is
`semaprax.project-candidate-artifact-delta.v1`; verification uses
`semaprax.project-candidate-artifact-delta-verification.v1`. The exact candidate
digest and requested kind bind every invocation.

Report generation independently replays the complete candidate history and
canonical candidate evidence before constructing either carrier. It derives
base and candidate images, invokes the existing pathless builders, and uses
their independent carrier verification through
[Artifact Projection v1](IMAGE-TARGET-ARTIFACTS-V1.md). Verification repeats that
process and compares every submitted report byte. Recomputing public digests
around edited report facts cannot authenticate those edits.

Each side keeps the existing artifact projection: Project/image/graph bindings,
carrier schema and payload digest, envelope SHA256/byte length, actual file
paths, lengths and SHA256 values, selected exports and source-input provenance.
Encoded file bodies are not embedded in this review report. Existing artifact
projection and candidate/build formats remain unchanged.

`base` and `candidate` contain the complete existing projection values. `files`
and `exports` contain their sorted path/identity unions with explicit side facts,
equality flags and `added`, `removed`, `modified` or `unchanged` classification.
`comparison` distinguishes `artifact_bytes_equal`, `carrier_equal`,
`exports_equal` and `source_bindings_equal`; `inventory` counts changed and
unchanged entries. Exports use exact source-fact equality, including provenance,
not a claim that their callable interfaces are compatible.

## File and export relationships

Files are compared by their union of carrier-relative paths. Every file row
retains before/after facts, including unchanged files. Absence is explicit.
Byte equality compares actual file lengths and SHA256 bindings; carrier metadata
equality is separate. A carrier revision can change even when particular file
contents remain identical. File path ordering is deterministic and does not
change the order or content of the underlying carrier.

Exports are compared by persistent declaration identity and retain their exact
source facts. Display names or source paths do not substitute for stable IDs.
These exports belong to the selected carrier through the actual manifest and
builder invocation. The report does not invent a claim that every emitted file
exports every selected declaration or that an external application consumes it.

The complete source inventory identifies authenticated compiler inputs. It is
not runtime or test coverage. Source, export and carrier comparisons remain
separate from file-content comparisons. A preserved stable ID or equal output
digest does not establish ABI/API compatibility, behavioral equivalence,
successful installation or runtime correctness.

## Admission and authority

Existing profile admission is unchanged: Web uses the existing scalar Project
v1 pathless carrier, while npm uses its supported Project profiles. Unsupported
profile/kind combinations propagate their owning diagnostics; they are not
converted into empty successful output. Rust, C and OpenAPI package projections
remain outside this report's scope, not claimed to be absent from the platform.

No compiler executable, native compilation process, interpreter, test runner,
package manager or generated target runs. No files are installed or published.
The existing source/manifest/export invariant checks remain mandatory, and no
artifact report, verification receipt or digest grants commit authority.

## V5 build grant

`candidate/artifact-delta` requires the existing startup build grant and
candidate preparation. Candidate-only sessions cannot discover or invoke it.
Required parameters are `image_revision`, `candidate_revision` and `kind`
(`web` or `npm`); there is no target selector or request-selected build limit.

The method is classified under `candidate_build`, not ordinary semantic reads.
It authenticates live source before and after preparation, leaves the candidate
registry unchanged, and remains outside the parallel image-read batch. V1–v4
and the existing `candidate/build` route remain unchanged.

The closed `semaprax.image-artifact-delta-chunk.v1` envelope returns bounded
UTF-8 chunks with image/candidate/kind/report bindings and offsets. Optional
`offset` is 0–8 MiB and must be a boundary within the actual report;
`chunk_bytes` is 1,024–65,536, default 16,384. Source authority, artifact
materialization and target execution are explicitly false. Discovery and
generated clients describe the envelope; the heterogeneous report remains
explicitly listed as unbundled.

## Bounds and evidence

Each side uses the existing fixed 16 MiB build/envelope limit and 1 MiB compact
artifact projection limit. The delta report is capped at 8 MiB. Reports use
compact canonical UTF-8 JSON with lexical object keys, deterministic inventory
order and one terminal LF. Existing builder and carrier limits remain active;
overflow fails instead of dropping files or exports. Logical fact work is
bounded to 32 MiB, each file inventory to 64 entries, combined path/export union
to 65,536 entries, compiler-projection JSON syntax to 1,048,576 visits and
container depth to 128. These are structural and output bounds, not peak-memory
or latency guarantees.

Fact and verification digests use `semaprax.candidate-artifact-delta.fact.v1`
and `semaprax.candidate-artifact-delta.report.v1`, respectively, followed by NUL,
the little-endian u64 byte length and exact bytes. Each side's fact digest uses
canonical sorted JSON plus one LF, while the original artifact-projection wire
retains its no-LF contract. File/envelope SHA256 bindings and carrier payload
digests retain their existing owning definitions.

`SPX-G331` reports inconsistent delta facts, `SPX-G332` capacity overflow and
`SPX-G333` exact replay mismatch. Existing source, candidate, profile, target and
carrier diagnostics can propagate unchanged.

`tests/project_candidate_artifact_delta_v1.rs` owns library evidence;
`tests/image_artifact_delta_transport_v5.rs` covers build gating, discovery and
chunking. Tests are authored and unrun. No compiler/test/application executable
or long local quality gate was run for this batch; no completion row is promoted.

Broader carrier types, installed consumer relationships, cross-package migration,
artifact filesystem authority, runtime compatibility and the full
graph-operational programme remain outstanding.
