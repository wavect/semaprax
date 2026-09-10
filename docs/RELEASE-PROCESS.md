# Release process

Status: bounded tag-release procedure with exact published evidence.

The main sections below record the most recently published tagged milestone.
Later releases inherit the same release workflow; this archive keeps the detailed
evidence for that milestone.

Audience: maintainers and release reviewers.

SEMAPRAX tag releases are produced only by the repository CI workflow after
the exact tag commit passes every job aggregated by `release-gate`. A local
archive can establish scoped local packaging and product behavior, but is not
release-promotion evidence.

## Tag admission

The release tag must be `v` followed by the root `Cargo.toml` package version.
The packaging scripts reject a mismatch before building. They also require the
exact 40-character lowercase hexadecimal Git commit supplied by the workflow;
that commit is embedded into the compiler CLI's version response and recorded
in the manifest. The daemon has no version command or separately attested
embedded commit. The scripts check the supplied label's form and the unpacked
CLI's agreement with it; they do not independently authenticate the checkout
against Git HEAD or the tag. Exact-checkout provenance remains the release
workflow's responsibility, not a consequence of this self-consistency check.

The admitted release hosts and target archives are:

| Hosted runner | Exercised target | Archive |
| --- | --- | --- |
| Ubuntu 24.04 | `x86_64-unknown-linux-gnu` | `semaprax-v0.4.0-x86_64-unknown-linux-gnu.tar.gz` |
| macOS 15 | `aarch64-apple-darwin` | `semaprax-v0.4.0-aarch64-apple-darwin.tar.gz` |
| Windows 2025 | `x86_64-pc-windows-msvc` | `semaprax-v0.4.0-x86_64-pc-windows-msvc.zip` |

Each archive contains `semaprax`, `semapraxd`, `LICENSE`, `README.md`, a fixed
smoke program, and the deterministic `semaprax.release-artifact.v1` manifest.
The archive's `semaprax` is the unpublished `semaprax-toolchain` package's
`semaprax-full` binary, renamed during staging. The standalone crates.io
package excludes private Native Rust package publication, Windows revision-store
host operations, Windows owned npm publication, and the held-parent staged publication behind
the full toolchain's `new`; its own `new` is the bounded route owned by
[standalone project creation](NEW-PROJECT-STANDALONE-V1.md). Both binaries
dispatch `doctor` through the shared driver; the ordinary unavailable-profile
path grants no production tool authority. This distribution split does not
publish any private library crate or promote its platform support.
The platform script unpacks its completed archive and uses the unpacked
`semaprax` binary to run `--version`, `version --json`, `check`, and `run`
before the archive can be uploaded.

## Pre-tag release checklist

A package-version release is a repository-wide consistency change, not only a
root-manifest edit. Before committing the release, update and verify all of the
following surfaces together:

The mechanical portion is automated and intentionally excludes historical
evidence, frozen protocol identities, the human-written release record, and
release-note curation:

```sh
python3 scripts/prepare-release.py --write --version 0.4.0 --date 2026-09-06
python3 scripts/prepare-release.py --check --version 0.4.0
```

The write mode requires a clean worktree, updates the declared current-version
surfaces, and regenerates every repository-owned lockfile offline. Check mode
also runs locked Cargo metadata against the root, example, and platform-test
manifests. Review its diff and complete the human-owned items below.

1. Set the root `Cargo.toml` package version and the unpublished
   `semaprax-toolchain` package version to the release version.
2. Update every exact path-dependency requirement in workspace-private crates
   and isolated `platform-tests/*` manifests. Regenerate the root, example, and
   platform-test lockfiles so each local `semaprax` or `semaprax-toolchain`
   package row agrees with its manifest.
3. Update executable version contracts: CLI text and JSON fixtures, daemon and
   agent transport assertions, doctor report fixtures, external-consumer
   manifest assertions, and the provisioned doctor release tag.
4. Cut the accumulated `Unreleased` changelog entries under a dated release
   heading. Update the README badge and release links, installation archive
   names and example output, documentation landing page, changelog summary,
   citation metadata, and CodeMeta version, date, description, and Rust floor.
5. Search the repository for both the old bare version and old `v` tag. Review
   each survivor rather than replacing it mechanically: evidence logs,
   historical release records, dependency test vectors, crate dependency
   versions, and versioned WIT/schema identifiers are not package-release
   numbers. In particular, `semaprax:private@0.3.0` is a frozen WIT identity and
   does not follow the CLI package version.
6. Run the version, doctor, documentation, manifest/lockfile, package, and
   release-packaging contracts plus the repository's full quality profile. The
   tag must name the exact tested release commit, not a later documentation or
   lockfile repair.

