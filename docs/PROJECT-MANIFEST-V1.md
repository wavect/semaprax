# Project Manifest v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: language users, tool authors, and compiler contributors.

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
web_exports = ["calculator.add", "calculator.divide", "calculator.is-negative", "calculator.multiply", "calculator.not", "calculator.subtract"]
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

- a module is permit-free and its functions effect-free, unless every one of
  those permits and effects is declared by a retained Native Rust import;
- there are no authored types, generic templates or instances, or `use type`
  edges, and the only admitted interface declaration is one whose imports are
  all `import rust fn` callbacks; an ordinary interface import has no scalar
  calling convention and is rejected;
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

A Project v1 entry closure that declares a Native Rust callback is admitted
through a separate route that derives no target at all. WebAssembly rejects
native Rust imports (`SPX-W114`) and the ordinary native backend cannot lower a
callback call site, so such a Project has no Web target and no scalar Web
module. Its admission instead proves, under `SPX-J117`, that every selected
export names an explicitly identified monomorphic function, that the selected
identities are in canonical manifest order with no duplicate, that each has at
most eight by-value `i64`/`bool` parameters and an `i64`/`bool` result, and
that every effect it declares is granted by a declared callback. A Project that
declares no callback keeps the previous route unchanged: the scalar Web module
is still emitted and admitted, byte for byte.

Because no Web artifact exists for this shape, that admission derives no scalar
WIT descriptor, and the public scalar WIT accessor fails closed with
`SPX-J105`. The manifest field is still spelled `web_exports`. For a callback
Project it names the exported functions selected for the generated Rust SDK
even though the Project has no Web target; the field name is frozen v1 grammar
while its meaning now depends on the admitted route, which is a wart rather
than a claim of Web support. The only consumer of such a Project is the
generated C and safe Rust bridge that the Native Rust SDK builder renders from
the linked HIR.

## CLI and artifacts

`check` with no input checks `./semaprax.toml`; `check semaprax.toml` and
`check --manifest-path <semaprax.toml>` select an explicit manifest. Likewise,
`build` without an input selects `./semaprax.toml`, while `build
semaprax.toml` and `build --manifest-path <semaprax.toml>` select an explicit
manifest. For `check`, `run`, `test`, `build`, and `fmt`, a positional operand
that names an existing directory selects the `semaprax.toml` inside it, with
inert `.` components removed; `--manifest-path` is never resolved this way, and
a directory without a manifest reports `SPX-J102` for that path. `fmt <dir>`
and `fmt semaprax.toml` read only the manifest, not the authenticated project:
they format every `sources` entry in manifest order through the single-file
comment-preserving projection, parse every file before writing any, and with
`--check` print one `<path> is not canonically formatted` line per drifting
file and exit one; a manifest that cannot be read is reported as `cannot read
<path>`. Project builds publish two explicit targets:

- `--target web` (the default) publishes the digest-bound scalar Web package;
- `--target native` publishes one linked entry-closure executable compiled by
  the same held Clang C11 pipeline as the single-file native lane.

`check` retains its parsed source or Project selector through dispatch.
For a single source, `check --json file.spx` and `check file.spx --json`
select the same file and diagnostic mode; the option token is never reparsed
as a source path. Project selection and its default manifest remain unchanged.
On Windows, an extensionless Project-native output receives exactly one `.exe`
extension. Explicit existing extensions are retained; Unix names are unchanged.
The routing and naming regressions in `tests/cli_check_routing_v1.rs` and
`src/cli/native_output_tests.rs` are authored but unrun.

