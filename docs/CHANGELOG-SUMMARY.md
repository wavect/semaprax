# Changelog summary

Status: concise release notes for quick orientation.

Audience: users and contributors wanting the latest changes without scanning the full historical changelog.

For complete chronological detail, including historical context and archived artifacts, use:
- [CHANGELOG.md](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md)
- [docs/CHANGELOG-ARCHIVE.md](CHANGELOG-ARCHIVE.md).

## 0.4.1 highlights

- Public generic ownership is now a separate milestone rather than a side
  effect of the internal generic closure. It owns nine prerequisite gates, the
  separation invariants between them, and a standing decision that the surface
  is unsupported and unpublished — plus an executable separation gate that
  reddens if an internal admission ever starts producing a public generic
  signature.
- Four of those gates landed with local evidence: a versioned target-neutral
  type grammar with length-framed injective identities, explicit template and
  ordered argument identities that a display rename cannot move, semantic
  compatibility rules stricter than source compatibility wherever a foreign
  consumer sees more than a caller, and a candidate-bound delta over immutable
  Project candidates with byte-exact independent replay.
- Four generated metadata consumers — Rust, TypeScript/Wasm, C11 and C++ — are
  compiled warning-free and executed against nine hostile documents, and all
  of them must refuse each one with the same closed reason. The milestone
  corpus also runs on Linux, macOS and Windows as a declared release blocker.
- The bundled standard library gained effect-free policy and cursor packages
  across `std.fs`, `std.env.policy`, `std.process`, `std.agent`, `std.bytes`,
  `std.format`, `std.log`, `std.test`, `std.data.csv`, `std.data.toml`,
  `std.data.json.dec`, `std.encoding.base64`, `std.num.overflow`,
  `std.path.normalize` and `std.io.lines`, none of which claims a capability it
  does not exercise.
- Private owned iterator payloads, generic iterator operations, consuming
  `for own` traversal, function values and closures deepened, and durable Agent
  migration and iterative runtimes stayed bound to exact immutable semantic
  roots.

## 0.4.0 highlights

- Generic owned records and variants now cover bounded nested relay, internal
  ScalarV1 composition, two-sided owned results, and loop-carried owned byte
  buffers and vectors across interpreter, native C11, and Core Wasm evidence;
  Copy-scalar vectors also gain bounded immutable `for` traversal lowered to
  the existing length/get/while HIR, while compiler-owned bounded `Box<T>` and
  the alloc-tier `std.mem` package add synchronous scalar ownership transfer.
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

- `v0.4.1` is the current prerelease tag used by installation and distribution docs.
- `v0.4.0` remains the immediately preceding prerelease, and its
  [release baseline](RELEASE-0.4.0-STATUS.md) remains the accepted hosted-green
  evidence record.
- `v0.3.5` remains the prerelease before that.
- `v0.2.0` remains an archived historical tagged release milestone referenced by legacy completion and release-history records.

This file is intentionally compact: it highlights what changed most recently, not a complete project ledger.