Before tagging, inspect GitHub Actions at the job level. A `Release gate`
failure after a newer `main` push can be a synthetic consequence of cancelled
matrix jobs; it is not evidence that an executed test failed. Conversely, a
job whose overall conclusion is `cancelled` may have reached real test
failures before its timeout, so retain and inspect its log. The Windows
`integration-0` shard includes the large Project harness: on the v0.4.0
candidate that harness ran 337 passing tests locally in about fifteen minutes,
while the Windows shard also spent substantial time compiling and running
other targets. Its 60-minute job budget and `--nocapture` diagnostics are
therefore release-safety controls, not permission to skip, weaken, or hide a
test.

A late syntax or AST addition must compile every workspace-private all-target
consumer before release. Audit exhaustive statement matches and bounded
walkers explicitly: adding a catch-all can make compilation succeed while
still omitting multi-child traversal or introduced bindings from capacity
accounting. Compiler matrix shards are the final check for host-private
consumers, not the first place this audit should happen.

When a bounded carrier ceiling changes, audit its hostile and exact-boundary
fixtures in the same commit. An oversized-length fixture must derive
`current limit + 1` from the owning constant; a frozen historical byte count
can become admissible and then exercise a later malformed-input diagnostic
instead of the intended fail-before-allocation path.

Release automation must also select UTF-8 explicitly for every repository text
read and write. Python's ambient encoding is still a legacy charmap on some
Windows runners; relying on it makes valid UTF-8 documentation fail before the
version or changelog checks execute. The cross-platform workflow regression
disables Python UTF-8 mode while keeping captured process output UTF-8 so this
boundary remains exercised.

Exact CLI help ledgers preserve historical pins by removing each intentional
additive usage line before comparing the older byte length and digest. A new
command or operation must therefore add its exact line to that restoration
inventory; changing an old known-answer merely to accept the larger current
help page would discard the historical compatibility witness.

Use an annotated tag, matching the established repository convention, only
after the release commit is on `main` and the remote head still resolves to
that exact commit:

```sh
git tag -a v0.4.0 -m v0.4.0 <exact-release-commit>
git push origin v0.4.0
```

If another contributor advances `main` before the tag is created, rebase the
release commit, rerun the affected gates, and resolve the new exact commit. If
that newer head contains user-visible or gate behavior, move its changelog
entry into the release bucket before retesting; entries left under
`Unreleased` are intentionally omitted by `scripts/release-notes.py`.
Never move or recreate a published release tag to absorb later work.

## Build-output selection

Both scripts reserve a fresh `build-<target>` directory under the requested
output root and pass its absolute path through Cargo's explicit `--target-dir`.
They copy both binaries only from that same build directory. Ambient
`CARGO_TARGET_DIR` or Cargo configuration cannot redirect the build while
leaving the packager to select stale binaries from the repository's `target/`.
The build directory, package stage, archive and smoke extraction paths must
all be absent, including dangling links, before any of those paths is created.
A Windows smoke extraction root is created exactly once by literal-path .NET
ZIP extraction after that absence check; it is not pre-created through the
PowerShell provider.
A new output root is still supported. Windows resolves relative output paths
from PowerShell's filesystem location, not the process working directory.
The Unix host query and version/run smoke checks also retain each command's
exit status: expected stdout cannot turn a failed command into release success.

These scripts assume a trusted, quiescent source checkout and output parent.
Fresh-path checks are not retained-handle authentication against concurrent
filesystem substitution. Failures leave build/staging residue for inspection;
the scripts do not delete or retry over it. A successful archive still needs
the exact-head release gate and real unpacked-binary execution described above.

`tests/offline_package/release_packaging_unix.rs` and
`tests/offline_package/release_packaging_windows.rs` author packaging-mechanics regressions
using deliberately fake toolchain executables. They distinguish fresh build
output from stale sentinels, exercise paths containing spaces and rejection
before build/staging effects, and run the actual archive/extraction scripts
when selected. Unix failures include commands that emit the expected stdout
but exit unsuccessfully. The Windows fixture also separates PowerShell's location from
the process working directory. The implemented mechanics regressions are HOSTED GREEN; fake tools do not
prove compiler execution,
daemon behavior, release provenance, or a successful product release.

```sh
cargo test --locked -p semaprax --test offline_package release_packaging_unix::
cargo test --locked -p semaprax --test offline_package release_packaging_windows::
```

## Explicit unpacked-product acceptance