`run` and `test` execute in process from the already authenticated linked HIR.
They do not emit C or Wasm, create a temporary executable, spawn a process,
reparse sources, relink declarations, or create project state. `run` evaluates
the exact entry-module `main` and prints its `i64` result. `test` evaluates the
manifest-declared test-module `main`, and then each zero-parameter `i64`
function of that module whose name starts with `test_` as a named case; zero
passes and any nonzero result fails. There is no filesystem test discovery.
[Project Test Cases v1](PROJECT-TEST-CASES-V1.md) owns the case rule, the
human report, the additive `cases` array, and the contract-failure detail that
accompanies a language failure. Both commands distinguish a
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
parsing or re-resolution. Project v5 and v6 dispatch their manifest-selected
command stable ID through the fixed native-command process compiler; they do
not fall back to the ordinary `main`. The destination must not exist
(`SPX-I307`), so a native build never clobbers an existing file; the immediate
pre-check window is trusted and is not a hostile-window publication contract.

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

The explicitly injected [Project Revision Store
v1](PROJECT-REVISION-STORE-V1.md) may persist the exact canonical manifest and
source inputs of an already authenticated authority-neutral revision. Loading
independently replays the content-addressed inventory and rebuilds the ordinary
Project Phase-A/HIR subject; it is neither a serialized-verifier bypass nor a
default daemon cache. Its Unix root must be current-euid-owned, exact `0700`,
and host-exclusive against uncooperative same-principal mutation for the whole
invocation; its advisory lock coordinates cooperating callers only. Project
Manifest v1-v10 and Transport v2-v5 wire bytes remain unchanged.

Web publication inherits the scalar package's documented fresh-output,
caller-exclusive parent/new-tree contract.

A final held-input recheck follows publication. If it detects drift after one
complete package or executable was published, the operation reports `SPX-J103`.
The output may remain at its output path; callers must reconcile the retained
output with the current inputs and must never delete it automatically.

When a listed source imports a module that no listed source declares, the
Workspace Semantic Graph reports `SPX-G172` "target module is missing or
equals the caller module" at the `use`. The project loader keeps that code,
message, and span and adds a `help` line: when an unlisted `.spx` file in a
directory that holds a listed source declares the module, the help names that
file and the `sources` key (`` `src/util.spx` declares module `app.util` but is
not listed under `sources` in semaprax.toml; add it there ``); when no such file
exists, it says that no listed file declares the module; when the module
imports from itself, it says so. The scan reads at most 512 `.spx` files of at
most 1 MiB each, runs only after the build has already failed, and produces
advisory text only. Human and `--json` output carry the same `help`.
`tests/project_cli_v1.rs::unresolved_import_hint_names_the_unlisted_source_file`
is the gate.

| Diagnostic | Meaning |
| --- | --- |
| `SPX-J100` | Canonical manifest/path grammar rejection. |
| `SPX-J101` | A bounded Project v1 input limit was exceeded. |
| `SPX-J102` | Authentication or pre-publication held-input drift rejection. |
| `SPX-J103` | Post-publication held-input drift; reconcile the retained complete package, never delete it automatically. |
| `SPX-J117` | Native Rust callback admission rejected a selection, signature, identity, ordering, or ungranted effect. |
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
cargo test --locked -p semaprax --all-features --test project developer_loop:: -- --test-threads=1
cargo test --locked -p semaprax --all-features --test project native_publication:: -- --test-threads=1
cargo test --locked -p semaprax --test project language_command_native:: -- --test-threads=1
cargo test --locked -p semaprax --test project_manifest_v1
cargo test --locked -p semaprax --test project backend_equivalence:: -- --test-threads=1
cargo test --locked -p semaprax-native-rust-interop --test project_sdk_cli
```

The additive local Project Native Rust SDK v1 evidence uses the manifest's
exact `web_exports` set as the generated Rust facade and binds the canonical
manifest, Project/workspace/graph revisions, every declared source fact, and
each export's declaration origin before invoking the existing SDK builder over
the already linked entry HIR. Its end-to-end gate builds and runs the
calculator Project through Web/Node and Rust consumers before and after the
opt-in daemon rename and explicit shutdown:

```sh
SEMAPRAX_REQUIRE_PROJECT_NATIVE_RUST_SDK=1 cargo test --locked -p semaprax --test project agent_transport_rename::project_rename_transaction_refreshes_the_exact_project_and_preserves_web_api -- --nocapture
```

The unpublished builder workspace binary exposes that same authenticated
route as:

```sh
cargo run --locked --offline -p semaprax-native-rust-interop \
  --bin semaprax-native-rust-sdk -- project \
  --manifest-path "$(pwd)/examples/calculator-project/semaprax.toml" \
  --output "/fresh/absolute/output"
