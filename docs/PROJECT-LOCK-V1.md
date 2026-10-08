# Project Lock v1

Status: implemented bounded dependency-free Project lock; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
Resolution, acquisition, registry/cache, effects, licenses, SBOMs, provenance,
signatures and target execution are not supplied by this lock.

Audience: people and agents building with `semaprax.toml`, package-tooling
authors, and compiler contributors.

Project Lock v1 is `semaprax.lock` beside `semaprax.toml`. Unlike
[Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md), which proves a
supplied dependency graph, it binds the exact package in one working tree.
Rendering uses only the authenticated snapshot. Verification re-renders and
compares exact bytes, so source, manifest, or compiler drift fails closed.
Commands are explicit; `check` never writes or verifies the lock implicitly.

## Commands

```text
semaprax lock [<dir>|semaprax.toml]           # print the canonical lock to stdout
semaprax lock [<dir>|semaprax.toml] --write   # replace semaprax.lock beside the manifest
semaprax lock [<dir>|semaprax.toml] --verify  # verify an existing semaprax.lock
semaprax lock [<dir>|semaprax.toml] --compare <base.lock>       # coarse: classify against a baseline lock
semaprax lock [<dir>|semaprax.toml] --emit-interface            # emit the scalar interface descriptor
semaprax lock [<dir>|semaprax.toml] --compare-interface <b.json># fine: per-export scalar interface diff
```

From a directory containing `semaprax.toml`, the manifest operand can be
omitted, and naming a directory selects the `semaprax.toml` inside it, matching
`check`.

Each mode authenticates and checks the project exactly as `check` does, then
acts:

- The default prints the canonical lock and writes nothing.
- `--write` stages the bytes in a sibling file, renames them over
  `semaprax.lock`, and prints `wrote semaprax.lock for <name> (<digest>)`.
  The ordinary held-object recheck still runs after the write.
- `--verify` reads `semaprax.lock` beside the manifest and compares it against
  a fresh rendering, printing `verified semaprax.lock for <name> (<digest>)`
  on success.

`--compare <baseline.lock>` renders the current project lock and classifies it
against the baseline (see below). Select only one of `--write`, `--verify`, or
`--compare`. `check`, `run`, `test`, and `build` never read or write the
lock.

## Compatibility comparison

`--compare` is a coarse, digest-level compatibility verdict over the facts the
lock records, the project-level counterpart of the fine-grained offline
[Compatibility Evidence v1](OFFLINE-PACKAGE-COMPATIBILITY-EVIDENCE-V1.md). It
prints a `semaprax.project-lock-compatibility.v1` report to stdout and exits 0
when the change is compatible and 1 when it is breaking, so a CI gate can fail
on a break. The classification:

| Change | Verdict |
| --- | --- |
| Package name or frozen contract changed | breaking |
| An export was removed | breaking |
| An export was added | nonbreaking |
| The interface descriptor digest changed with the same export set | breaking |
| A required capability was added (widened) | breaking |
| A required capability was removed | nonbreaking |
| A target was removed | breaking |
| A target was added | nonbreaking |
| Only the version changed | informational |

A pure display rename does not change the interface descriptor digest, which is
normalized without display names, so it is not breaking. The overall verdict is
breaking if any change is breaking, else compatible. This verdict is over the
lock's recorded facts; the per-export type, ownership, effect, and contract
classification remains the offline Compatibility Evidence over Report-v2
subjects.

### Fine-grained scalar interface comparison

For a Project v1 scalar package, `--emit-interface` prints the
`semaprax.project.scalar-wit-interface.v1` descriptor, which carries each
export's stable id, parameter WIT types, and result WIT type. Store it as a
baseline and later run `--compare-interface <baseline.json>` to get a per-export
verdict, printed as `semaprax.project-scalar-wit-compatibility.v1` and exiting
nonzero when breaking. Unlike the coarse `--compare`, this names the exact
export and how its signature changed:

| Change | Verdict |
| --- | --- |
| An export was removed | breaking |
| An export was added | nonbreaking |
| A retained export's result type changed | breaking |
| A retained export's parameter count changed | breaking |
| A retained export's parameter type changed | breaking (names the position) |

Only export signatures and the interface digest are compared, never the project
revision, so two descriptors of the same interface at different revisions are
compatible. `--emit-interface` and `--compare-interface` are scalar-profile
only; other profiles have no scalar WIT interface and return the existing
`SPX-J105` diagnostic. A missing or foreign baseline descriptor rejects with
`SPX-J124`.

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
| `interface` | `exports` (the manifest's exported stable IDs), `kind`, and `digest`. `kind` is `scalar-wit.v1` for the scalar contract (the retained WIT digest), `public-owned-data-api.v1`, `flat-owned-record-api.v1`, `owned-utf8-api.v1`, or `nested-owned-record-api.v1` for the owned profiles (the retained descriptor digest), and `unproven` with a null digest for useful-data and legacy command profiles, which retain no interface descriptor. The native-only Project v26 route uses `source-command.v1` with a null digest and empty exports; this identifies a closed invocation profile, not a public interface or authority. |
| `dependencies` | The admitted manifest's `[dependencies]` rows. Ordinary bundled source dependencies, including the v26 decimal command fixture, are retained here. Unsupported dependencies fail closed before a lock is rendered. |
| `targets` | One row per target with `state`: `declared` for a `[targets] matrix`, `default` for `native64` and `wasm32` when the manifest declares none. A declaration, not proof that the target builds or runs. |
| `capabilities` | The manifest's required capabilities. |
| `compiler` | `package`, `version`, `lock_compatibility`, and the admitted `manifest_layouts`. A different compiler version renders different bytes and therefore reports the lock stale; that is the compatibility rule of this version. |
| `resolution_policy` | `dependencies = none`, `range_grammar = exact-tilde-caret.v1`, `registry = none`, `cache = none`. |
| `nonclaims` | Fixed strings naming what the lock does not assert. |

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-J123` | `semaprax.lock` is stale: the message lists the drifted payload fields. |
| `SPX-J124` | `semaprax.lock` or a `--compare` baseline is missing, is not a plain file of at most 1 MiB, is not readable UTF-8, or is not a Project Lock v1 JSON object. |
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
