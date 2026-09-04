# Changelog summary

Status: concise release notes for quick orientation.

Audience: users and contributors wanting the latest changes without scanning the full historical changelog.

For complete chronological detail, including historical context and archived artifacts, use:
- [CHANGELOG.md](../CHANGELOG.md)
- [docs/CHANGELOG-ARCHIVE.md](CHANGELOG-ARCHIVE.md).

## Unreleased snapshot

- `semaprax lock` and `semaprax resolve` now accept project directories as arguments, with explicit defaulting and improved missing-manifest guidance.
- Project formatting and comment semantics are now cleaner and safer:
  - formatter and patching preserve `//` comments (including canonicalized placement under comments-preserving mode).
  - patch and parse behavior is more stable for existing projects and generated scaffolds.
- Tooling commands gained clearer diagnostics and result detail:
  - richer `run`/`test` failure outputs with function-id context and contract details,
  - stable project test reporting with case-level outcomes and help lines,
  - improved CLI command discovery and argument hints.
- Language and project workflows gained improved dependency/lock workflows:
  - fine-grained interface compatibility checks (`--emit-interface`, `--compare-interface`),
  - deterministic dependency resolution against local cache artifacts,
  - expanded project lock compatibility and dependency diagnostics.
- Developer ergonomics and verification evidence have been streamlined:
  - quality-routing now routes CLI/editor edits through targeted gates,
  - project generator output, docs, and AGENTS guidance are more complete for both standalone and full-toolchain templates,
  - VS Code diagnostics-on-save now maps machine output to editor ranges with stable suggestions.
- Standard-library growth and numeric contracts were expanded (notably `std.num`), and owned-data profiles now include richer contract/hosted-path handling.

## Latest published milestone

- `v0.3.0` is the current published prerelease tag used by installation and distribution docs.
- `v0.2.0` remains an archived historical tagged release milestone referenced by legacy completion and release-history records.

This file is intentionally compact: it highlights what changed most recently, not a complete project ledger.
