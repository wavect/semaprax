# Release signing and provenance policy v1

Status: versioned trusted-identity policy and schema contract. The keyless
workflow path is configured; no SEMAPRAX release is signed today, and only a
completed hosted tag run with immutable assets can change that status.

Audience: maintainers, release engineers, and security reviewers.

## Scope and current state

[Issue #168](https://github.com/wavect/semaprax/issues/168) tracks signed,
machine-verifiable tagged releases. Wavect GmbH authorized a v0.6.0 tag gate,
but that gate has not produced a qualifying release. Generated code and the
compiler have no signing or publication authority. Only the configured GitHub
`publish-release` job can receive a short-lived OIDC token. Local work covers
the provenance format, trusted identity, binding checks, and replay against an
explicit historical trusted-root snapshot. Hosted signing and human review
remain separate steps.

**The release archives remain unsigned.** `docs/RELEASE-PROCESS.md`'s
nonclaims correctly still say so, and this document must not be read as
superseding that until the checklist below is actually completed and a real
signed release has shipped.

## Threat model

| Threat | What defends against it today | What remains open |
| --- | --- | --- |
| **Artifact substitution** (a downloaded archive differs from what CI built) | `scripts/release-manifest.py` binds each archive's exact SHA-256 digest; `src/release_provenance.rs` independently re-hashes archive bytes against the manifest ([`verify_manifest_artifacts_on_disk`]) | No signature over the manifest exists yet, so a compromised mirror could still substitute a manifest *and* its archives together |
| **Tag movement** (a tag is force-moved to a different commit after release) | `docs/RELEASE-PROCESS.md`'s "Never move or recreate a published release tag" rule; Git tag objects are content-addressed | This is a process rule, not a cryptographic one; nothing here detects a force-pushed tag after the fact |
| **Compromised workflow** (a modified `ci.yml` builds from unexpected inputs or an unapproved ref) | The identity policy below binds a claimed signature to an exact `issuer`/`subject`/`workflow_ref`, checked by [`verify_signature_claim_binds_provenance`] | No real signature exists to carry that identity yet; a compromised workflow could still forge a structurally valid but unsigned claim, which is exactly why this module never treats claim validity as proof |
| **Compromised maintainer account** (a valid GitHub credential publishes an unreviewed release) | `release-gate`'s required-check aggregation (`tests/offline_package/ci_release_gate.rs`) still must pass before `publish-release` runs; the configured keyless signing path is scoped to the workflow identity rather than a personal signing key | No qualifying hosted tag run has produced the immutable signed asset set, so the configured mitigation has no release evidence yet |
| **Stale or revoked identity** (a signature claims an identity that was valid in the past but has since been revoked) | The identity policy is a single versioned table (this document), not per-signature configuration; the offline verifier checks the certificate and transparency evidence against the exact imported trusted-root snapshot | Offline replay has no ambient network or current revocation feed. It establishes validity under that historical root snapshot, not that the identity or key remains trusted today |
| **Mirror or download corruption** (bit rot, a lossy proxy, an incomplete download) | SHA-256 digests already catch this (`docs/RELEASE-PROCESS.md`'s existing nonclaims) | Unchanged by this document |
| **Replayed provenance** (an old, validly-signed provenance/signature pair is presented alongside a newer release's artifacts) | [`verify_signature_claim_binds_provenance`]'s `subject_digest` is a byte-exact digest of the *exact* provenance document under test; a claim computed over a different version's provenance bytes cannot match | None identified beyond digest binding, which is sufficient here because there is no shared key material across versions to replay |
| **Mutable manifest signed too early** (signing an inventory before the final artifact set is known) | `scripts/release-manifest.py` is built only after every target archive exists (`collect_artifacts` fails closed on a missing target); a provenance document's `manifest_digest` binds to that exact, already-complete manifest; the source-locked workflow contract checks the signing order | A real hosted tag run and immutable published assets are still required to show that the configured path executed as specified |

## Trusted identity policy v1

These three values are the single source of truth for "which identity is
trusted to have built a SEMAPRAX release." They are enforced, byte-for-byte,
by `src/release_provenance.rs`'s `TRUSTED_ISSUER`/`TRUSTED_REPOSITORY`/
`TRUSTED_WORKFLOW_PATH` constants and by the equivalently named constants in
`scripts/release-provenance.py`; `tests/offline_package/release_provenance.rs`
cross-checks all three (code, script, and this table) agree.

| Field | Value | Meaning |
| --- | --- | --- |
| Trusted OIDC issuer | `https://token.actions.githubusercontent.com` | The only accepted token issuer for a keyless (Sigstore/Fulcio) identity -- GitHub Actions' own OIDC provider. A claim from any other issuer is rejected regardless of its other fields. |
| Trusted repository | `wavect/semaprax` | The only accepted GitHub repository slug. |
| Trusted workflow path | `.github/workflows/ci.yml` | The only workflow file whose run may claim to have produced a release. |
| Immutable OIDC subject prefix | `repo:wavect@47505194/semaprax@1326961553` | Pins the GitHub owner and repository IDs for this repository's current immutable-subject configuration; a rename/transfer requires an explicit policy revision. |
| Policy owner | Wavect GmbH release maintainers | Own the workflow, identity rotation and revocation decision; a coding agent cannot change trust roots or publish a release on its own. |
| Tag pattern | `refs/tags/v<MAJOR>.<MINOR>.<PATCH>` | Bound per-release, not globally: verification always compares against the *exact* tag the provenance statement under test itself declares (its `tag` field), not a wildcard. This is what makes "replayed provenance from another version" fail: the expected identity subject is recomputed from the tag under test, so a claim minted for `v0.4.1` cannot satisfy a check against `v0.4.2`. |

The expected GitHub OIDC **`sub` claim** for a release built from tag
`vX.Y.Z` is exactly:

```
repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/vX.Y.Z
```

The Fulcio certificate identity used by `cosign verify-blob` is the workflow
URL SAN, exactly:

```
https://github.com/wavect/semaprax/.github/workflows/ci.yml@refs/tags/vX.Y.Z
```

An independent `cosign verify-blob` invocation must pin
`--certificate-identity "https://github.com/wavect/semaprax/.github/workflows/ci.yml@refs/tags/vX.Y.Z"`;
that URL-SAN check alone does not replace this verifier's immutable OIDC
subject and repository/owner ID extension checks.

The corresponding provenance **workflow reference** (the same identity
without the URL scheme and host) is exactly:

```
wavect/semaprax/.github/workflows/ci.yml@refs/tags/vX.Y.Z
```

The immutable `sub` prefix is the repository's currently configured GitHub
OIDC policy (`use_immutable_subject: true`), not an inferred name-only subject.
The OIDC `sub` and Fulcio certificate URL SAN are deliberately distinct. The
structural signature claim records the expected `sub`. After verifying the
signature and certificate chain, the offline verifier checks the Fulcio leaf's
embedded original-token subject (OID `1.3.6.1.4.1.57264.1.24`) and immutable
repository/owner IDs (OIDs `.15` and `.17`) against this policy, as well as the
certificate URL SAN and issuer. The structural claim is not itself evidence;
all three extension values must be present exactly once in the verified leaf.

### Rotation and revocation

Because the policy is exactly this one table, not per-signature
configuration or a certificate transparency log this repository maintains,
rotation and revocation are document edits:

- **Repository or workflow path change** (e.g. a rename or workflow-file
  move): update `TRUSTED_REPOSITORY`/`TRUSTED_WORKFLOW_PATH` in
  `src/release_provenance.rs` and `scripts/release-provenance.py` and this
  table together, in the same commit, so the cross-check test in
  `tests/offline_package/release_provenance.rs` keeps them from drifting.
  Every release signed under the old identity remains verifiable against a
  policy version pinned to the commit that produced it (this is why the
  policy is versioned, not floating).
- **Suspected compromise of the workflow's OIDC identity**: there is no
  long-lived key to revoke in a keyless model -- each certificate is
  short-lived (Sigstore's Fulcio certificates are valid for minutes). The
  actionable response is to rotate the *workflow* (disable the compromised
  `ci.yml` run, audit and fix it, and require every subsequent release to be
  re-verified against the corrected workflow's identity) rather than to
  revoke a key.
- **A specific past release later found compromised**: record the finding in
  `docs/RELEASE-PROCESS.md`'s dated evidence section for that version (never
  silently delete or rewrite history), and do not backdate a "signed" claim
  onto it -- see "Explicitly out of scope" in issue #168.

## Schemas

### `semaprax.release-provenance.v1`

Built by `scripts/release-provenance.py` from an already-built
`semaprax.release-manifest.v1` (#167). Never restates the manifest's
version/tag/commit/prerelease/required_checks/artifacts fields independently
-- it copies them verbatim and additionally records a byte-exact
`manifest_digest`, so a verifier can catch a single-byte edit to the
manifest even if the edit reparses to the same JSON value.

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | string | Always `semaprax.release-provenance.v1`. |
| `version` / `tag` / `commit` / `prerelease` | as in the manifest | Copied verbatim from the manifest; must agree exactly. |
| `required_checks` | array of strings | Copied verbatim from the manifest's required-check inventory. |
| `artifacts` | array of `{name, platform, size, digest}` | Copied verbatim from the manifest's artifact inventory. |
| `manifest_digest` | `sha256:<64 lowercase hex>` | Digest of the exact manifest bytes this document was built from. |
| `source.repository` / `source.commit` / `source.tag` | strings | The exact source commit and tag; `repository` must equal the trusted repository. |
| `builder.workflow_identity` | string | `<repository>/<workflow path>@refs/tags/<tag>` -- what the builder claims produced this release. Checked against the trusted identity policy at build time (the script itself rejects a mismatch) and again at verification time (bound to the claim's identity). |
| `builder.run_id` / `builder.run_attempt` | strings | The workflow run that produced this document, for operator traceability. Not independently verified -- see nonclaims. |
| `toolchain.rustc_version` | string | The Rust compiler version used to build the release binaries. |
| `toolchain.cargo_locked` | boolean | Always `true`: every packaging command in `docs/RELEASE-PROCESS.md` uses `--locked`. |
| `build_host_class` | string | The admitted hosted runner that generated this aggregate provenance document, not a claim that every listed archive was built on that host. Each archive's separate GitHub attestation carries its own matrix builder identity. One of `github-hosted-ubuntu-24.04`, `github-hosted-ubuntu-24.04-arm`, `github-hosted-macos-15`, `github-hosted-macos-15-intel`, `github-hosted-windows-2025` from `docs/RELEASE-PROCESS.md`. |
| `nonclaims` | array of strings | What this document does not assert (see below); never empty. |

This document is generated **before** publication (like the manifest it
extends) and therefore cannot itself record a real publication timestamp or
event. The actual GitHub Release publication event remains recorded only in
`docs/RELEASE-PROCESS.md`'s dated `## X.Y.Z hosted release evidence`
section, written by hand after a real Release is confirmed published --
never inferred from this document alone.

### `semaprax.release-signature-claim.v1`

`scripts/release-signature-claim.py` deterministically projects this document
from the exact final provenance bytes and a structurally complete `cosign
sign-blob` v0.3 message-signature bundle. It derives the subject digest and pinned per-tag
identity itself, and copies only the bundle's canonical base64 signature and
certificate encodings. It does **not** sign, verify a signature, contact a
transparency log, select/download a trust root, read a CI environment
variable, or publish. The independent Rust consumer still parses the complete
closed bundle framing before a caller-supplied offline verifier receives it.
The current workflow invokes this builder and publishes a claim/root only on a
qualifying tag after its release gate. No such signed hosted release has yet
been accepted; the script alone remains no signature or publication evidence.

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | string | Always `semaprax.release-signature-claim.v1`. |
| `subject_digest` | `sha256:<64 lowercase hex>` | Digest of the exact `semaprax.release-provenance.v1` document bytes this claim signs. |
| `subject_name` | string | Human-readable label for the subject (e.g. `release-provenance.json`). |
| `identity.issuer` | string | Must equal the trusted OIDC issuer. |
| `identity.subject` | string | Must equal the pinned immutable OIDC subject prefix plus `:ref:refs/tags/<tag>` for the exact tag the provenance document declares. This is a policy expectation, not an independently extracted certificate fact. |
| `identity.workflow_ref` | string | Must equal `<trusted repository>/<trusted workflow path>@refs/tags/<tag>`, and must also agree with the provenance document's own `builder.workflow_identity`. |
| `algorithm` | string | One recognized value (currently only `sigstore-cosign-bundle-v0.3`); recognizing a value here is a structural admission, not a cryptographic endorsement. |
| `signature` | string (opaque) | A deterministic projection copied from the bundle and checked byte-for-byte against it. Cryptographic verification consumes the canonical bundle field rather than treating this duplicate string as another signature. |
| `certificate` | string (opaque) | Same projection rule as `signature`; certificate parsing and chain verification consume the canonical bundle. |

### Bundled offline verification material (narrow v0.3 slice)

The tag workflow publishes two intentionally different, exact Sigstore bundle
shapes. They are not interchangeable and a verifier rejects either shape where
the other is expected.

- `release-attestation-<target>.json` is the GitHub
  `actions/attest-build-provenance` output: one
  `application/vnd.dev.sigstore.bundle.v0.3+json` **DSSE** envelope, carrying
  one `application/vnd.in-toto+json` in-toto Statement v1 whose one SLSA
  provenance v1 `subject` names that target archive and its lowercase SHA-256
  digest. Its predicate is the closed GitHub workflow-v1 producer snapshot:
  `buildDefinition` has the exact workflow build type, workflow external
  parameters, GitHub internal parameters, and one through eight git-commit
  dependencies; `runDetails` has exactly builder ID and invocation ID. The
  aggregate verifier compares the subject's name and digest to the manifest
  and to the actual archive bytes; an attestation for one target cannot be
  replayed for another.
- `release-provenance.bundle` is the `cosign sign-blob` v0.3
  **`messageSignature`** bundle over the final `release-provenance.json`.
  A `semaprax.release-signature-claim.v1` consumes it exactly: the bundle's
  SHA2-256 message digest must be the digest of the exact provenance bytes,
  and the claim's opaque `signature` and `certificate` strings must be the
  exact strings in that bundle. A claim is therefore not an independently
  editable second signature representation.

The claim builder can be replayed without a signing capability:

```sh
python3 scripts/release-signature-claim.py \
  --provenance dist/release-provenance.json \
  --bundle dist/release-provenance.bundle \
  --output dist/release-signature-claim.json
python3 scripts/release-signature-claim.py \
  --provenance dist/release-provenance.json \
  --bundle dist/release-provenance.bundle \
  --check dist/release-signature-claim.json
```

The second command requires a byte-exact deterministic rendering. A changed
provenance byte, a replayed bundle message digest, non-canonical copied base64,
or a claim serialization drift fails closed. This replay is only preparation
for the subsequent explicit cryptographic verifier; it is not itself a
cryptographic result.
The builder reads at most 4 MiB of provenance, 2 MiB of bundle material, and
64 KiB for an existing claim under `--check`, so a hostile replay path cannot
request an unbounded allocation. It rejects duplicate JSON keys. Output uses a
same-directory temporary file followed by replacement; the caller still owns
and must trust the selected parent directory. The builder validates only the
identity/digest and closed material projection it consumes; the independent
Rust decoder remains responsible for the complete provenance and Rekor-entry
contract before any cryptographic verifier is invoked.

The parser admits one through eight fully shaped v0.3 Rekor entries for the
content kind being consumed: `hashedrekord` for the message-signature bundle and
`dsse` for an archive attestation. Every such entry has canonical decimal
indexes/times, the v0.0.1 kind/version pair, a signed-entry timestamp,
inclusion proof, and canonicalized body. `logId.keyId`, the signed-entry
timestamp, proof root/path hashes, and canonicalized body are canonical padded
standard base64. `timestampVerificationData` is either the exact empty object
(the protobuf JSON representation of zero RFC3161 timestamps) or one through
eight exact `rfc3161Timestamps` records whose `signedTimestamp` fields are the same
canonical base64. Thus an empty object never stands in for an unparsed
timestamp record, and an empty `{}` tlog entry never admits.

The certificate, `messageSignature.messageDigest.digest`,
`messageSignature.signature`, DSSE payload/`sig`, and all admitted tlog and
RFC3161 byte fields are canonical padded standard base64, not merely non-empty
strings. The narrow parser rejects URL-safe, unpadded, non-canonical, or
malformed alternatives before passing the exact bundle bytes to an external
verifier.

The admitted decimal JSON strings for every transparency-log index, integrated
time, and proof tree size are canonical nonnegative signed-64-bit values
(`0` through `9223372036854775807`); the parser rejects a sign, leading zero,
or overflow. Predicate parsing establishes that one producer snapshot is
well-formed, not that its workflow, commit, IDs, builder, or invocation is
cryptographically trustworthy or semantically bound to this release. Those
claims remain for the offline capability and its explicit identity.

Offline consumers import the exact `trusted_root.jsonl` emitted by
`gh attestation trusted-root` alongside the archive/bundle, before crossing
the air gap. The package is bounded UTF-8 JSON Lines with no empty record and
is supplied by the caller as exact bytes; this repository never downloads,
updates, or silently selects a trust root. GitHub recommends refreshing that
root whenever new signed material is imported, because a stale offline root
does not learn later key revocation or rotation.

The parser deliberately admits only those v0.3, single-signature,
single-archive-subject forms. It is not a general Sigstore, DSSE, in-toto,
SLSA, X.509, certificate-chain, or Rekor client. The complete cryptographic
replay (signature, certificate identity/chain, Rekor inclusion proof, and
imported trusted-root relationship) is implemented by
`SigstoreOfflineVerifier`, which satisfies the explicit
`OfflineBundleVerificationCapability` boundary. An embedding caller may still
inject another implementation. Either verifier receives the exact subject,
bundle, and root bytes only after all structural and digest bindings pass; it
receives no filesystem, process, network, signing, or publication authority.

## The verification module

`src/release_provenance.rs` (tested by its own `#[cfg(test)] mod tests` and
by `tests/offline_package/release_provenance.rs`) provides:

- `parse_manifest` / `parse_provenance` / `parse_signature_claim`: independent,
  from-bytes structural validation of each schema (exact key sets, closed
  vocabularies, `sha256:`/40-hex wire forms). Every one of these fails closed
  on a malformed, wrong-schema, or incomplete document.
- `verify_provenance_binds_manifest`: the provenance's `manifest_digest`,
  version, tag, commit, prerelease flag, required checks, and artifact
  inventory must all agree exactly with a manifest's actual on-disk bytes.
- `verify_manifest_artifacts_on_disk`: independently re-hashes every artifact
  a manifest names from a caller-supplied directory's real bytes; this is a
  from-scratch replay, not a re-use of `release-manifest.py`'s own build-time
  check.
- `verify_signature_claim_binds_provenance`: the claim's `subject_digest`
  must equal a byte-exact digest of the exact provenance bytes under test,
  and its `identity` fields must equal both the trusted policy above and the
  provenance document's own recorded builder identity.
- `verify_release_binding`: the two binding checks composed, for a single
  entry point over a manifest/provenance/claim triple.
- `SignatureVerificationCapability` / `verify_release_binding_with_capability`:
  a legacy format-neutral extension point for a caller-supplied verifier. It
  remains separate from the release-specific Sigstore bundle capability below
  and creates no key or identity material. It composes with the binding checks
  (which still run first and fail closed on their own) rather than duplicating
  them. This is the reusable surface #195 (signed package registry) and #209
  (signed audit capsule) can implement without redefining what "verify a
  signature claim" means. Its tests use a throwaway HMAC-SHA256 key generated
  inside the test module
  (`src/release_provenance/tests.rs`) to prove the interface actually gates
  on cryptographic verification and is not a no-op -- HMAC is a symmetric
  stand-in for that generic wiring only, never the release algorithm.
- `parse_sigstore_archive_attestation_bundle` /
  `verify_archive_attestation_binds_manifest` /
  `verify_archive_attestation_binds_release`: bounded, closed replay of the
  published archive DSSE bundle's one SLSA subject against the exact manifest
  entry and archive bytes. Aggregate verification additionally binds the
  predicate's GitHub repository URL, workflow path, tag ref, and one resolved
  source dependency commit to the exact provenance statement before handing
  any bytes to a cryptographic capability. It does not verify the predicate's
  signature or other semantics.
- `parse_sigstore_message_signature_bundle` /
  `verify_signature_claim_consumes_sigstore_bundle`: bounded, closed replay
  of the distinct `cosign sign-blob` message-signature bundle and exact claim
  consumption. A changed bundle digest, signature, certificate, or provenance
  byte rejects before any cryptographic capability runs.
- `parse_sigstore_trusted_root_jsonl` plus
  `OfflineBundleVerificationCapability`: accept caller-imported, exact,
  bounded root-package bytes and hand them with the exact subject/bundle bytes
  to a pure explicit verifier. `SigstoreOfflineVerifier` is the built-in
  implementation: it performs Sigstore v0.3 certificate-chain, identity,
  payload-signature, transparency-log, checkpoint, inclusion-proof, and
  signed-time verification against only those supplied root bytes. It never
  downloads or refreshes a root. `SPX-Z707` is the stable refusal for a bundle
  or root that cannot satisfy that cryptographic policy.
- `verify_offline_release_with_capability`: the only **aggregate** offline
  release API. It requires the manifest/provenance/claim/message bundle,
  one exact trusted-root package, and exactly one archive plus one DSSE
  attestation per manifest entry. Missing, duplicate, or extra archive names
  reject. It completes every structural, digest, claim, root-framing, and
  inventory check before the first capability call, then invokes that
  capability over provenance followed by archives in canonical manifest order.
  Every call receives the exact input bytes and an
  `ExpectedReleaseIdentity` derived from the bound trusted issuer,
  repository, workflow path, and exact tag.
  `aggregate_release_actually_invokes_the_built_in_sigstore_verifier`
  (`src/release_provenance/tests.rs`) is the differential proof that this
  function's `SigstoreOfflineVerifier` branch is a real engine and not dead
  code: the identical fixture that a caller-supplied capability accepts --
  proving every structural and binding gate already passed on its own --
  still fails as `SPX-Z707` under the built-in verifier, because only its
  signature, certificate, and transparency-log bytes are fabricated rather
  than really signed.

`verify_archive_attestation_with_offline_capability` and
`verify_signature_claim_with_offline_capability` remain explicitly partial
helpers for an already-selected subject. They are not a complete release
inventory check; callers verifying a downloaded release use the aggregate API.

### The one documented command

`semaprax release verify <release-dir>` is the single command a downloader
runs over an unpacked release directory. It is a thin front over the module
above and adds no verification of its own: it reads
`release-manifest.json`, `release-provenance.json`, and -- if present --
`release-signature-claim.json` from that directory, then hands their exact
bytes to `verify_provenance_binds_manifest`,
the CLI adapter's held no-follow archive reader, and
`verify_signature_claim_binds_provenance`. Everything is re-derived from
what is on disk: the manifest digest is recomputed from the manifest's real
bytes, every archive the manifest names is re-hashed from its real bytes,
and a claim's subject digest is recomputed from the provenance statement's
real bytes. The adapter does not list the directory or reject unrelated files;
it checks only the exact regular files the admitted manifest names. Nothing a
document says about itself is trusted.

The standalone binary uses the pure `SigstoreOfflineVerifier` when the
directory presents the complete v0.3 offline inventory:
`release-provenance.bundle`, `trusted_root.jsonl`, and one
`release-attestation-<admitted-target>.json` for each of this policy's
closed archive targets (three through v0.8.0, five afterwards: the two Linux
and two macOS targets plus Windows). An embedding host may explicitly replace that default
with another `OfflineBundleVerificationCapability`; the CLI adapter still reads
the complete bounded inventory and passes its exact bytes to
`verify_offline_release_with_capability`. A partial inventory fails as a
missing document (`SPX-Z705`), malformed or inconsistent framing fails before
cryptography, and a cryptographic rejection reports `SPX-Z707`.
`release_verify_reaches_the_built_in_cryptographic_verifier_and_reports_spx_z707`
(`tests/offline_package/release_verify_cli.rs`) proves this default wiring end
to end through the actual compiled binary: a complete, self-consistent
directory whose signature/certificate/transparency-log bytes are fabricated
is rejected as `SPX-Z707`, not silently accepted as
`CRYPTOGRAPHICALLY VERIFIED OFFLINE`.

```sh
semaprax release verify dist
```

It fails closed on the first disagreement, exiting non-zero with the owning
module's stable code -- `SPX-Z701` (document shape), `SPX-Z702` (binding:
altered manifest, provenance for another commit or tag, replayed claim),
`SPX-Z703` (identity policy: unapproved issuer, repository, or workflow),
`SPX-Z704` (artifact: missing, resized, or substituted archive) -- plus its
own `SPX-Z705` when the directory presents no readable document at all, and
`SPX-Z707` when cryptographic Sigstore verification refuses a bundle or
trusted-root snapshot. It
opens only the exact paths the manifest names, lists no directory, touches
no network, spawns no process, and never executes or unpacks an artifact.

A successful run over a complete signed-material directory prints a
cryptographically verified offline status. That means the exact held subjects
passed the pinned identity and Sigstore checks against the exact supplied
historical trusted-root bytes; it does not mean those roots are current, the
release was downloaded from an official location, or a SEMAPRAX release has
actually shipped with those assets. A successful run over a directory with no
offline bundle material prints
`status: VERIFIED UNSIGNED RELEASE`. That is a
successful verification of an **unsigned** release, never evidence that a
release was signed: no SEMAPRAX release is signed today and no signing key
or qualifying hosted keyless run exists for this repository. Verifying,
publishing, signing, and installing remain separate: this command performs
only the first.

### What verification does and does not prove

The additive R10 implementation proposal `semaprax doctor verify-release
<release-dir> --trusted-root-sha256 <64-lowercase-hex>` reuses this same held loader, aggregate verifier, diagnostic
codes, and fixed identity policy. Unlike `release verify`, it requires complete
signed material and has no unsigned fallback. The separate mandatory invocation
commitment binds the held root bytes before any other document, bundle, archive,
or verifier work. Missing/malformed commitments fail as CLI errors; mismatched
root bytes fail as `SPX-Z707`. The operator must obtain the commitment through
an independently trusted channel: a digest copied from the release directory
is not authentication, and matching hex does not prove independent provenance.
The receipt records that caller precondition, not a newly inferred trust policy.
The release directory alone can never authorize built-in doctor verification.
The existing `release verify` route remains unchanged. This is separate from ordinary
doctor tool-profile admission and from the Ed25519 doctor-install policy.
Its local positive transport test uses an explicitly injected recording
capability, labels that result as non-cryptographic, and requires the identical
fabricated signing material to fail under the built-in verifier (`SPX-Z707`).
Swapped archive attestations, stale claims, tampered archives, untrusted
identities/roots, and missing material must refuse without success output. A
working real Sigstore root fixture (calibrated with its genuine external-signer
bundle) is also refused when substituted without the independent commitment;
malformed JSON alone is not the root-substitution control.
No valid signed SEMAPRAX release is supplied by these tests; the existing
external-signer Sigstore fixture is not a substitute for the pinned release
identity. Hosted acceptance, signing identity ownership and rotation/revocation
decisions remain outstanding and unchanged.

**Does prove:** that a manifest, a provenance document, and a signature claim
name exactly the same commit, tag, version, and artifact digests; that their
exact held subjects satisfy the admitted Sigstore v0.3 signatures,
certificate-chain and pinned issuer/identity policy, transparency-log evidence,
and signed-time checks under the explicitly supplied trusted-root snapshot;
and that a claim was not lifted from a different release.

**Does not prove:** that the supplied trusted-root snapshot reflects later key
rotation or revocation; that the bytes came from GitHub or an official release
location; that a hosted SEMAPRAX release containing those bytes exists; or that
publication and installation policy were satisfied. Parsing and structural
binding still are not cryptographic verification by themselves: authenticity
is established only after `SigstoreOfflineVerifier` (or an explicitly injected
real capability) accepts every bundle. That success remains separate from
product support, reproducibility, notarization, and publication.

Also not proved by anything in this document: reproducible builds (no
cross-host byte-identical rebuild is claimed or attempted), notarization or
OS code-signing (tracked separately, see `docs/DOCTOR-SIGNED-INSTALL-V1.md`
for the one existing, narrowly-scoped Ed25519 verification path this
repository has, which is unrelated -- it authenticates a *doctor-installed
generation directory*, not a release archive), production support, or
semantic/compiler correctness.

## Hosted-release follow-up (v0.6.0 gate has a failure)

The old-head v0.6.0 tag run has [partial successful hosted jobs](RELEASE-0.6.0-STATUS.md),
but at least one job failed and others have not concluded. A source-locked
coverage repair is now on `main`, but it does not retroactively certify that
tag, a signed artifact, or publication.

The workflow now applies items 1-4 below on a qualifying tag. They are listed
as an auditable configuration contract, not as a claim that a signed release
exists. A completed hosted run must still be reviewed before the status and
historical evidence are changed.

1. **Keyless workflow authority is scoped by artifact shape.** Each
   `release-artifacts` matrix producer has the `id-token: write` and
   `attestations: write` permissions needed for the pinned
   `actions/attest-build-provenance` action to attest its own smoke-tested
   archive; its returned bundle is copied into the exact target's release
   artifact and must be present at aggregate publication. `publish-release`
   separately has `id-token: write` only for the
   Sigstore/Fulcio certificate whose URL SAN is bound to
   `https://github.com/wavect/semaprax/.github/workflows/ci.yml@refs/tags/<tag>`
   over the final aggregate provenance. The underlying GitHub OIDC `sub`
   is `repo:wavect@47505194/semaprax@1326961553:ref:refs/tags/<tag>` under
   this repository's immutable-subject configuration; it is not the
   certificate identity accepted by `cosign verify-blob`. No repository
   signing secret is configured.
2. **The signing tools are pinned.** The workflow pins both
   `actions/attest-build-provenance` and `sigstore/cosign-installer` by
   immutable action revision, and requests the declared `cosign` release
   version. The workflow contract test rejects a missing or changed pin.
3. **Attestation and signing follow the admitted subjects.** Each producer
   smoke-tests then attests its archive before upload. `publish-release` freezes
   the bounded `trusted_root.jsonl` and cryptographically checks each held
   archive against its matching held bundle, pinned repository/caller workflow,
   exact tag ref, exact source commit, and hosted-runner policy before it writes
   `SHA256SUMS`. It then runs `scripts/release-manifest.py`,
   `scripts/release-provenance.py`, and signs the resulting
   `release-provenance.json`. It never signs a manifest before the last archive
   is built and independently attested. Before `gh release create`, the
   publisher streams the exact Linux CLI member from the attested archive and
   runs `doctor verify-release` against the frozen trusted-root digest and
   complete held directory. A cryptographic/identity refusal stops publication;
   `gh release create` also fails rather than replacing existing assets.
4. **The concrete configured shape is:**
   ```sh
   gh attestation trusted-root | head -c 4194305 > dist/trusted_root.jsonl
   gh attestation verify "dist/$ARCHIVE" \
     --bundle "dist/$ATTESTATION" \
     --custom-trusted-root dist/trusted_root.jsonl \
     --repo wavect/semaprax \
     --signer-workflow wavect/semaprax/.github/workflows/ci.yml \
     --source-digest "$COMMIT" --source-ref "refs/tags/$TAG" \
     --deny-self-hosted-runners
   python3 scripts/release-manifest.py --version "$VERSION" --tag "$TAG" \
     --commit "$COMMIT" --archives-dir dist --output dist/release-manifest.json
   python3 scripts/release-provenance.py --manifest dist/release-manifest.json \
     --workflow-identity "wavect/semaprax/.github/workflows/ci.yml@refs/tags/$TAG" \
     --run-id "$GITHUB_RUN_ID" --run-attempt "$GITHUB_RUN_ATTEMPT" \
     --rustc-version "$(rustc --version)" --host-class github-hosted-ubuntu-24.04 \
     --output dist/release-provenance.json
   cosign sign-blob --yes --bundle dist/release-provenance.bundle \
     dist/release-provenance.json
   python3 scripts/release-signature-claim.py \
     --provenance dist/release-provenance.json \
     --bundle dist/release-provenance.bundle \
     --output dist/release-signature-claim.json
   python3 scripts/release-signature-claim.py \
     --provenance dist/release-provenance.json \
     --bundle dist/release-provenance.bundle \
     --check dist/release-signature-claim.json
   ```
   followed by uploading `release-manifest.json`, `release-provenance.json`,
   `release-provenance.bundle`, `release-signature-claim.json`, and
   `trusted_root.jsonl` as release assets alongside the archives (and `install.sh`/`install.ps1` when present) and
   their attestations. The source-locked CI contract requires this order and
   exact asset set. The trusted root is fetched while the publisher is online;
   later offline verification receives those exact bytes explicitly and never
   updates them through an ambient network. The pipeline caps the snapshot at
   the verifier's 4 MiB input limit while streaming it, so a remote response
   cannot grow the release workspace without bound.
5. **Publish verification instructions with the one documented command.**
   The command itself now exists: `semaprax release verify <release-dir>`
   (see "The one documented command" above) performs binding, artifact,
   identity, and cryptographic Sigstore checks offline against explicitly
   supplied root bytes. The release-process nonclaims must keep saying releases
   are unsigned until item 6 holds.
6. **Only after a real signed release has shipped**, update
   `docs/RELEASE-PROCESS.md`'s nonclaims to stop describing releases as
   unsigned, and add that release's own dated hosted-evidence section
   recording the real signature/provenance assets, exactly as its existing
   sections record archives today.
7. **Rotate or revoke only through Wavect GmbH release maintainers**: an
   identity change requires a reviewed policy revision and a new exact-ID
   verifier, with old identity acceptance explicitly retired. Offline replay
   against a historical root cannot prove current trust. This document grants
   no coding agent independent signing, publication, or policy-approval power.

Items 1-4 are configuration now present in the workflow, including the
deterministic claim and offline-root release assets; hosted publication and
its evidence review remain human-owned. The schemas, identity policy, binding verifier, and source-locked
workflow contract make a real hosted signature mechanically checkable rather
than a fact trusted only from prose. They do not substitute for that hosted
signature or its review.

## Nonclaims

Integrity, authenticity, provenance, reproducibility, and production support
remain five separate claims, exactly as issue #168 requires:

- **Integrity** (do these bytes match what was recorded?) is what SHA-256
  digests already gave this repository, and what this document's binding
  checks extend across three documents instead of one.
- **Authenticity** (were these bytes produced by the claimed identity?) is
  established for one held bundle/subject set only when the offline verifier
  accepts it under the explicitly supplied historical trusted-root bytes. No
  release is signed today, and local verifier capability is not hosted-release
  evidence.
- **Provenance** (what exactly was bound: commit, workflow, toolchain, host
  class, artifact digests) is what `semaprax.release-provenance.v1` records.
- **Reproducibility** (can a third party rebuild byte-identical artifacts)
  is not claimed, attempted, or implied by any field here.
- **Production support** is a separate, unrelated claim this document does
  not make or imply for any release, signed or not.
