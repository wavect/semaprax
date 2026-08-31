# Image target and artifact projections v1

Status: authored, unrun; full programme and target execution remain unverified.
Audience: semantic agent clients, compiler contributors, and embedding hosts.

`ProjectSemanticImage::target_admission` derives actual compiler-emission facts
for the selected image's complete linked entry and test programs. It reuses the
candidate target producer, including native C11 emission and wasmparser
structural validation of Core Wasm. The selected authored function's membership
is checked against each retained role program. A whole-closure failure does not
establish that this function caused the failure. The report labels this scope
and carries the owning compiler diagnostic without claiming runtime execution,
native machine-code compilation, standalone function support or failure blame.

`artifact_projection(expected_image, ImageArtifactKind::{Web,Npm,OpenApi}, max_bytes)`
invokes the pathless Project carrier builder for the selected kind.
The existing manifest/profile decides admission; Web remains scalar Project v1,
and npm keeps its existing supported profiles. OpenAPI retains its scalar
document profile as described below. The returned carrier is
independently replayed before its schema, payload digest, exact envelope SHA256,
file paths, byte counts and individual SHA256 bindings are projected. Nothing
is installed, executed or written to a filesystem output directory.

Manifest-selected public export stable IDs link to their retained source
declarations. Every authenticated source path/digest/revision is also listed as
a Project input. These are source/manifest and carrier relationships, not
dynamic coverage, proof that every file exports every declaration, external
consumer usage, or npm installation evidence. Rust/C package carriers and
package-consumer migration remain outside this report. The additive
[OpenAPI artifact kind](IMAGE-OPENAPI-ARTIFACTS-V1.md) provides source-bound
per-module documents through the existing scalar generator and full Project
source replay, while preserving Web/npm report bytes.

The artifact build/envelope bound is host-selected within 1 KiB–16 MiB. Compact
reports are at most 1 MiB and contain no encoded artifact bodies. Existing
carrier limits and bounded compiler emitters remain active. These output and
construction bounds do not claim a total heap or wall-clock limit.

Both reports bind the exact image and Project revisions. Artifact reports also
bind the semantic graph digest, build kind and build bound.
`verify_artifact_projection` regenerates and replays the complete carrier and
requires exact report bytes. Report JSON is recursively key-sorted, compact,
and has no terminal LF. `SPX-G290` rejects selections/offsets, `SPX-G291` bounds,
`SPX-G292` unexpected compiler-carrier bindings, and `SPX-G293` report mismatch.
Existing source, target and carrier diagnostics remain unchanged where delegated.

## Protocol

Image Agent Protocol v5 adds `image/target-admission` as a `semantic_read` query.
It takes exact `image_revision`, `target`, and optional bounded UTF-8 chunk
offset/size. `candidate/build` is exposed only if the host selected both
candidate preparation and build authority. It takes a retained candidate, kind
`web`, `npm` or additive `openapi`, and chunk controls; requests cannot widen its fixed 16 MiB build
bound. The candidate's entire recovery history is independently restored before
building and replaying its pathless carrier. It returns artifact-projection
chunks, never filesystem materialization or publication authority.

Chunk envelopes identify the full report schema, exact session image and
optional candidate/target/kind selections, offset, total bytes and next offset.
Existing host source authentication surrounds queries; v5 does not change the
method sets of v1–v4. The distinction between build and artifact-materialization
authority remains explicit.

`tests/image_target_artifacts_v1.rs` authors membership, actual carrier binding,
export/source provenance, exact replay/mutation, capacity and no-write checks.
No tests, compiler gates or generated artifacts were executed during this work.

The additive [Candidate Artifact Delta v1](PROJECT-CANDIDATE-ARTIFACT-DELTA-V1.md)
compares these actual projections across a replayed candidate's original and
final revisions. It preserves this report's bytes and profile admission, and
uses the existing v5 build grant for its separate chunked comparison route.
