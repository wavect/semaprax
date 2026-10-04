# Changelog summary

Status: public release summary; v0.8.0 is a source candidate, not published.
Audience: users and contributors who need the recent changes.

This is a quick orientation, not a feature-support claim. For exact changes, read
the [full changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md).
For implementation status and required evidence, use the
[completion matrix](COMPLETION-MATRIX.md).

## v0.8.0 source candidate

- LAW-16 records pinned Bend U32 and SEMAPRAX Lean law evidence, with exact
  source identities and bounded controls. The checked-u32 cells and overall
  LAW-16 closure remain open.
- RI-13 records separate macOS arm64 and Linux x86_64 guest M1/M2/M3
  application receipts. The linked consumer and copy/friction ledgers are
  available; Linux performance is guest evidence.
- RI-08 extends the bounded generated Rust callback and owner profiles, with
  matching interpreter, native C, and Core Wasm checks for admitted shapes.
- HR-07 records macOS arm64 hot reload measurements and an installed VS Code
  Extension Host gate; the measured interpreter reload was slower than a full
  restart on its fixture.
- Current-main CI regressions in help, semantic fixtures, proof guards,
  package lockfile classification, native tool setup, and service packaging
  have focused local fixes awaiting hosted verification.

The source package version is `0.8.0`. No v0.8.0 tag or release archives are
claimed. See [v0.8.0 status](RELEASE-0.8.0-STATUS.md).

## v0.7.0 historical summary

- LAW-07 begins a bounded structured proof profile for immutable finite
  records, closed variants, and explicit match paths. Local Z3 gates prove a
  two-account transfer and reject five seeded defects; pinned Lean checks a
  record and variant match example plus a false theorem. Source-bound replay
  and selected-law attachment remain open.

- RI-07 adds checked Semaprax call routing through concrete Rust const/type
  specializations, source-mapped trait diagnostics, and fallible Serde wire
  conversion with counted rollback and explicit payload-copy metrics. Bounded
  acceptance includes non-opt-in JSON/Vec execution, rustc orphan/coherence
  refusals, field-name hygiene, and the unchanged public-generic separation
  gates. Generalized Rust trait solving and layout compatibility remain outside
  this profile.

- LAW-06 adds versioned, live-replayed checked-summary certificates for pure
  scalar calls and an installed Z3 proof route that joins selected Project and
  strict managed-Workspace laws. Version 2 records stable IDs for callee clauses,
  caller preconditions, and caller postconditions. The admitted effectful
  Project refusal and repeated-call identity controls pass; the bounded
  profile preserves runtime guards.

- The experimental native String bridge now accepts bounded UTF-8 byte input
  through an explicit C factory, with checked signed/unsigned lengths, Rust-side
  allocation, and executed malformed-input/allocation-failure controls.

- RI-05 adds a generated Rust context lifetime facade with cross-crate
  compile-fail controls and isolated Drop-panic/abort execution. Actual native
  archive and complete package bytes match a pre-RI05 capture on arm64 macOS.

- RI-05 adds authenticated owner admission before atomic argument transfer.
  CleanupPlan v14 and Graph v60 retain canonical rollback on carrier refusal,
  including two-owner and sticky cleanup-failure controls. RI-05 acceptance
  is complete for the bounded profile; the full quality profile is deferred.

- RI-05 pins frozen owned-data v1 generated-file digests to a pre-owner baseline
  and executes counted host-copy/provider-free controls, including a compiled
  skipped-copy negative, without changing the v1 runtime.

The v0.7.0 prerelease was published on October 1, 2026. Earlier failed tag
attempts are recorded separately. See
[v0.7.0 status](RELEASE-0.7.0-STATUS.md) and the
[historical v0.6.0 gate record](RELEASE-0.6.0-STATUS.md).

- The checked `endpoint Bytes` session profile connects legal protocol order
  to a unique source carrier through terminal consumption. Its recorded
  interpreter, native and Wasm tests are local evidence; broader preservation
  and integration remain pending.
- Private continued owned-Agent wait work now reaches Decision cleanup on its
  physical release path. Continued Outcome/Reduce, public execution and
  restart recovery remain unfinished.
- Local package-registry work now covers signed metadata, lock-bound artifact
  reads, held generations, and a resolver-cache bridge. It is not a hosted
  package service.
- The experimental native owner renderer now carries closed Option<String> and
  Result<String,i64> values through checked helpers and consuming imports,
  including the closed nested Result<String,Option<i64>> domain-error shape.
  Focused physical cleanup and failure controls pass; selected container
  packaging remains outside this renderer profile.
- Native law declarations now select exact Project law modules through manifest
  v2. Contract and independent scalar relational propositions have stable IDs,
  canonical graph/query projections, and explicit open coverage until verified
  evidence exists. The filename `LAWS.spx` has no discovery authority.
- Private native owned-byte and Core Wasm work gained bounded fixtures and
  observed JavaScript-arena settlement evidence. Public parity and complete
  cleanup/fuel evidence remain open.
- The release verifier checks Wavect GmbH's approved GitHub repository and
  owner identities in the signing certificate. This implementation does not
  establish a signed release before the exact tag gate passes.

## Latest available prerelease: v0.7.0

The [v0.7.0 release page](https://github.com/wavect/semaprax/releases/tag/v0.7.0)
lists archives for Linux x86-64, Apple Silicon macOS, and Windows x86-64,
alongside checksums and release provenance.

## Earlier milestones

- v0.4.1 introduced separate public-generic prerequisites and gates. The
  public generic surface remains unsupported and unpublished.
- v0.4.0 is the last accepted
  [hosted-green baseline](RELEASE-0.4.0-STATUS.md). It expanded internal owned
  data, agent workflows, Project profiles, and standard-library packages.
- v0.3.5 and v0.2.0 remain historical prereleases.

The [changelog archive](CHANGELOG-ARCHIVE.md) preserves older detailed notes.
