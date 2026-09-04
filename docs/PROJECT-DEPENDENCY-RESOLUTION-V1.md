# Project Dependency Resolution v1

Status: additive implementation with a local executable gate
(`tests/project.rs::dependency_resolution_v1`); unpromoted. Resolution is a
read-only analysis over a caller-populated local cache. No registry is
contacted, no package is acquired or built, and no `ACTIVE` pivot or artifact
is produced.

Audience: people and agents building with `semaprax.toml`, package-tooling
authors, and compiler contributors.

`semaprax resolve` bridges a project's declared `[dependencies]`
([Package Manifest v1](PACKAGE-MANIFEST-V1.md)) to the offline
[Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md): it reads a local
content-addressed cache of Semantic Package Subject-v3 envelopes, deterministically
selects one version per package that satisfies the manifest's ranges and their
transitive requirements, and prints the resolver's evidence, which embeds an
[Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md). Reading the
cache is the explicit effect of this command; `check`, `build`, `run`, and
`test` never touch it, and a declared dependency still fails the build closed
with `SPX-J121` until a build path exists.

## Command

```text
semaprax resolve <manifest> --target <native64|wasm32> --cache <dir> [--write|--verify] [--max-bytes N]
```

`resolve` parses the manifest (it does not build the project, so it resolves a
manifest whose `[dependencies]` a build would reject), takes its `[dependencies]`
as the root requirements, its `[capabilities]` as the allowed capabilities, and
the given `--target`, then loads the cache and runs the resolver. The evidence
is printed to stdout. Resolution is per target: a project with both targets in
its matrix is resolved once per target.

The manifest `[dependencies]` supply 1 to 4 roots; a manifest that declares
none has nothing to resolve. `--target` must be `native64` or `wasm32`, and
must be within the manifest's `[targets] matrix` when one is declared.

## Content-addressed cache

The cache is a directory. Each package version is one file `<hex>.json` holding
a Subject-v3 envelope whose `digest` field is `sha256:<hex>`; the file name is
the subject's own digest, so the store is content-addressed and a misfiled or
tampered subject is rejected before resolution. `resolve` loads every `.json`
file (at most 64, the resolver's catalog bound), in digest order, so the result
does not depend on directory iteration order. Each subject is independently
replayed by the resolver; the file-name check adds integrity on top.

The cache is caller-populated. Placing subjects into it — from a registry, a
`fetch`, or a vendoring step — is an explicit action outside the compiler, in
keeping with the rule that registry access is never an implicit compiler
action. This toolchain ships no `add`, `fetch`, or `update`.

## Determinism and authority

For one manifest and one cache, `resolve` prints byte-identical evidence on
every run. The evidence is the resolver's exact output: it binds the canonical
roots, target, allowed capabilities, catalog digest, selected coordinates, and
the embedded lock, and it independently replays every subject. The command has
no network, process, or mutation authority and reads only the named manifest
and cache files; it acquires nothing and builds nothing.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-J126` | The manifest declares no dependencies, the target is outside the declared matrix, the cache is missing, oversized, unreadable, or holds a subject that is not content-addressed, or a `--verify` pin is missing or stale. |
| resolver `SPX-PR6xx` | The requirements cannot be satisfied from the cache: a missing package, an unsatisfiable range, a cycle, or a bound exceeded. |

Usage errors exit with status 2 and a `semaprax resolve --help` hint. A bad
`--target` value is a usage error; a target outside the matrix is `SPX-J126`.

## Evidence and nonclaims

`tests/project.rs::dependency_resolution_v1` pins: transitive selection from a
three-subject cache where a caret range picks the higher version and pulls in
its dependency; byte-identical repeat runs; the empty-`[dependencies]`,
content-address-mismatch, target-outside-matrix, missing-cache, and bad-target
rejections; a declared target resolving; and the usage and scoped-help
contracts. Cache fixtures are built from committed example sources through the
same `package_report_v2` and `package_lock_v3` API the envelope resolver uses.

`resolve` does not acquire, download, cache, build, or execute any package,
contacts no registry, and makes no compatibility, provenance, license, or SBOM
claim. Those remain the subjects of the offline envelope specifications and the
reserved tables of [Package Manifest v1](PACKAGE-MANIFEST-V1.md).
