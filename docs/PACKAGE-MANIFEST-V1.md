# Package Manifest v1

Status: additive implementation with local executable gates; unpromoted. The
table layout is admitted by every project route, and Project builds link the
closed compiler-bundled `std.*` inventory. No ordinary-package build,
acquisition, registry, or publication claim is made.

Audience: people and agents writing `semaprax.toml`, package-tooling authors,
and compiler contributors.

Package Manifest v1 is the one extensible `semaprax.toml` layout. Each frozen
[Project Manifest v1](PROJECT-MANIFEST-V1.md) through v11 schema fixes a
whole-file sequence of assignments, so every product tranche so far has added
a new `semaprax.project.vN` string. This layout instead admits one closed
catalog of optional tables and keys under a single schema string,
`semaprax.manifest.v1`, and lowers every admitted manifest onto the frozen
profile contract it selects. Future tranches add a table or a key to this
specification; they do not add a whole-project schema.

The frozen layouts remain admitted and byte-for-byte unchanged. A project may
use either layout; the two differ only in manifest bytes.

## Manifest

The scalar contract, equivalent to the committed
[calculator project](../examples/calculator-project/semaprax.toml):

```toml
schema = "semaprax.manifest.v1"

[package]
name = "calculator"
version = "0.1.0"

[modules]
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["calculator.tests"]

[exports]
web = ["calculator.add", "calculator.divide", "calculator.is-negative", "calculator.multiply", "calculator.not", "calculator.subtract"]
```

A command profile, equivalent to the committed
[spxgrep project](../examples/spxgrep-project/semaprax.toml), with the two
optional tables this toolchain admits beyond the profile facts:

```toml
schema = "semaprax.manifest.v1"

[package]
name = "spxgrep"
version = "0.1.0"
profile = "useful-data-command.v1"

[modules]
entry = "spxgrep.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["spxgrep.tests"]

[exports]
web = ["spxgrep.contains"]

[command]
function = "spxgrep.contains"

[capabilities]
required = ["process.stdout.write"]

[dependencies]
bytes-util = "^1.2.0"

[targets]
matrix = ["native64", "wasm32"]
```

### Table catalog

Tables appear in this order. A table marked required must be present; an
optional table is omitted when it would be empty.

| Table | Keys | Presence | Meaning |
| --- | --- | --- | --- |
| `[package]` | `name`, `version`, `profile` | required | Package identity. `name` matches `[a-z][a-z0-9-]*` within 64 bytes. `version` is canonical Semantic Versioning text within 128 bytes and is always present. `profile` selects one profile contract from the table below; omitting it selects the scalar contract. |
| `[modules]` | `entry`, `sources`, `tests` | required | Source modules. `entry` and the single `tests` module are bounded module names and must differ. `sources` lists 2 to 16 strictly byte-sorted canonical relative `.spx` paths. |
| `[exports]` | `web` | required | Exported semantic interfaces: 1 to 32 strictly byte-sorted stable IDs. For a command profile it contains exactly the command function. |
| `[command]` | `function`, `input` | required for command profiles, forbidden otherwise | Entry point of a command profile. `input` is required exactly when the profile fixes an input contract. |
| `[capabilities]` | `required` | required for command profiles, forbidden otherwise | Required capabilities. Each command profile fixes the exact list. |
| `[dependencies]` | one `name = "range"` row per dependency | optional | Dependency requirements. Names are dotted lowercase package identities (`[a-z][a-z0-9._-]*` of at most 128 bytes with non-empty `.`-separated segments, e.g. `examples.meaning`), strictly byte-sorted, matching the resolver package identity; ranges use only `=x.y.z`, `~x.y.z`, or `^x.y.z` with canonical `u32` components, the grammar of [Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md). At most 64 rows. `semaprax resolve` selects them against a content-addressed cache ([Project Dependency Resolution v1](PROJECT-DEPENDENCY-RESOLUTION-V1.md)). |
| `[targets]` | `matrix` | optional | Target matrix: a non-empty, strictly byte-sorted subset of `native64` and `wasm32`. Absent means every target is admitted. |

Values are plain strings without escapes or one-line arrays of such strings
separated by `", "`. Tables are separated by exactly one blank line, keys keep
the order of the table above, and the file ends with one LF. There are no
comments, dotted keys, inline tables, or multi-line arrays. Byte limits are
those of Project Manifest v1: 64 KiB per manifest and 16 MiB of source.

### Reserved tables and keys

The following names are reserved for additive revisions of this
specification and reject today with `SPX-J120`, so an older toolchain fails
closed on a manifest that uses a newer table rather than silently ignoring it:

- tables `[agents]`, `[artifacts]`, `[compatibility]`, `[features]`,
  `[interfaces]`, and `[profiles]`;
- keys `compatibility`, `license`, and `description` inside `[package]`.

Any other unknown table or key also rejects with `SPX-J120`, naming the
admitted catalog.

## Lowering

`[package] profile` selects the frozen profile contract. The parsed manifest
reports that contract through `ProjectManifest::schema`, exactly as a frozen
manifest does, and reports the source layout through
`ProjectManifest::manifest_schema` and `ProjectManifest::layout`. Every
project route, descriptor, revision store header, and generated package reads
only the contract, so a table manifest produces the same `project_schema`
values, the same web/npm/native artifacts, and the same Wasm bytes as its
frozen equivalent. Only the manifest bytes differ, and with them the
canonical manifest embedded in images, revision-store entries, and candidate
archives.

