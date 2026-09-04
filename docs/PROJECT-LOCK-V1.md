# Project Lock v1

Status: additive implementation with a local executable gate
(`tests/project.rs::project_lock_v1`); unpromoted. The lock binds one
dependency-free project; it claims no resolution, acquisition, registry,
cache, effect, license, SBOM, provenance, signature, or target-execution fact.

Audience: people and agents building with `semaprax.toml`, package-tooling
authors, and compiler contributors.

Project Lock v1 is the `semaprax.lock` file beside a `semaprax.toml`. It is
the project-level counterpart of the envelope-level
[Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md): where that
lock proves a caller-supplied dependency graph, this one binds the exact
package a working tree contains, so a consumer, a CI job, or a later
resolution route can tell it is looking at the same program. Rendering is a
pure function of the authenticated project snapshot; verification re-renders
and compares bytes, so any source, manifest, or compiler drift fails closed.
Like every other package operation in this repository, the lock is produced
and checked only by an explicit command, never as an implicit effect of
`check`.

## Commands

```text
semaprax lock <manifest>            # print the canonical lock to stdout
semaprax lock <manifest> --write    # replace semaprax.lock beside the manifest
semaprax lock <manifest> --verify   # verify an existing semaprax.lock
```

Each mode authenticates and checks the project exactly as `check` does, then
acts:

- The default prints the canonical lock and writes nothing.
- `--write` stages the bytes in a sibling file, renames them over
  `semaprax.lock`, and prints `wrote semaprax.lock for <name> (<digest>)`.
  The ordinary held-object recheck still runs after the write.
- `--verify` reads `semaprax.lock` beside the manifest and compares it against
  a fresh rendering, printing `verified semaprax.lock for <name> (<digest>)`
  on success.

`--write` and `--verify` are mutually exclusive. `check`, `run`, `test`, and
`build` never read or write the lock.

## Envelope and payload

The file is one line of compact JSON plus a terminal LF. Keys of every object
are in byte order. The envelope carries `bytes`, `digest`, `payload`, and
`schema`, where `schema` is `semaprax.project-lock.v1`, `payload` is the
object below, `bytes` is its compact length, and `digest` is `sha256:<hex>`
over the domain `semaprax.project-lock.v1` plus NUL, the little-endian `u64`
payload length, and the exact payload bytes.

| Field | Meaning |
| --- | --- |
| `schema` | `semaprax.project-lock.v1`. |
| `package` | `name`, `version` (null for the frozen v1 layout, which carries none), `manifest_schema` (the layout the bytes were parsed from), `contract` (the frozen profile contract the manifest lowers to), `profile` (`scalar` or the profile name), and `manifest_digest` over the canonical manifest bytes under the domain `semaprax.project-lock.manifest.v1` plus NUL. |
| `program_root` | The project revision: the digest binding the canonical manifest and the workspace revision, and the value `check` prints. This is the lock's program root. |
| `source` | `workspace_revision` and one `files` row per declared source with `path`, `source_revision`, and `source_digest`. Source text is never embedded. |
| `interface` | `exports` (the manifest's exported stable IDs), `kind`, and `digest`. `kind` is `scalar-wit.v1` for the scalar contract (the retained WIT digest), `public-owned-data-api.v1`, `flat-owned-record-api.v1`, `owned-utf8-api.v1`, or `nested-owned-record-api.v1` for the owned profiles (the retained descriptor digest), and `unproven` with a null digest for the six useful-data and command profiles, which retain no interface descriptor. |
| `dependencies` | The manifest's `[dependencies]` rows. Always empty on this toolchain, because a declared dependency fails every build closed with `SPX-J121` before a lock can be rendered. |
| `targets` | One row per target with `state`: `declared` for a `[targets] matrix`, `default` for `native64` and `wasm32` when the manifest declares none. A declaration, not proof that the target builds or runs. |
| `capabilities` | The manifest's required capabilities. |
| `compiler` | `package`, `version`, `lock_compatibility`, and the admitted `manifest_layouts`. A different compiler version renders different bytes and therefore reports the lock stale; that is the compatibility rule of this version. |
| `resolution_policy` | `dependencies = none`, `range_grammar = exact-tilde-caret.v1`, `registry = none`, `cache = none`. |
| `nonclaims` | Fixed strings naming what the lock does not assert. |

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-J123` | `semaprax.lock` is stale: the message lists the drifted payload fields. |
| `SPX-J124` | `semaprax.lock` is missing, is not a plain file of at most 1 MiB, is not readable UTF-8, or is not a Project Lock v1 JSON object. |
| `SPX-J125` | `--write` could not stage or rename the lock. |

Usage errors of `lock` exit with status 2 and a `semaprax lock --help` hint.

## Evidence and nonclaims

`tests/project.rs::project_lock_v1` pins: byte-identical renders, digest
recomputation from the payload bytes, the program root equal to the revision
`check` prints, digest-only source rows, the default target rows, the
`--write` round trip and its idempotence, `--verify` success and the
missing-lock rejection, source drift and manifest drift each failing with
`SPX-J123` and the exact drifted field list, foreign and directory locks
failing with `SPX-J124`, `check` passing unaffected with and without a lock,
the interface kinds for the scalar, command, and owned-data profiles, and the
usage and scoped-help contracts.

The lock does not resolve, acquire, or cache dependencies, does not execute
any target, and carries no effect, license, SBOM, provenance, or signature
facts. Those remain the subjects of
[Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md),
[Offline Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md), and the reserved tables
of [Package Manifest v1](PACKAGE-MANIFEST-V1.md).
