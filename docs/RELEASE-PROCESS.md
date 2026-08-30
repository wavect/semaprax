# Release process

Status: bounded v0.2 tag-release procedure.

Audience: maintainers and release reviewers.

SEMAPRAX tag releases are produced only by the repository CI workflow after
the exact tag commit passes every job aggregated by `release-gate`. A local
archive is useful for checking packaging mechanics, but is not release
evidence.

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
package excludes private `new`, `doctor`, Native Rust package publication,
and Windows revision-store host operations. This distribution split does not
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
A new output root is still supported. Windows resolves relative output paths
from PowerShell's filesystem location, not the process working directory.
The Unix host query and version/run smoke checks also retain each command's
exit status: expected stdout cannot turn a failed command into release success.

These scripts assume a trusted, quiescent source checkout and output parent.
Fresh-path checks are not retained-handle authentication against concurrent
filesystem substitution. Failures leave build/staging residue for inspection;
the scripts do not delete or retry over it. A successful archive still needs
the exact-head release gate and real unpacked-binary execution described above.

`tests/release_packaging_unix_v1.rs` and
`tests/release_packaging_windows_v1.rs` author packaging-mechanics regressions
using deliberately fake toolchain executables. They distinguish fresh build
output from stale sentinels, exercise paths containing spaces and rejection
before build/staging effects, and run the actual archive/extraction scripts
when selected. Unix failures include commands that emit the expected stdout
but exit unsuccessfully. The Windows fixture also separates PowerShell's location from
the process working directory. These tests were not run while implementing
the correction; even when run, fake tools do not prove compiler execution,
daemon behavior, release provenance, or a successful product release.

```sh
cargo test --locked -p semaprax --test release_packaging_unix_v1
cargo test --locked -p semaprax --test release_packaging_windows_v1
```

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
Publication does not promote any completion-matrix row or establish production
readiness, a stable language ABI, a stable public protocol, or safety-critical
suitability.