`tests/release_archive_product_v1.rs` is an opt-in local product gate over an
already unpacked archive. It does not build or extract that archive, install
anything, consult hosted CI, or fall back to a checkout compiler. Provision
`SEMAPRAX_RELEASE_ROOT` as an absolute native archive directory outside the
checkout, and `SEMAPRAX_RELEASE_COMMIT` as its expected 40-byte lowercase Git
label. The gate checks the closed plain-file inventory, canonical manifest,
fixed smoke source, documentation bytes and exact CLI version responses.

The calculator/daemon lane creates a fresh outside-checkout project, checks its
literal template and repeated graph output, runs check/test/run/Web publication,
and checks no-clobber behavior. Two finite default-v2 daemon sessions obtain
the daemon's revision bindings, exercise read-only snapshot/check/graph/test,
reject a stale revision, recover with a healthy query, and shut down. The
daemon has no version command; this is behavior evidence, not daemon commit
attestation.

The owned-frame lane publishes npm and Rust packages using the archived CLI,
then runs the unchanged standalone Node and locked/offline Rust consumers for
both the baseline and display-renamed Project. The canonical nine-case and
supplemental 72-case corpora are shared with the ordinary frame suite. One
shared test oracle replays source-bound descriptors, regenerates npm artifacts
and reconstructs the native manifest from published bytes and the current
test driver's provider. A differing older compiler must fail those comparisons,
not silently adopt the driver's artifact bytes. This does not prove arbitrary
cross-version compatibility.

Both lanes require a trusted, quiescent archive, source checkout, temporary
parent and selected tool installation. Fresh fixtures and captures are retained.
The owned-frame lane also retains its separately reserved Cargo build cache,
including on failure: direct-child settlement does not justify deleting files
that an unproven descendant might still use.
Finite file-backed input/output and deadline polling bound capture reads and
direct-child settlement attempts; they are not a hard disk quota, descendant
containment, hostile same-principal isolation or a sandbox. No library or
compiler gains authority from this test helper. Archive hashes and label
agreement prove self-consistency only; the caller still owns provenance.

Run the ordinary admission/capture controls without provisioning an archive:

```sh
cargo test --locked --offline -p semaprax --test release_archive_product_v1
```

After setting the two archive variables above, select the real onboarding lane:

```sh
cargo test --locked --offline -p semaprax --test release_archive_product_v1 provisioned_archive_cli_and_daemon_work_outside_checkout -- --ignored --exact
```

The owned-frame lane additionally requires absolute provisioned `NODE`,
`CLANG`, `SEMAPRAX_ARCHIVER` and `CARGO` paths, a compatible Rust/linker/SDK
environment, and already cached consumer dependencies. On Windows retain the
existing `SEMAPRAX_LINKER`/`SEMAPRAX_VCTOOLS` policy. No dependency downloads are
performed and missing prerequisites fail the selected gate:

```sh
cargo test --locked --offline -p semaprax --test release_archive_product_v1 provisioned_archive_owned_frame_consumers_work_outside_checkout -- --ignored --exact
```

<a id="v020-hosted-release-evidence"></a>

## 0.2.0 hosted release evidence

