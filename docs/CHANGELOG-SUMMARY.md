# Changelog summary

Status: concise release notes for quick orientation.

Audience: users and contributors wanting the latest changes without scanning the full historical changelog.

For complete chronological detail, including historical context and archived artifacts, use:
- [CHANGELOG.md](../CHANGELOG.md)
- [docs/CHANGELOG-ARCHIVE.md](CHANGELOG-ARCHIVE.md).

## 0.4.0 highlights

- Generic owned records and variants now cover bounded nested relay, internal
  ScalarV1 composition, two-sided owned results, and loop-carried owned byte
  buffers and vectors across interpreter, native C11, and Core Wasm evidence;
  Copy-scalar vectors also gain bounded immutable `for` traversal lowered to
  the existing length/get/while HIR.
- ProgramRoot v3, Exact Program Context v2, contracts/test facts, universal
  semantic query and transaction operations, persistent service transports,
  and installed diagnostics/fix guidance deepen the agent-facing semantic
  workflow while retaining revision binding and explicit authority.
- The language-native agent path now includes source Agent compilation,
  interaction contracts, lifecycle execution, durable checkpoints, and a
  bounded Proposal-to-Runtime v1 compatibility adapter.
- Project v12/v13 add replayable network and HTTPS command profiles, including
  fixture-only npm/Core-Wasm execution, native C11 HTTPS, explicit capability
  admission, and hosted browser evidence without ambient network authority.
- The bundled standard library adds the bounded JSON package family through
  structural documents and escape decoding, alongside expanded collections,
  encoding, URL, path, time, random, CSV, TOML, text, and byte utilities.
- CLI, VS Code, project dependency, scaffold, lock, verification, and guided
  help surfaces gained broader exact workflows and more actionable stable
  diagnostics.
- CI and release infrastructure now fails closed at the Release gate, preserves
  Windows diagnostics for the large Project shard, exercises cross-platform
  generated C consumers safely, and carries the offline doctor tool closure
  through its provisioned release paths.

## Latest published milestone

- `v0.4.0` is the current prerelease tag used by installation and distribution docs.
- `v0.3.5` remains the immediately preceding prerelease.
- `v0.2.0` remains an archived historical tagged release milestone referenced by legacy completion and release-history records.

This file is intentionally compact: it highlights what changed most recently, not a complete project ledger.
