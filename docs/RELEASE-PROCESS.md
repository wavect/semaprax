# Release process

Status: bounded tag-release procedure with exact published evidence.

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
| Ubuntu 24.04 | `x86_64-unknown-linux-gnu` | `semaprax-v0.2.0-x86_64-unknown-linux-gnu.tar.gz` |
| macOS 15 | `aarch64-apple-darwin` | `semaprax-v0.2.0-aarch64-apple-darwin.tar.gz` |
| Windows 2025 | `x86_64-pc-windows-msvc` | `semaprax-v0.2.0-x86_64-pc-windows-msvc.zip` |

Each archive contains `semaprax`, `semapraxd`, `LICENSE`, `README.md`, a fixed
smoke program, and the deterministic `semaprax.release-artifact.v1` manifest.
The archive's `semaprax` is the unpublished `semaprax-toolchain` package's
`semaprax-full` binary, renamed during staging. The standalone crates.io
package excludes private `doctor`, Native Rust package publication, Windows
revision-store host operations, and the held-parent staged publication behind
the full toolchain's `new`; its own `new` is the bounded route owned by
[standalone project creation](NEW-PROJECT-STANDALONE-V1.md). This distribution split does not
publish any private library crate or promote its platform support.
The platform script unpacks its completed archive and uses the unpacked
`semaprax` binary to run `--version`, `version --json`, `check`, and `run`
before the archive can be uploaded.

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
the process working directory. These tests were not run while implementing
the correction; even when run, fake tools do not prove compiler execution,
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

## Publication boundary

Artifact matrix jobs retain read-only repository authority. The final
`publish-release` job alone receives `contents: write`, and only after both
`release-gate` and every artifact-matrix child succeed. It authenticates the
exact three-archive inventory, writes one `SHA256SUMS`, and publishes a GitHub
prerelease because v0.2 remains pre-alpha.

## Nonclaims

The archives are unsigned and are not notarized. No cross-host reproducible build is claimed.
The deterministic manifest does not make the enclosing archive byte-reproducible.
SHA-256 checksums are integrity facts, not signatures, provenance, or publisher authentication.
Publication completes only the v0.2.0 tagged-artifact milestone recorded as
WP-04. Other than that bounded release record, publication does not promote any completion-matrix row
or establish production readiness, a stable language ABI, a stable public
protocol, or safety-critical suitability.
