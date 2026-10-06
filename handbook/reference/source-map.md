# Implementation map and handbook maintenance

Use this page to connect a handbook explanation to the code behind it, and to
keep the handbook complete when a release adds commands or features.

## Source baseline

This edition describes Semaprax 0.9.0, tag
[`v0.9.0`](https://github.com/wavect/semaprax/tree/v0.9.0). Implementation links
below point at that tag. Topic guides link to the living specifications on
`main`. When you reproduce a result, record the version and the commit.

## Follow a source file through the compiler

```text
.spx source → parse → resolve and check → checked HIR → cleanup plan
                                                ↓
                                  queries / interpreter / target build
```

Parsing reads structure. Resolution names declarations and types. HIR records the
checked program. Cleanup planning decides how owned values and resources settle.
Queries and execution routes read those results for their own jobs. A source
program, a semantic report, an approved operation and an executable package are
different objects: keep the producing command and revision with each.

## Find the code for a topic

| Topic | Code and spec | Handbook page |
| --- | --- | --- |
| Commands and flags | [CLI catalog](https://github.com/wavect/semaprax/blob/v0.9.0/src/cli/help.rs), [dispatch](https://github.com/wavect/semaprax/blob/v0.9.0/src/cli_driver.rs) | [Command catalog](commands.md) |
| Single-file run and stdout | [Source execution](https://github.com/wavect/semaprax/blob/v0.9.0/src/cli_driver/source_execution.rs) | [First program](../getting-started/first-program.md) |
| Project creation | [Project creator](https://github.com/wavect/semaprax/blob/v0.9.0/src/project/create.rs) | [First project](../getting-started/first-project.md) |
| Linked functions and profiles | [HIR linker](https://github.com/wavect/semaprax/blob/v0.9.0/src/hir/workspace_link.rs) | [Profiles](../projects/profiles.md) |
| Law declarations and proof binding | [Law parser](https://github.com/wavect/semaprax/blob/v0.9.0/src/native_law_source.rs), [proof binding](https://github.com/wavect/semaprax/blob/v0.9.0/src/assurance_manifest/law_set/native_proof.rs), [example manifest](https://github.com/wavect/semaprax/blob/v0.9.0/examples/native-law-project/semaprax.toml) | [Laws](../language/laws.md) |
| Agent runtime, recovery, migration | [Runtime v2](https://github.com/wavect/semaprax/blob/v0.9.0/src/agent_runtime_v2.rs), [lifecycle](https://github.com/wavect/semaprax/blob/v0.9.0/docs/AGENT-ITERATIVE-LIFECYCLE-V2.md) | [Agent programs](../agents/programs.md), [Recovery](../agents/recovery.md) |
| Rust API index and bindings | [Index](https://github.com/wavect/semaprax/blob/v0.9.0/src/rust_api_index/mod.rs), [binding](https://github.com/wavect/semaprax/blob/v0.9.0/src/native_rust_binding.rs), [builder](https://github.com/wavect/semaprax/tree/v0.9.0/crates/semaprax-native-rust-interop-builder) | [Integrations](../projects/integrations.md) |
| Token reports | [Report helper](https://github.com/wavect/semaprax/blob/v0.9.0/scripts/token_report.py), [measurement](https://github.com/wavect/semaprax/blob/v0.9.0/scripts/token_measurement.py) | [Context performance](../practices/context-performance.md) |
| Semantic cache | [Cache contract](https://github.com/wavect/semaprax/blob/v0.9.0/docs/PERSISTENT-SEMANTIC-CACHE-V1.md) | [Context performance](../practices/context-performance.md) |
| Registry front | [Registry CLI](https://github.com/wavect/semaprax/blob/v0.9.0/src/cli/registry.rs), [registry rules](https://github.com/wavect/semaprax/tree/v0.9.0/src/package_registry) | [Shipping](../projects/shipping.md), [Trust](../tools/trust.md) |
| Audit capsules and workflows | [Capsule](https://github.com/wavect/semaprax/blob/v0.9.0/src/audit_capsule.rs), [workflow engine](https://github.com/wavect/semaprax/tree/v0.9.0/src/typed_workflow) | [Shipping](../projects/shipping.md), [Trust](../tools/trust.md) |
| Harness | [Harness crate](https://github.com/wavect/semaprax/tree/v0.9.0/crates/semaprax-harness), [adapters](https://github.com/wavect/semaprax/tree/v0.9.0/packages/semaprax-harness-adapters) | [Harness](../tools/harness.md) |
| Editor | [Extension guide](https://github.com/wavect/semaprax/blob/v0.9.0/editors/vscode/README.md) | [VS Code](../getting-started/editor.md) |
| Documentation tests | [Harness](https://github.com/wavect/semaprax/blob/v0.9.0/tests/documentation.rs), [examples](https://github.com/wavect/semaprax/blob/v0.9.0/examples/README.md) | [Testing](../practices/testing.md) |

The [architecture map](https://github.com/wavect/semaprax/blob/main/docs/ARCHITECTURE.md)
has the full module map. The
[completion matrix](https://github.com/wavect/semaprax/blob/main/docs/COMPLETION-MATRIX.md)
and [quality gates](https://github.com/wavect/semaprax/blob/main/docs/QUALITY-GATES.md)
say what is implemented and how it is tested. Read a gate for its named subject
and target; a module name does not prove a test result.

## Write pages that stay true

- Open each page with what the reader can do after it. Lead with a runnable
  example, then explain it.
- Define a term at first use and link the [glossary](glossary.md).
- Give a runnable example a file name, a working directory, a command and its
  output. Keep command templates in their own block and say which parts to replace.
- Label private, experimental, preview and main-only features as such.
- When source changes, follow the change through the parser, the checker, the
  runner and the target. Never change compiler behavior to make an example fit.

## Run the handbook checks

Python 3.10 or newer, from the repository root:

```sh
python3 scripts/test-check-handbook.py
python3 scripts/check-handbook.py --structure-only
python3 scripts/check-handbook.py --compiler /absolute/path/to/semaprax
```

The structure check validates local links, that every page has one `SUMMARY`
entry, fence closure and example markers. The compiler-backed run also formats
temporary copies of marked modules, checks and runs them, and compares the output.
Only blocks marked `handbook-smoke` or `handbook-project-file` run. Shell fences
never do. Marked examples must pass in the interpreter, so a program that needs
`--native` stays unmarked.

## Keep the handbook complete for each release

Run this audit before each release. A release is not documented until every public
command and every user-visible feature has a page that is true for that version.

**1. Commands.** List what the binary accepts and find names no page mentions:

```sh
semaprax help all | grep '^semaprax ' | cut -d' ' -f2 | sort -u > commands.txt
for c in $(cat commands.txt); do
  grep -rqE "(^|[^a-z-])$c([^a-z-]|\$)" handbook --include='*.md' || echo "missing: $c"
done
```

The 0.9.0 audit lists 132 commands and no missing name. [Command catalog](commands.md)
spells every command in full, so a new command must be added there with its
page. A hit in `grep` shows a mention, not an explanation; open the page.
Also read `crates/semaprax-harness/src/cli.rs` for harness verbs, which `help all`
does not list.

**2. Features.** Read the new rows and status changes in the
[completion matrix](https://github.com/wavect/semaprax/blob/main/docs/COMPLETION-MATRIX.md)
and the release section of the
[changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md). Give each
user-visible item a row in the tables below.

**3. Language and library.** Compare `semaprax help language` and `semaprax help
library` with [Cheatsheet](cheatsheet.md), [Built-in functions](builtins.md) and
[Standard library](stdlib.md). `std/catalog.json` lists every package. Regenerate
the package table in the standard-library page when it changes.

**4. Diagnostics.** Run `semaprax help diagnostic codes` and add new indexed codes
to [Diagnostics reference](diagnostics.md).

**5. Examples.** Run the compiler-backed check above.

### Coverage by area (0.9.0)

"Page" is where a reader learns it. "Overview" means one section or a short entry
with a link to the spec. "Not in the handbook" means private, internal or not shipped.

| Area | Page | Depth |
| --- | --- | --- |
| Install by archive, Homebrew, release verify | [Install](../getting-started/install.md), [Shipping](../projects/shipping.md) | Full for 0.8.0 archives. The 0.9.0 installers, five targets and WinGet are pending the post-release install rewrite. |
| First program, project, editor, Configure Compiler | [Getting started](../getting-started/first-program.md), [VS Code](../getting-started/editor.md) | Full |
| Language: scalars, control flow, records, variants, classes, generics, closures, matching, loops, iterators, collections, Box | [Language chapters](../language/essentials.md) | Full |
| Ownership, borrow, strings, bytes, resources, cleanup, `unsafe`, session protocols, laws | [Ownership](../language/ownership.md), [Resources](../language/resources.md), [Contracts and effects](../language/contracts-effects.md), [Laws](../language/laws.md) | Full |
| Effects and I/O: stdout, args, stdin, files, TCP | [Input and output](../language/io.md) | Full |
| Effects and I/O: TLS, listeners, HTTPS, `stdout_append`, checked atomic write | [Built-in functions](builtins.md) | Overview |
| Projects, manifests, profiles, modules, targets (native, web, wasm, npm, oci, native-callable) | [Projects chapters](../projects/modules.md) | Full |
| Hot reload (`dev`) | [Targets](../projects/targets.md#edit-and-re-run-hot-reload) | Full |
| Lock, resolve, add, fetch, packages, registry, audit, workflow, release verify | [Shipping](../projects/shipping.md), [Trust](../tools/trust.md) | Full |
| Semantic changes: preview, rebase, merge, patch, evidence, receipts, workspaces, candidates, images | [Shipping](../projects/shipping.md), [Explore](../practices/explorer.md) | Full |
| Servers: `serve`, `service`, MCP, image protocols, host policy, `semapraxd` | [Shipping](../projects/shipping.md), [Specialist commands](../tools/specialist-commands.md) | Full |
| Query, context, graph, doc, compact, cache | [Agents](../practices/agents.md), [Context performance](../practices/context-performance.md) | Full |
| Agent programs: declare, run, route, budgets, journals, checkpoints, migration | [Agent programs](../agents/programs.md), [Recovery](../agents/recovery.md) | Full |
| Agent harness, adapters, bridge, routing | [Harness](../tools/harness.md) | Full, labeled development tooling |
| C, C++, OpenAPI, Rust, freestanding | [Integrations](../projects/integrations.md) | Full |
| Capability manifest, protocol check, SIMD, region report, hygienic generation, plugin manifest, UI schema | [Specialist commands](../tools/specialist-commands.md) | Overview |
| WIT and components | [Specialist commands](../tools/specialist-commands.md) | Overview, labeled not a product |
| Assurance policy, proofs, property tests | [Shipping](../projects/shipping.md), [Laws](../language/laws.md), [Testing](../practices/testing.md) | Full |
| Standard library | [Standard library](stdlib.md) | All 47 packages listed |
| Diagnostics and fixes, `fix`, `repair` | [Diagnostics](diagnostics.md), [Debugging](../practices/debugging.md) | Full |
| Doctor, version, quality plan | [Targets](../projects/targets.md#check-the-environment), [Specialist commands](../tools/specialist-commands.md) | Full |
| Retention metadata stores | [Specialist commands](../tools/specialist-commands.md) | Overview |
| Structured concurrency (Rust scoped-thread runtime) | `std.async` in [Standard library](stdlib.md) | Not in the handbook as a language feature |
| Java/Kotlin, Swift/Apple bridges, UI runtimes for iOS, Android, desktop, public generic signatures, ARC zones | None | Not shipped; see the completion matrix |
| LAW16 campaigns, CI repairs, kernel and bootstrap documents | None | Internal evidence |

### What 0.9.0 added

| Changelog item | Page |
| --- | --- |
| One-command installers, version pinning, receipts, uninstall | [Install](../getting-started/install.md), pending rewrite |
| `aarch64-unknown-linux-gnu` and `x86_64-apple-darwin` archives, glibc 2.35 baseline | [Install](../getting-started/install.md), pending rewrite |
| Homebrew tap (covered), WinGet manifests (pending) | [Install](../getting-started/install.md) |
| VS Code Configure Compiler and status item | [VS Code](../getting-started/editor.md) |
| Hot-reload stdin and framing fixes | [Targets](../projects/targets.md#edit-and-re-run-hot-reload) |
| Harness fixes and model routing (MR-00 to MR-15), `choice-select/v1`, `harness status --routing` | [Harness](../tools/harness.md), [Agent programs](../agents/programs.md) |
| New `help:` hints and diagnostics (`SPX-U103`, `SPX-O116`, `SPX-T207`, `SPX-F102` and others) | [Diagnostics](diagnostics.md), [Debugging](../practices/debugging.md) |
| Native operand read order fix for `let mut` (#561) | None; a bug fix |
