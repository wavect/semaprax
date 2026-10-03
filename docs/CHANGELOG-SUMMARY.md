# Changelog summary

Status: public release summary; v0.7.0 is a candidate, not published.
Audience: users and contributors who need the recent changes.

This is a quick orientation, not a feature-support claim. For exact changes, read
the [full changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md).
For implementation status and required evidence, use the
[completion matrix](COMPLETION-MATRIX.md).

## v0.7.0 candidate

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

`v0.7.0` is the current source version. Its exact release commit, tag gate, and
archives have not yet been accepted. The v0.6.0 tag gate failed, and v0.6.0
was not published. See [v0.7.0 status](RELEASE-0.7.0-STATUS.md) and the
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

## Latest available prerelease: v0.5.0

v0.5.0 added source-Agent accounting and bounded live-operation envelopes,
broader owned-data and closure support, and reliability repairs across the
supported host paths. It remains the latest downloadable prerelease. Its
[release page](https://github.com/wavect/semaprax/releases/tag/v0.5.0) lists
the available archives.

## Earlier milestones

- v0.4.1 introduced separate public-generic prerequisites and gates. The
  public generic surface remains unsupported and unpublished.
- v0.4.0 is the last accepted
  [hosted-green baseline](RELEASE-0.4.0-STATUS.md). It expanded internal owned
  data, agent workflows, Project profiles, and standard-library packages.
- v0.3.5 and v0.2.0 remain historical prereleases.

The [changelog archive](CHANGELOG-ARCHIVE.md) preserves older detailed notes.
