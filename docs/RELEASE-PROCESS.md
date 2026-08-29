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
that commit is embedded into both binaries and recorded in the manifest.

The admitted release hosts and target archives are:

| Hosted runner | Exercised target | Archive |
| --- | --- | --- |
| Ubuntu 24.04 | `x86_64-unknown-linux-gnu` | `semaprax-v0.2.0-x86_64-unknown-linux-gnu.tar.gz` |
| macOS 15 | `aarch64-apple-darwin` | `semaprax-v0.2.0-aarch64-apple-darwin.tar.gz` |
| Windows 2025 | `x86_64-pc-windows-msvc` | `semaprax-v0.2.0-x86_64-pc-windows-msvc.zip` |

Each archive contains `semaprax`, `semapraxd`, `LICENSE`, `README.md`, a fixed
smoke program, and the deterministic `semaprax.release-artifact.v1` manifest.
The platform script unpacks its completed archive and uses the unpacked
`semaprax` binary to run `--version`, `version --json`, `check`, and `run`
before the archive can be uploaded.

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