| `profile` | Contract | Owning specification |
| --- | --- | --- |
| omitted | `semaprax.project.v1` | [Project Manifest v1](PROJECT-MANIFEST-V1.md) |
| `useful-text-consumer.v1` | `semaprax.project.v2` | [Project Manifest v2](PROJECT-MANIFEST-V2.md) |
| `useful-data.v1` | `semaprax.project.v3` | [Project Manifest v3](PROJECT-MANIFEST-V3.md) |
| `useful-data-command.v1` | `semaprax.project.v4` | [Project Manifest v4](PROJECT-MANIFEST-V4.md) |
| `useful-data-command.v2` | `semaprax.project.v5` | [Project Manifest v5](PROJECT-MANIFEST-V5.md) |
| `language-command-io.v1` | `semaprax.project.v6` | [Bounded Language Command IO v1](BOUNDED-LANGUAGE-COMMAND-IO-V1.md) |
| `line-command-io.v1` | `semaprax.project.v7` | [Project Manifest v1, v7 profile](PROJECT-MANIFEST-V1.md#additive-project-manifest-v7-line-command-profile) |
| `owned-data-api.v1` | `semaprax.project.v8` | [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) |
| `flat-owned-record-api.v1` | `semaprax.project.v9` | [Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md) |
| `owned-utf8-api.v1` | `semaprax.project.v10` | [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) |
| `nested-owned-record-api.v1` | `semaprax.project.v11` | [Public Nested Owned Record API v1](PUBLIC-NESTED-OWNED-RECORD-API-V1.md) |

The profile-specific rules the frozen layouts encode by position apply
unchanged: the four command profiles require `[command]` and the exact
`[capabilities] required` list of their contract, `useful-data-command.v2`
requires `input = "stdin-bytes+one-utf8-arg.v1"`, the two `-io.v1` profiles
require `input = "argv-utf8+stdin-bytes.v1"`, and every other profile forbids
both tables. The scalar contract additionally carries a `version`, which the
frozen v1 layout lacks; `ProjectManifest::package_version` reports it.

## Canonical bytes

A manifest is admitted only when its bytes equal its own canonical rendering,
the same rule the frozen layouts follow, so the bytes bound into project
revisions, images, and stores are unique for a given meaning. A non-canonical
table manifest rejects with `SPX-J100` and a `help` line naming the first
differing line, for example
`line 8: expected `entry = "calculator.app"`, found `sources = [...]``. A
comment or a doubled space is therefore a diagnostic, not a silent
normalization.

## Semantics of the optional tables

`[dependencies]` admits the requirement grammar and has two deliberately
separate routes. `semaprax resolve` selects ordinary packages from its explicit
caller-populated cache, but Project builds do not yet consume those results.
Project build/check/test/run instead recognize only the closed compiler-bundled
`std.*` inventory at version `0.1.0`, validate the range, and link its immutable
source plus transitive standard dependencies in memory. This requires no cache
and performs no acquisition or network access. An unknown dependency or a
range excluding the bundled version fails with `SPX-J121` before output.

`[targets] matrix` gates the CLI `build` route: `web`, `wasm`, and `npm`
require `wasm32`, and every other target requires `native64`. A target the
matrix excludes rejects with `SPX-J122` after project admission and before any
output effect. The matrix is a declaration of intended targets, not proof
that a target builds, runs, or is supported on any host.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-J100` | Manifest grammar: missing or mistyped required key, foreign profile facts, dependency or target grammar, or non-canonical bytes (with a first-differing-line `help`). |
| `SPX-J101` | Capacity: manifest bytes, source count, export count, or dependency count over the frozen bounds. |
| `SPX-J120` | A reserved or unknown table or key. The message names the reserved name or the admitted catalog. |
| `SPX-J121` | A dependency is not in the bundled standard-library inventory or its range excludes the bundled version. |
| `SPX-J122` | A CLI build target outside the declared `[targets] matrix`. |

## Evidence and nonclaims

`tests/project.rs::package_manifest_v1` pins: the lowering of all eleven
profiles against their frozen equivalents, including `is_vN` and the
per-profile command rules; the reserved and unknown table and key rejections;
the first-differing-line canonical diagnostics; the dependency grammar and the
`SPX-J121` CLI rejection; the target grammar, `SPX-J122` CLI rejection, and an
admitted web build; `check`, `test`, `run`, `project-image`, and web `build`
over the calculator example rewritten into the table layout, with byte-equal
Wasm against the frozen manifest; `check` and `test` over the spxgrep command
example; and canonical parsing of every `toml` block in this document.

This specification does not claim dependency resolution, a lockfile, a
content-addressed cache, registry access, feature flags, agent definitions,
build profiles, generated-artifact records, a compatibility policy, licenses,
provenance, or an interface digest. Those are the reserved names above and
the subjects of [Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md),
[Offline Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md), and
[Compatibility Evidence v1](OFFLINE-PACKAGE-COMPATIBILITY-EVIDENCE-V1.md),
which still operate on caller-supplied envelopes rather than on this manifest.
`semaprax project-scaffold` continues to print the frozen v1 layout.
