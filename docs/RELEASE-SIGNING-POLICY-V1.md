# Release signing and provenance policy v1

Status: versioned trusted-identity policy, schema contract, and human-owned
checklist. No SEMAPRAX release is signed today; this document defines what a
signed pipeline must satisfy and what a maintainer must still supply by hand.

Audience: maintainers, release engineers, and security reviewers.

## Scope and current state

[Issue #168](https://github.com/wavect/semaprax/issues/168) asks for
authentic signing and machine-verifiable provenance for tagged releases.
Nobody working this issue has a signing key, a keyless-signing (Sigstore)
identity, a registry credential, or authority to publish, tag, or trigger a
release workflow; per `AGENTS.md`, generated code and this repository's own
tooling gain no ambient signing authority. This document and its paired
implementation therefore split the work into what is safely buildable
without any secret -- the **provenance document**, the **identity policy**,
and **binding verification** -- and what remains a human-owned setup task,
listed in full at the end of this document.

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
| **Compromised maintainer account** (a valid GitHub credential publishes an unreviewed release) | `release-gate`'s required-check aggregation (`tests/offline_package/ci_release_gate.rs`) still must pass before `publish-release` runs | Keyless signing scoped to the *workflow* identity (not a personal account) is specifically the mitigation Sigstore/Fulcio provides for this threat, and is not wired up yet (see checklist) |
| **Stale or revoked identity** (a signature claims an identity that was valid in the past but has since been revoked) | The identity policy is a single versioned table (this document), not per-signature configuration, so revoking an identity is one document edit | No revocation list or expiry mechanism exists; Sigstore's own short-lived certificates (minutes, not the lifetime of a long-lived key) are the recommended mitigation, not built here |
| **Mirror or download corruption** (bit rot, a lossy proxy, an incomplete download) | SHA-256 digests already catch this (`docs/RELEASE-PROCESS.md`'s existing nonclaims) | Unchanged by this document |
| **Replayed provenance** (an old, validly-signed provenance/signature pair is presented alongside a newer release's artifacts) | [`verify_signature_claim_binds_provenance`]'s `subject_digest` is a byte-exact digest of the *exact* provenance document under test; a claim computed over a different version's provenance bytes cannot match | None identified beyond digest binding, which is sufficient here because there is no shared key material across versions to replay |
| **Mutable manifest signed too early** (signing an inventory before the final artifact set is known) | `scripts/release-manifest.py` is built only after every target archive exists (`collect_artifacts` fails closed on a missing target); a provenance document's `manifest_digest` binds to that exact, already-complete manifest | The checklist below states this ordering as a hard requirement for the real signing step, since nothing here can enforce workflow step ordering |

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
| Tag pattern | `refs/tags/v<MAJOR>.<MINOR>.<PATCH>` | Bound per-release, not globally: verification always compares against the *exact* tag the provenance statement under test itself declares (its `tag` field), not a wildcard. This is what makes "replayed provenance from another version" fail: the expected identity subject is recomputed from the tag under test, so a claim minted for `v0.4.1` cannot satisfy a check against `v0.4.2`. |

The expected Sigstore/Fulcio certificate **subject** for a release built from
tag `vX.Y.Z` is exactly:

```
repo:wavect/semaprax:ref:refs/tags/vX.Y.Z
```

and the expected **workflow reference** is exactly:

```
wavect/semaprax/.github/workflows/ci.yml@refs/tags/vX.Y.Z
```

Both are GitHub Actions' own standard OIDC claim shapes for a tag-triggered
workflow run using `id-token: write`; nothing here invents a new claim
format.

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
| `build_host_class` | string | One of `github-hosted-ubuntu-24.04`, `github-hosted-macos-15`, `github-hosted-windows-2025` -- the admitted hosted runners table in `docs/RELEASE-PROCESS.md`. |
| `nonclaims` | array of strings | What this document does not assert (see below); never empty. |

This document is generated **before** publication (like the manifest it
extends) and therefore cannot itself record a real publication timestamp or
event. The actual GitHub Release publication event remains recorded only in
`docs/RELEASE-PROCESS.md`'s dated `## X.Y.Z hosted release evidence`
section, written by hand after a real Release is confirmed published --
never inferred from this document alone.

### `semaprax.release-signature-claim.v1`

Not built by any script in this repository today (there is no signing key
to produce a real one). Defined here so `src/release_provenance.rs` has
something concrete to verify the *binding* of, and so the checklist below
can point at an exact target shape for the day a real signer exists.

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | string | Always `semaprax.release-signature-claim.v1`. |
| `subject_digest` | `sha256:<64 lowercase hex>` | Digest of the exact `semaprax.release-provenance.v1` document bytes this claim signs. |
| `subject_name` | string | Human-readable label for the subject (e.g. `release-provenance.json`). |
| `identity.issuer` | string | Must equal the trusted OIDC issuer. |
| `identity.subject` | string | Must equal `repo:<trusted repository>:ref:refs/tags/<tag>` for the exact tag the provenance document declares. |
| `identity.workflow_ref` | string | Must equal `<trusted repository>/<trusted workflow path>@refs/tags/<tag>`, and must also agree with the provenance document's own `builder.workflow_identity`. |
| `algorithm` | string | One recognized value (currently only `sigstore-cosign-bundle-v0.3`); recognizing a value here is a structural admission, not a cryptographic endorsement. |
| `signature` | string (opaque) | Never decoded or cryptographically verified by this repository's code -- see "What verification does and does not prove" below. |
| `certificate` | string (opaque) | Same as `signature`. |

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

### What verification does and does not prove

**Does prove:** that a manifest, a provenance document, and a signature
claim name exactly the same commit, tag, version, and artifact digests; that
none of the three has been altered by even one byte since the triple was
assembled; that a claim was not lifted from a different release (a replay);
and that a claim's declared identity matches the pinned trusted-identity
policy for the exact tag under test.

**Does not prove:** that `signature`/`certificate` bytes are a real
cryptographic signature produced by the claimed identity's private key.
Verifying that honestly requires a signature-verification implementation
(Sigstore/cosign bundle verification, or raw Ed25519/ECDSA point
arithmetic), which needs a cryptography dependency this change is not
permitted to add. **A structurally valid, well-bound claim is not the same
claim as a cryptographically authentic one** -- conflating the two here
would be exactly the mistake this repository's own session precedent (no
toy password hash, no toy MAC -- see the #191 work this issue's assignment
names) warns against, applied to signing. Closing that gap requires pairing
this module's binding check with a real external verifier, such as:

```sh
cosign verify-blob \
  --certificate-identity "repo:wavect/semaprax:ref:refs/tags/vX.Y.Z" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  --bundle release-signature.bundle \
  release-provenance.json
```

run by a real signing CI step or by a maintainer, never by this repository's
own compiler or scripts.

Also not proved by anything in this document: reproducible builds (no
cross-host byte-identical rebuild is claimed or attempted), notarization or
OS code-signing (tracked separately, see `docs/DOCTOR-SIGNED-INSTALL-V1.md`
for the one existing, narrowly-scoped Ed25519 verification path this
repository has, which is unrelated -- it authenticates a *doctor-installed
generation directory*, not a release archive), production support, or
semantic/compiler correctness.

## Human-owned checklist (`HUMAN_BLOCKED`)

Everything below requires a decision, a credential, or an infrastructure
change nobody implementing this issue has authority to make. Each item names
exactly what "done" looks like so this stays a short checklist rather than a
research project.

1. **Enable keyless signing in the release workflow.** Add `id-token: write`
   permission to the `publish-release` job in `.github/workflows/ci.yml`
   (out of scope for this change -- `.github/workflows/**` requires a
   maintainer or the coordinator) so it can mint a Sigstore/Fulcio
   certificate bound to `repo:wavect/semaprax:ref:refs/tags/<tag>`.
2. **Choose and pin the actual signing tool.** `cosign` (Sigstore) is the
   concrete recommendation this document assumes above; record the exact
   pinned version in the workflow, the same way other tool versions in this
   repository are pinned.
3. **Order signing after the artifact inventory is final, not before.**
   `publish-release` must run `scripts/release-manifest.py` (already
   documented in `docs/RELEASE-PROCESS.md`) and `scripts/release-provenance.py`
   (this change) only after every target archive exists, then sign the
   resulting `release-provenance.json` -- never sign a manifest before the
   last archive is built, and never let a rerun replace an archive while
   keeping an old signature (see the threat model row above).
4. **Wire the concrete workflow step.** The recommended shape is:
   ```sh
   python3 scripts/release-manifest.py --version "$VERSION" --tag "$TAG" \
     --commit "$COMMIT" --archives-dir dist --output dist/release-manifest.json
   python3 scripts/release-provenance.py --manifest dist/release-manifest.json \
     --workflow-identity "wavect/semaprax/.github/workflows/ci.yml@refs/tags/$TAG" \
     --run-id "$GITHUB_RUN_ID" --run-attempt "$GITHUB_RUN_ATTEMPT" \
     --rustc-version "$(rustc --version)" --host-class github-hosted-ubuntu-24.04 \
     --output dist/release-provenance.json
   cosign sign-blob --yes --bundle dist/release-provenance.bundle \
     dist/release-provenance.json
   ```
   followed by uploading `release-manifest.json`, `release-provenance.json`,
   and `release-provenance.bundle` as release assets alongside the three
   archives. This is a recommendation, not a change made here.
5. **Publish verification instructions with the one documented command.**
   Once the above exists, `docs/RELEASE-PROCESS.md` should gain a worked
   `cosign verify-blob` invocation plus a call into this repository's own
   `verify_release_binding` (through a small CLI wrapper -- adding a
   `semaprax release verify` subcommand touches `src/cli_driver.rs`, which is
   outside this change's file lease; see the final report for the exact
   recommendation).
6. **Only after a real signed release has shipped**, update
   `docs/RELEASE-PROCESS.md`'s nonclaims to stop describing releases as
   unsigned, and add that release's own dated hosted-evidence section
   recording the real signature/provenance assets, exactly as its existing
   sections record archives today.
7. **Decide and record identity rotation ownership**: who (which maintainer
   role) is authorized to edit the trusted identity policy table above, and
   what review is required before that edit merges. This document does not
   itself grant that authority to anyone.

None of items 1-7 are performed by this change. What is implemented instead
-- the schemas, the identity policy, and the binding verifier -- is what
would let a real signing pipeline, once wired up by a maintainer, be
verified mechanically rather than trusted on prose.

## Nonclaims

Integrity, authenticity, provenance, reproducibility, and production support
remain five separate claims, exactly as issue #168 requires:

- **Integrity** (do these bytes match what was recorded?) is what SHA-256
  digests already gave this repository, and what this document's binding
  checks extend across three documents instead of one.
- **Authenticity** (were these bytes produced by the claimed identity?) is
  explicitly **not** established by anything in this document or
  `src/release_provenance.rs` -- see "What verification does and does not
  prove" above. No release is signed today.
- **Provenance** (what exactly was bound: commit, workflow, toolchain, host
  class, artifact digests) is what `semaprax.release-provenance.v1` records.
- **Reproducibility** (can a third party rebuild byte-identical artifacts)
  is not claimed, attempted, or implied by any field here.
- **Production support** is a separate, unrelated claim this document does
  not make or imply for any release, signed or not.
