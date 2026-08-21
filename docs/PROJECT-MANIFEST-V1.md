# Project Manifest v1

Project Manifest v1 is a bounded, invocation-local way to build one explicit
multi-file pure-scalar program. It reuses the existing Semantic Workspace
Phase-A resolver in memory, then links the selected entry and test provider
closures into validated HIR. It creates no `.semaprax-workspace`, generation,
`ACTIVE` pivot, cache, lock, source rewrite, dependency resolution, or third
workspace.

This is a build-input protocol, not the managed Semantic Workspace or
Workspace Transaction authority. It publishes neither source state nor a
reusable authorization token.

## Manifest

The manifest path must be named `semaprax.toml`. It is canonical UTF-8 without
a BOM or CRLF, is at most 65,536 bytes, and has exactly these six assignments
in this order followed by one LF:

```toml
schema = "semaprax.project.v1"
name = "calculator"
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["calculator.add", "calculator.divide", "calculator.is-negative", "calculator.not"]
tests = ["calculator.tests"]
```

The checked example is [examples/calculator-project/semaprax.toml](../examples/calculator-project/semaprax.toml).
Unknown, repeated, reordered, omitted, or noncanonical assignments are
rejected.

| Field | Rule |
| --- | --- |
| `schema` | Exactly `semaprax.project.v1`. |
| `name` | Lowercase `[a-z][a-z0-9-]*`, 1–64 bytes. |
| `entry` | One bounded module name, at most 240 bytes. |
| `sources` | Strictly sorted, unique explicit canonical relative `.spx` paths; 2–16 entries, each at most 240 bytes. |
| `web_exports` | Strictly sorted, unique lowercase `[a-z0-9._-]` stable IDs; 1–32 entries, each at most 128 bytes. |
| `tests` | Exactly one bounded module name, at most 240 bytes. |

The total exact source input is at most 16 MiB. There are no dependencies,
package names, version ranges, registries, discovery rules, capability grants,
environment interpolation, include directives, or extension keys.

## Admission and linking

The loader holds and authenticates the manifest, every declared source, and its
directory ancestry; it rejects aliases, symlinks/reparse points, duplicate
physical source files, noncanonical source text, and final input drift. It
then supplies precisely the listed source bytes to the existing Semantic
Workspace Phase-A preflight once. That preflight retains its usual bounded
explicit stable-ID function-provider/DAG checks; it is not Semantic Workspace
initialization and does not publish an immutable generation.

Project v1 additionally admits only the complete pure-scalar authenticated set:

- every module is effect-free and permit-free;
- there are no authored types, interface declarations, interface/native
  imports, generic templates or instances, or `use type` edges;
- each executable function has only by-value `i64`/`bool` parameters and an
  `i64`/`bool` result;
- the entry and sole test modules each define exactly one explicitly identified
  `main`; a provider module cannot define `main`.

The entry and test closures include only their transitive explicit function
providers. Explicit stable-ID `use function` provider edges are the sole
cross-file composition mechanism. Reverse consumers are excluded. Retained
functions keep their real resolved bodies, stable identities, display names,
and source identity origins; there are no imported default-body stubs or
synthetic `main` declarations. Display-name duplicates across modules are valid
because linkage and calls use stable IDs. The linker reconstructs cleanup
inventory and cleanup plans over each linked closure and finally validates HIR
before any backend is invoked.

## CLI and artifacts

`check` with no input checks `./semaprax.toml`; `check semaprax.toml` and
`check --manifest-path <semaprax.toml>` select an explicit manifest. Likewise,
`build` without an input selects `./semaprax.toml`, while `build
semaprax.toml` and `build --manifest-path <semaprax.toml>` select an explicit
manifest. Project builds publish only the `web` package target; native
executable publication, `run`, and a public project test command are held. The
entry and web exports come exclusively from the authenticated manifest, so
`--function` and `--export` are rejected for this route.

The linked entry HIR also feeds internal native C lowering/equivalence evidence,
but the public Project CLI emits only the Web scalar package. That package has
the separate `semaprax.web-project.v1` manifest binding project revision,
Workspace Phase-A revision, entry module, selected exports, and exact artifact
digests. The test closure is retained for project verification evidence; it is
not a general test framework, public test runner, or `semaprax run` target.

Web publication inherits the scalar package's documented fresh-output,
caller-exclusive parent/new-tree contract. Project v1 makes no public native
output or hostile-directory native-publication claim.

A final held-input recheck follows publication. If it detects drift after a
complete digest-bound fresh package was published, the operation reports
`SPX-J103`. The package may remain at its output path; callers must reconcile
the output manifest/artifact digests with the current inputs and must never
delete that package automatically.

| Diagnostic | Meaning |
| --- | --- |
| `SPX-J100` | Canonical manifest/path grammar rejection. |
| `SPX-J101` | A bounded Project v1 input limit was exceeded. |
| `SPX-J102` | Authentication or pre-publication held-input drift rejection. |
| `SPX-J103` | Post-publication held-input drift; reconcile the retained complete package, never delete it automatically. |

## Evidence and nonclaims

Focused local evidence covers canonical/hostile manifest input, exact held
source rechecks, closure selection, duplicate display names, linked native and
Wasm behavior, deterministic Web artifacts, Node consumption, and stable-ID
display rename preservation. The required focused commands are:

```sh
cargo test --locked -p semaprax --all-features --lib project::tests::
cargo test --locked -p semaprax --all-features --test project_cli_v1 -- --test-threads=1
cargo test --locked -p semaprax --test project_manifest_v1
cargo test --locked -p semaprax --test project_backend_equivalence_v1 -- --test-threads=1
```

Exact-head hosted promotion remains pending. Project v1 does not claim general
packages/dependencies, registry or network access, capabilities, aggregate or
resource composition, generics, interface/native imports or `use type` edges,
effects, general multi-file compilation, native output confinement, test
discovery, component output, public native executable publication, public
project run/test commands, repository analysis, provenance, approval, or
production readiness.