```

Its exact-one options and canonical
`semaprax.project-native-rust-sdk-result.v1` result do not add a root
`semaprax build --target rust` route. Separately, the local browser harness
authenticates exact known-answer baseline and display-renamed Project subjects,
requires their exact six-function scalar ABI and six non-manifest generated
artifacts to remain byte-identical, and runs the unchanged calculator shell
against both plus the direct-source package.

That evidence is local only. It proves revision-bound stable-ID behavior, not
general whole-package byte equality across arbitrary changes, a root Project
CLI Rust target, an installed/public CLI, or
general Project/package/import/capability/aggregate/resource support.

The Project v1 matrix and its additive native publication/Project Rust SDK
lanes are exact-tag hosted green at v0.2.0 commit
`5f6fb9655fdec92c57ab71615cfd7bfa8cc76051`. The blocking Project Manifest jobs
passed on [Ubuntu](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100195944533),
[macOS](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100195941349),
and [Windows](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100195908281),
and the complete Product Acceptance jobs passed on
[Ubuntu](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100195951104),
[macOS](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100195940394),
and [Windows](https://github.com/wavect/semaprax/actions/runs/33608662244/job/100195908639),
including `project/native_publication` and
the Project Native Rust SDK gate. This proves only the selected lanes at the
exact tag; it does not publish or promote Project-v8/v9/v10 packages. Project
v1 does not
claim general packages/dependencies, registry or network access, capabilities,
aggregate or resource composition, generics, ordinary interface imports, an
admitted target for a declared Native Rust callback, or
`use type` edges, general multi-file compilation, native output
confinement or hostile-window no-clobber publication, cross-build executable
byte determinism, test discovery, component output, target execution through
the in-process runner, repository analysis, provenance, approval, or
production readiness. Exact-head hosted promotion for the developer loop is
limited to the bounded Transport-v4 workflow exercised by the Product
Acceptance jobs cited above.

## Additive Project Manifest v7 line-command profile

V7 preserves v1-v6 canonical bytes and adds exactly
`profile = "line-command-io.v1"`. It requires the existing
`argv-utf8+stdin-bytes.v1` input, one explicit `() -> bool` command stable ID,
and the sorted args-read, stdin-read, stderr-write, and stdout-write capability
inventory. Compiler-owned fallible `byte_range` and cumulative
`stdout_append`/`stderr_append` share an exact 65,536-byte output envelope and
publish both semantic transcripts only with a settled terminal result.

Range meaning selects Graph v20 when no later schema is required, and keeps
CleanupPlan v4. The committed line-filter's nonempty Shared Loan Plan selects
Graph v23 while retaining its exact byte-range and command-I/O facts.
Core-Wasm uses private invocation-local descriptors bound to an exact root
token, offset, and length; they are neither public pointers nor owned tokens.
The npm artifact is bound by
independently replayed `semaprax.project-npm-build.v6`. Cross-module imports may
add only `borrow Slice<u8>` parameters to an otherwise admitted monomorphic
signature whose result is a non-borrowing scalar.

Focused local evidence is `examples/spxgrep-lines-project` plus its interpreter,
native, and Core-Wasm/Node tests. It does not claim real-browser or multi-engine
execution, general streaming, files,
WASI, physical cross-descriptor atomicity, persistence, safe Windows npm
publication, registry publication, or exact-head hosted promotion.
