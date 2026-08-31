# Source-bound C artifacts v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors, semantic agent clients and embedding hosts.

The C artifact kind connects candidate review to actual native C11 source and
header projections. It preserves the compiler's existing linkage, context,
status and result-publication conventions. The files are inspectable compiler
outputs, not a compiled library, standalone FFI header or supported SDK.
Canonical `.spx` source remains authoritative.

## API and source replay

`ProjectRevision::build_c_inline(max_bytes)` returns a canonical pathless
carrier `String`. `ProjectSemanticImage::artifact_projection(expected_image,
ImageArtifactKind::C, max_bytes)` summarizes the same generated artifacts.
The carrier schema is `semaprax.project-c-build.v1`.

The builder uses the exact manifest export stable IDs and retained canonical
source declarations. The ordinary native emitter receives the checked complete
linked entry program, including its manifest-selected export roots. No imported
callee is replaced with a stub, no synthetic body substitutes for source
meaning and no backend verification is bypassed.

Before returning carrier bytes, the builder reparses the canonical manifest
and rebuilds every canonical source through ordinary complete Project
admission. Revisions, the full semantic graph and exact canonical sources must
agree; native and header generation then repeat and the complete carrier must
match exactly. Submitted carrier metadata cannot become compiler authority.
This is a rebuild from retained owned source bytes, not a fresh disk read or
runtime execution.

## Artifact inventory and header admission

The carrier contains the actual native source at `native/entry.c`. Each source
with selected manifest exports contributes `c-header/<source-path>.json`, an
actual `semaprax.c-header.v1` envelope, and `c-header/<source-path>.h`, its bare
header bytes. These deterministic carrier-relative names do not authorize
filesystem creation.

The existing [C Header renderer](C-HEADER-V1.md) owns scalar admission,
exclusion reasons, comment hygiene, stable include guards and header content.
For admitted declarations, the helper extracts each prototype verbatim from
the actual Project native output. It does not infer an ABI from source types
or rewrite a static function into a public symbol.

Every selected export is explicitly admitted or excluded by header admission.
Admitted rows bind the source stable ID to its native artifact, header,
envelope, exact symbol and exact prototype. Excluded rows retain the owning
reason and the envelope containing that exclusion, with null header, symbol
and prototype mappings; they are never represented as successful
empty ABI declarations. A source can therefore yield an empty header with
explicit exclusions, preserving the existing C Header behavior. Complete native
generation must still succeed for the C carrier, even if every selected header
declaration is excluded.

The native output retains its compiler-owned context and status types, static
linkage and entry wrapper. A prototype in the header does not establish that
another translation unit can link to it. No safe wrapper, shared-library
export, platform ABI promise, installed consumer or native conformance result
is supplied by this report.

## Image and candidate review

The compact image report retains `semaprax.image-artifact-projection.v1` with
`kind: "c"`. It binds image, Project, Workspace and graph revisions; exact file
names, sizes and SHA256 values; source inputs; and actual header relationships
or exclusions. It does not embed encoded file bodies.

`ProjectCandidate::artifact_delta(expected_candidate, ImageArtifactKind::C)`
first replays the complete candidate history, then builds the base and final
C carriers through the same source-replayed path. File, export, source and
carrier changes remain separate comparisons. The verification API regenerates
the exact report. A changed prototype is an inspectable interface difference,
not proof of compatibility or incompatibility with an external application.

Existing Web, npm and OpenAPI report bytes remain unchanged. Their historical
kind-specific scope lists must not be interpreted as the platform's complete
artifact capability inventory. The C report explicitly leaves Rust carriers
and compiled C libraries outside its scope.

## Protocol and authority

V5 `candidate/build` and `candidate/artifact-delta` accept the additive `c`
kind under the existing startup candidate/build grants. Requests cannot add
authority, invoke a compiler executable, choose a filesystem path or publish
an artifact. Both methods retain exact image and candidate bindings, bounded
UTF-8 chunks and exclusion from immutable parallel-read batches.

Discovery, closed chunk schemas and generated TypeScript, Python and Rust
clients include the new kind. Earlier protocol profiles are unchanged.
Artifact generation and verification do not confer source-commit authority.

## Bounds and evidence

The build/carrier limit remains 1 KiB–16 MiB. Native emission is bounded before
its bytes are retained. Source groups, export selection, encoded artifact
bytes and complete carrier output are bounded; overflow rejects the complete
operation. Image summaries retain the 1 MiB limit and candidate deltas retain
their existing 8 MiB report and logical-work limits. These are output and
structural limits, not measured process-memory or execution-time guarantees.

The inventory permits at most 16 source groups and 32 manifest exports, hence
at most 33 files including the native source. Aggregate hexadecimal artifact
encoding is checked before retention, and the final carrier including metadata
must fit its own limit. Canonical carrier JSON has lexical object keys,
path-ordered artifacts, stable-ID-ordered exports and one terminal LF.
`payload_digest` uses `semaprax.project-c-build.payload.v1`, NUL, the
little-endian u64 payload length and canonical carrier bytes without that field
or terminal LF. File and manifest SHA256 values hash exact bytes; declaration
and header digests retain the existing C Header domains.

Existing Project, native backend and C Header diagnostics propagate. Image
bounds, binding and exact-replay failures retain their existing diagnostic
families. No rejection is replaced with an invented prototype or partial
successful native output.

Authored cases in [image/candidate evidence](../tests/image_c_artifacts_v1.rs)
and [transport evidence](../tests/image_c_artifacts_transport_v5.rs) cover
actual prototype/file correspondence, cross-file source identity, candidate
signature evolution, exact replay, exclusions, hostile inputs and build grants.
Tests, compiler checks and native execution were not run; no completion-matrix
row is promoted.

Broader C ABI/ownership admission, Rust carriers, compiled consumers, safe
wrappers, cross-package migration, artifact filesystem authority and measured
agent productivity remain outstanding.
