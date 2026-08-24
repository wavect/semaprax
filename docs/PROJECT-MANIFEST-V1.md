# Project Manifest v1

Project Manifest v1 is a bounded, invocation-local way to check, execute, test,
or build one explicit
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
manifest. Project builds publish two explicit targets:

- `--target web` (the default) publishes the digest-bound scalar Web package;
- `--target native` publishes one linked entry-closure executable compiled by
  the same held Clang C11 pipeline as the single-file native lane.

`run` and `test` execute in process from the already authenticated linked HIR.
They do not emit C or Wasm, create a temporary executable, spawn a process,
reparse sources, relink declarations, or create project state. `run` evaluates
the exact entry-module `main` and prints its `i64` result. `test` evaluates only
the manifest-declared test-module `main`; zero passes and any nonzero result
fails. There is no filesystem test discovery. Both commands distinguish a
language failure, fuel exhaustion, and call-depth exhaustion, and `--json`
emits a deterministic `semaprax.project-execution.v1` envelope binding the
project and Workspace revisions, closure role and module, stable entry ID,
fuel accounting, outcome, nonclaims, and a domain-separated payload digest.
The public `project::verify_execution_envelope` route independently enforces
the closed schema, semantic bounds, status/outcome vocabulary, exact canonical
reconstruction, and digest before accepting report bytes; verification grants
no execution or other authority.

The entry and web exports come exclusively from the authenticated manifest, so
`--function` and `--export` are rejected for this route, as is the
`native-callable` target.

Native publication compiles exactly the linked entry HIR that Web publication
and internal native lowering-equivalence evidence consume; it performs no
parsing or re-resolution. The destination must not exist (`SPX-I307`), so a
native build never clobbers an existing file; the immediate pre-check window is
trusted and is not a hostile-window publication contract.

The linked entry HIR also feeds internal native C lowering/equivalence evidence.
The Web package has the separate `semaprax.web-project.v1` manifest binding
project revision, Workspace Phase-A revision, entry module, selected exports,
and exact artifact digests. The test closure is retained for the bounded
project runner and backend equivalence evidence; it is not a general test
framework or discovery system.

Additive [Project Agent Transport v2](PROJECT-AGENT-TRANSPORT-V2.md) retains the
same one-time Phase-A products inside a sequential `semapraxd --stdio` process:
entry/test HIR, one complete declared-project graph, and one typed context
index. It reauthenticates held inputs around every revision-bound semantic
response and grants no build or mutation authority.

Web publication inherits the scalar package's documented fresh-output,
caller-exclusive parent/new-tree contract.

A final held-input recheck follows publication. If it detects drift after one
complete package or executable was published, the operation reports `SPX-J103`.
The output may remain at its output path; callers must reconcile the retained
output with the current inputs and must never delete it automatically.

| Diagnostic | Meaning |
| --- | --- |
| `SPX-J100` | Canonical manifest/path grammar rejection. |
| `SPX-J101` | A bounded Project v1 input limit was exceeded. |
| `SPX-J102` | Authentication or pre-publication held-input drift rejection. |
| `SPX-J103` | Post-publication held-input drift; reconcile the retained complete package, never delete it automatically. |
| `SPX-F106` | Project execution report verification rejected noncanonical, confused, out-of-bounds, or digest-invalid bytes. |

## Evidence and nonclaims

Focused local evidence covers canonical/hostile manifest input, exact held
source rechecks, closure selection, duplicate display names, linked native and
Wasm behavior, deterministic Web artifacts, Node consumption, and stable-ID
display rename preservation. Public Project Native Publication v1 adds
explicit create-new native publication evidence: CLI admission and exact output
naming, linked-entry execution, replay behavior, pre-publication drift
rejection before any output exists, post-publication `SPX-J103` uncertainty
that preserves the executable, existing-destination rejection, deterministic
entry C projections, and stable-ID display rename preservation. The required
focused commands are:

```sh
cargo test --locked -p semaprax --all-features --lib project::tests::
cargo test --locked -p semaprax --all-features --test project_cli_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test project_developer_loop_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test project_native_publication_v1 -- --test-threads=1
cargo test --locked -p semaprax --test project_manifest_v1
cargo test --locked -p semaprax --test project_backend_equivalence_v1 -- --test-threads=1
```

The additive local Project Native Rust SDK v1 evidence uses the manifest's
exact `web_exports` set as the generated Rust facade and binds the canonical
manifest, Project/workspace/graph revisions, every declared source fact, and
each export's declaration origin before invoking the existing SDK builder over
the already linked entry HIR. Its end-to-end gate builds and runs the
calculator Project through Web/Node and Rust consumers before and after the
opt-in daemon rename and explicit shutdown:

```sh
SEMAPRAX_REQUIRE_PROJECT_NATIVE_RUST_SDK=1 cargo test --locked -p semaprax --test project_agent_transport_rename_v1 project_rename_transaction_refreshes_the_exact_project_and_preserves_web_api -- --nocapture
```

That evidence is local only. It proves revision-bound stable-ID behavior, not
whole-package byte equality across the rename, a Project CLI Rust target, or
general Project/package/import/capability/aggregate/resource support.

The exact `d883ace579bfd86f723cdc6819224fde51f0677d` Project v1 matrix is
hosted green in [run 32523952912](https://github.com/wavect/semaprax/actions/runs/32523952912),
including [Ubuntu](https://github.com/wavect/semaprax/actions/runs/32523952912/job/96901973139),
[macOS](https://github.com/wavect/semaprax/actions/runs/32523952912/job/96901973190),
and [Windows](https://github.com/wavect/semaprax/actions/runs/32523952912/job/96901973112).
That run predates native publication and the Project Native Rust SDK; the new
lanes additionally require an exact-head hosted Ubuntu/macOS/Windows matrix
that includes `project_native_publication_v1` and the Project Native Rust SDK
gate before any hosted claim for those lanes. Project v1 does not
claim general packages/dependencies, registry or network access, capabilities,
aggregate or resource composition, generics, interface/native imports or
`use type` edges, effects, general multi-file compilation, native output
confinement or hostile-window no-clobber publication, cross-build executable
byte determinism, test discovery, component output, target execution through
the in-process runner, repository analysis, provenance, approval, or
production readiness. The developer-loop evidence is local only until an
exact-head hosted matrix includes it; no hosted promotion is claimed here.
