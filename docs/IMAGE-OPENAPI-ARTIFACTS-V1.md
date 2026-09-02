# Source-bound OpenAPI artifacts v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors, semantic agent clients and embedding hosts.

OpenAPI joins Web and npm as an image artifact kind. Agents can inspect actual
generated document bindings and compare a candidate with its original source
revision without running a server, writing output files or granting publication
authority. Canonical `.spx` source remains the only program authority.

## Selection and source meaning

`ProjectSemanticImage::artifact_projection(expected_image,
ImageArtifactKind::OpenApi, max_bytes)` selects the exact manifest's public
export stable IDs. Requests cannot substitute display names, choose additional
source paths or change the manifest export inventory. Selected declarations
must belong to retained canonical Project sources.

The existing [OpenAPI generator](OPENAPI-V1.md) owns document rendering and
admission: monomorphic, effect-free functions with direct by-value Copy-scalar
parameters and a direct Copy-scalar result. Its existing diagnostic reasons
remain intact. This addition does not widen admission to nominal types,
resources, non-value parameters or generic functions.

Cross-file source meaning is checked by ordinary complete Project admission.
The generator consumes each selected source's real parsed declarations; it
does not replace imported functions with invented bodies or pretend that an
unverified standalone module is a checked Project. All canonical sources and
the manifest participate in independent Project rebuild before artifact replay.

## Actual documents and carrier

`ProjectRevision::build_openapi_inline(max_bytes)` returns the fully replayed
canonical carrier as a UTF-8 `String`. It has the same pathless ownership
boundary as the image query; retaining or writing that string elsewhere is a
separate host decision. Image and candidate review use this builder and do not
introduce a second document renderer.

Selected exports are grouped by canonical source path. Each group produces one
actual `semaprax.openapi.v1` envelope through the existing generator. Its
document describes only the selected declarations in that source. Artifact
paths use `openapi/<canonical-source-path>.json`; for example, `lib.spx` yields
`openapi/lib.spx.json`. These are carrier-relative artifact names, never paths
opened or written by the query.

The project carrier uses `semaprax.project-openapi-build.v1` and binds the
Project, Workspace and semantic graph revisions, selected exports, canonical
source provenance and exact generated artifact bytes. Individual artifacts
retain lengths and SHA256 bindings. Export relationships retain the selected
stable ID, source declaration and actual artifact/operation mapping; they do
not claim that an installed external consumer uses that operation.

Carrier replay rebuilds the complete Project from its canonical manifest and
all canonical source bytes, regenerates the documents through the shared
renderer and compares the exact carrier. Matching caller-supplied digests do
not replace source replay. No cache, document or carrier can introduce extra
program meaning.

Carrier JSON uses lexical object keys, deterministic artifact/export order and
one terminal LF. `payload_digest` hashes the canonical carrier without that
field or terminal LF, using `semaprax.project-openapi-build.payload.v1`, a NUL,
the little-endian u64 payload byte length and exact payload bytes. File and
manifest SHA256 values bind their exact bytes; nested document digests retain
the existing OpenAPI domain. These different bindings are not interchangeable.

The returned compact report keeps
`semaprax.image-artifact-projection.v1`, with `kind: "openapi"`, file inventory,
export relationships, source inputs and carrier bindings. It contains no
encoded artifact bodies. The pre-existing Web/npm projection bytes and their
admission rules remain unchanged.

## Candidate review and protocol

`ProjectCandidate::artifact_delta(expected_candidate, ImageArtifactKind::OpenApi)`
uses the same image projection on the original base and final candidate after
replaying the complete semantic intention history. The existing delta report
compares file content, carrier bindings, export facts and source provenance
separately. Its exact-byte verification reruns the whole process.

A changed source path can remove one artifact and add another while preserving
an exported stable ID. A source-only edit can change provenance or carrier
bytes without changing the callable signature. Neither equality nor a change
classification proves API compatibility, behavioral equivalence or runtime
conformance. The standalone `openapi-compat` command retains its own separate
structural compatibility contract.

V5 `candidate/build` and `candidate/artifact-delta` add `openapi` to their
closed `kind` parameter. Both still require the host's startup candidate and
build grants, retain exact image/candidate bindings and remain excluded from
immutable parallel read batches. Requests cannot widen authority or choose a
filesystem output path. Chunk schemas and generated TypeScript, Python and
Rust clients describe the additional kind. V1–v4 method sets are unchanged.

## Bounds, diagnostics and evidence

The image build/envelope limit remains 1 KiB–16 MiB and its compact report
limit remains 1 MiB. Existing OpenAPI selection and bounded-output limits also
apply: at most 32 selected exports across at most 16 source groups.
Multi-source grouping does not permit unbounded export selection. The
candidate delta retains its 8 MiB report limit and existing logical work and
inventory limits. Overflow rejects the complete operation instead of returning
partial documents or dropping selected exports.

Ordinary Project admission and OpenAPI diagnostics propagate. The existing
image `SPX-G290`–`SPX-G293` family continues to own invalid selectors, bounds,
carrier inconsistency and report mismatch. Candidate stale/replay and artifact
delta diagnostics retain their existing ownership.

Authored cases live in
[image/candidate evidence](../tests/image_protocol/openapi_artifacts_v1.rs) and
[transport evidence](../tests/image_transport_v5/openapi_artifacts.rs). They
cover source-bound cross-file selection, actual document/file relationships,
candidate changes, exact replay, hostile selectors and host-selected authority.
These cases remain unrun; no completion-matrix row is promoted.

Rust/C package projections, imported schemas, installed package consumers,
live conformance, hosting, schema migration, filesystem artifact publication
and measured agent productivity remain outstanding. This report grants none
of those capabilities.
