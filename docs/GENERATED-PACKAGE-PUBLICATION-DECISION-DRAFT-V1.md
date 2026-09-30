# Generated package publication decision (DRAFT -- not approved)

Status: **unapproved publication design draft**; historical issue #145
background, not the authority for the accepted Rust-only maintenance scope.
Nothing here authorizes publication, signing or public-support promotion.
Audience: maintainers considering a separately authorized publication design.

[ADR 0003](decisions/0003-maintained-generated-package-support.md#current-maintainer-decision)
owns the current maintenance decision, reaffirmed on 2026-09-30 for #325:
ordinary owned-data Rust only; npm maintenance deferred to its own ADR. The
package/preview identities are fixed there, with real SemVer required before
publication. Generated packages remain unpublished. Selecting registry names,
publisher identities, release versions, credentials and provenance is still
separate work; this draft does not reopen the accepted support scope.

## Historical proposal

The original draft considered the existing generated-package profile below
for a maintained, reproducible external-consumer route. ADR 0003 subsequently
selected only its Rust package. The npm identifiers in this inventory do not
constitute an npm support or publication decision:

| Field | Value |
| --- | --- |
| Project schema / profile | `semaprax.project.v8` / `owned-data-api.v1` |
| Generated npm package identifiers | `semaprax.project-npm-build.v7` carrier, `semaprax.owned-data-api.v1` metadata |
| Generated Rust package identifier | `semaprax.native-rust-owned-data-sdk.v1` |
| Owning specification | [docs/PUBLIC-OWNED-DATA-API-V1.md](PUBLIC-OWNED-DATA-API-V1.md) |

This draft uses the existing generated-package route marked feature-complete and
release-regressed (HOSTED GREEN under the v0.4.0 baseline). Publication remains
an open decision. The public generic ABI (issue #144/SPX-AI-045 territory) is
out of scope: this draft changes no admission rule and grants no authority to
generic packages.

## What is genuinely new in this slice

`scripts/generated-package-release.py` adds a release-preparation and
dry-run-check layer *around* the compiler's own generated output -- it never
edits the compiler's render pipeline or its pinned byte-exact output tests:

- `prepare` validates a built package directory's inventory is exactly the
  closed set this profile produces, scans every byte for secret-shaped and
  local-host-path-shaped substrings, and confirms the Rust crate is
  structurally unpublishable by Cargo (`publish = false`, no
  path/registry dependency) and the npm package still carries none of the
  supply-chain-risk keys the compiler already forbids
  (`dependencies`/`devDependencies`/`scripts`/`private`). It then copies the
  exact input bytes into a `payload/` directory unchanged and adds
  deterministic wrapping documents: a README naming the exact public API
  descriptor digest and declared supported toolchain, the repository's
  Apache-2.0 `LICENSE`, and a checksum manifest
  (`package-preview-manifest.json`, schema
  `semaprax.generated-package-preview.v1`) covering every file.
- `check` recomputes and diffs that checksum manifest against what is
  actually on disk (tamper detection), and only in its default (and only
  implemented) dry-run mode, optionally exercises a real `npm pack --dry-run`
  or `cargo publish --dry-run` against an explicit, caller-supplied absolute
  tool path -- never a PATH-discovered one. `--publish` is always refused:
  there is no live-publish code path in this tool.
- Both subcommands refuse outright, before touching a file, if any of a fixed
  list of publish-credential-shaped environment variables
  (`NPM_TOKEN`, `CARGO_REGISTRY_TOKEN`, and siblings) is set. This tool has no
  legitimate use for one.

See `scripts/test-generated-package-release.py` for the determinism,
tamper-detection, and refusal evidence.

## Future publication decisions (outside issue #325)

1. **Whether to publish at all**, and if so to which registries (npmjs.org
   scope/org name; crates.io publisher identity) under which package name.
   Because each generated package is tied to one exact Project's exact
   public API descriptor digest, a maintainer must decide whether "publish"
   means one canonical reference package (e.g. a fixed demonstration
   Project) or a per-consumer-project workflow this repository only tools,
   never executes on a consumer's behalf.
2. **Registry credentials.** Real publication needs an `NPM_TOKEN` and a
   `CARGO_REGISTRY_TOKEN` (or equivalent OIDC trusted-publishing setup) that
   only a maintainer can provision; this tool is deliberately incapable of
   accepting or using either.
3. **Signing.** Issue [#168](https://github.com/wavect/semaprax/issues/168)
   and [docs/RELEASE-SIGNING-POLICY-V1.md](RELEASE-SIGNING-POLICY-V1.md) own
   the toolchain-archive signing decision; the same open questions
   (signing-tool selection, trusted-identity policy, `id-token: write`
   wiring) apply to a generated package and are not resolved by this draft.
4. **CI wiring for a real publish job.** This slice adds no new CI job and
   changes no job count; a future `publish-generated-package` job (mirroring
   `publish-release`'s pattern of running only after every release blocker
   succeeds, with `contents: write`/registry-publish scope granted to no
   earlier job) is a separate, explicitly maintainer-reviewed change.
## Consumer evidence already retained

Genuine compiler-built Rust `.crate` extraction and npm `.tgz`
pack/install/execute gates have passed; see ADR 0003's retained evidence rows
14-16, including the exact local Cargo/rustc 1.85.0 Rust-consumer pass. The
synthetic npm fixture and release-root path-dependency lane remain separate
wrapper/integration coverage, not substitutes for those genuine consumers.
Do not describe the genuine npm gate as awaiting its first local execution.

Archive consumption is not registry installation. The cited runs remain bound
to their revisions, hosts and selected toolchains; they do not grant npm
maintenance, publisher identity, real release provenance or publication.
No new hosted run is required solely to close the maintenance-decision ticket
#325. A future registry workflow would need its own explicit authority and
execution evidence; this draft does not request credentials or a registry run.

## Rollback / deprecation policy (proposed)

Nothing under this profile has ever been published, so there is no existing
artifact to roll back. Once (if) a maintainer approves real publication:

- A published version is never mutated or deleted; a broken release is
  superseded by a new version, following the existing toolchain-archive
  precedent in [docs/RELEASE-PROCESS.md](RELEASE-PROCESS.md#nonclaims).
- Deprecating the whole route means the CI publish job stops running for new
  tags; it does not retract already-published artifacts.
- A breaking change to the profile's descriptor/rename compatibility rules
  (see docs/PUBLIC-OWNED-DATA-API-V1.md) must not be disguised as a patch
  version, per this issue's required test list.

## Review checkpoint

This publication draft still requires independent maintainer review; it is
not an approved publication decision. That review must record the actual
reviewer, date and authorized/deferred publication scope. The accepted Rust-only
maintenance decision is already recorded in ADR 0003 and is not awaiting
acceptance of this draft. npm maintenance requires its own ADR regardless of
any future review of this publication design.