The annotated `v0.2.0` tag resolves to exact commit
`5f6fb9655fdec92c57ab71615cfd7bfa8cc76051`. Its tag-triggered
[workflow run 33608662244](https://github.com/wavect/semaprax/actions/runs/33608662244)
completed successfully on 2026-09-02 with all 45 jobs green. That includes the
complete release-blocking Linux, macOS, Windows, Rust 1.88, dependency,
sanitizer, browser, Project, generated-Rust-consumer, desktop, Android, iOS,
and Component lanes. The blocking
[release gate](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100200871523)
then admitted all three host-built archive jobs and the final
[publication job](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100204458909).

The published [SEMAPRAX v0.2.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.2.0)
contains exactly these release assets:

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `semaprax-v0.2.0-x86_64-unknown-linux-gnu.tar.gz` | 12,064,489 | `955a892dd750cf8d783df583b39b65bf456d8832b55320781166c618a3ba325c` |
| `semaprax-v0.2.0-aarch64-apple-darwin.tar.gz` | 10,542,259 | `aaa453e5b6226afed3d2ba25df2db9e46154968342305a6a793ebf56972efe80` |
| `semaprax-v0.2.0-x86_64-pc-windows-msvc.zip` | 12,355,475 | `879d9b825fab8cff995ec73fc41992348a8ae2db5a120fcdcb85cd5976bf76dc` |
| `SHA256SUMS` | 333 | `2f433932cca89307441e42802527789a253e95ff6300084ff78bbd570f6c67b1` |

The `SHA256SUMS` contents independently agree with the three archive digests
reported by GitHub. Each archive job built on its advertised host, unpacked
its own output, and ran the packaged CLI version, JSON version, `check`, and
`run` smoke before upload. This is exact release-build and smoke evidence; it
does not mean every opt-in or ignored archive-consumer test ran, establish
cross-host byte reproducibility, or broaden any feature contract beyond its
owning specification.

## Historical local archive evidence

A real local `aarch64-apple-darwin` archive was built offline from clean source
commit `177fccfd5f5ab08ac2c86da77046b47f5b4c22f1`, using Rust 1.98 and the
unchanged optimized release profile. The packaging script's unpacked
`--version`, `version --json`, `check`, and `run` checks passed. The resulting
`semaprax-v0.2.0-aarch64-apple-darwin.tar.gz` has SHA-256
`2c07c488a726824ff3b4b3a59379e1cd71a32bcbe93b5f7551283a621efa49c6`.
Six Unix packaging-mechanics regressions also passed separately at that commit;
their fake tools do not contribute to the real compiler execution claim.

The new archive acceptance driver subsequently passed both explicitly selected
lanes on this same retained macOS archive: calculator/Web and read-only daemon
onboarding, plus Node 24.3 and Rust 1.98 consumers of the baseline and renamed
owned-frame packages over both corpora. Five default harness tests also pass,
including admission hostility and finite capture controls; the two actual
archive lanes remain ignored unless explicitly selected. The existing ordinary
frame suite and calibrated macOS ASan/UBSan gate pass after sharing the artifact
oracle, and the two focused test targets pass Clippy with warnings denied.

This older retained local artifact is distinct from the later v0.2.0 tag and
published archives recorded above. Its local acceptance results must not be
relabeled as evidence from the tag commit; changes to either require separate
evidence.

## 0.4.0 hosted release evidence

The annotated `v0.4.0` tag resolves to exact commit
`dfc15e2ddc818fa97744b5a9d69fd6108dd6a321`. The accepted implementation and
release-regression baseline is **HOSTED GREEN**, as recorded in the
[release baseline](RELEASE-0.4.0-STATUS.md). The release-note length issue was
a publication issue, not an outstanding implementation or conformance gate.
The [SEMAPRAX v0.4.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.4.0)
was published at `2026-09-10T10:31:03Z` with the three archives below.
Historical Actions attempts keep their recorded conclusions; this acceptance
record does not relabel an attempt or invent a successful run identifier.

The published prerelease contains exactly these release assets (digests as
reported by the GitHub release API and matching the `SHA256SUMS` generated
during publication):

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `semaprax-v0.4.0-x86_64-unknown-linux-gnu.tar.gz` | 17,922,460 | `21613bed94c9ed41d8ca67cee0924429fff18b1236198c58b16cb4bfd40786f9` |
| `semaprax-v0.4.0-aarch64-apple-darwin.tar.gz` | 15,789,284 | `9b4ebf2bc0e9ca8bdb12db4dea795f9bf7b7b8cf73731f077c3bb80221acda60` |
| `semaprax-v0.4.0-x86_64-pc-windows-msvc.zip` | 18,547,218 | `e175bfc830f189229f0b9881df3afcf0bfb10939cd4a89c5b18bc138def54d07` |

Each archive job built on its advertised host, unpacked its own output, and ran
the packaged CLI version, JSON version, `check`, and `run` smoke before upload.
This is exact release-build and smoke evidence; it does not mean every opt-in
or ignored archive-consumer test ran, establish cross-host byte reproducibility,
or broaden any feature contract beyond its owning specification.

## Publication boundary

Artifact matrix jobs retain read-only repository authority. The final
`publish-release` job alone receives `contents: write`, and only after both
`release-gate` and every artifact-matrix child succeed. It authenticates the
exact three-archive inventory, writes one `SHA256SUMS`, and publishes a GitHub
prerelease because SEMAPRAX remains pre-alpha. The publisher derives the body
with `scripts/release-notes.py`: it selects only the tagged version's dated
`CHANGELOG.md` section, stopping at the next release heading, and surrounds it
with the release nonclaims. A missing, duplicate, or empty section fails the
publication instead of silently creating incomplete notes.

## Nonclaims

The archives are unsigned and are not notarized. No cross-host reproducible build is claimed.
The deterministic manifest does not make the enclosing archive byte-reproducible.
SHA-256 checksums are integrity facts, not signatures, provenance, or publisher authentication.
The historical v0.2.0 publication completed only the tagged-artifact milestone
recorded as WP-04. A later publication is likewise only its bounded release
record: it does not promote any completion-matrix row
or establish production readiness, a stable language ABI, a stable public
protocol, or safety-critical suitability.
