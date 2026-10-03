# Implementation map and handbook maintenance

Use this page when you want to connect a handbook explanation to the code that
implements it, or when you are updating a tutorial after a source change.

## Source baseline

This edition follows `main` at
[`508b851a5fda25002ec27453bb559755a6a0d930`](https://github.com/wavect/semaprax/commit/508b851a5fda25002ec27453bb559755a6a0d930).
The workspace package version at that commit is `0.7.0`. The v0.7.0 prerelease
was published on October 1, 2026; a later development snapshot can share the
same package version. Record both the version and commit when reproducing work.

The implementation links below are pinned to this snapshot. Topic guides link
to the living specifications for ongoing development.

## Follow a source file through the compiler

```text
.spx source → parse → resolve and check → checked HIR → cleanup plan
                                                ↓
                                  queries / interpreter / target build
```

Parsing reads the source structure. Resolution identifies declarations and
types. HIR records the checked program. Cleanup planning defines how owned
values and resources are settled. Queries and execution routes consume these
representations for their different jobs.

A source program, a semantic report, an approved operation, and an executable
package are distinct objects. Keep the producing operation and revision with
each artifact when reviewing a workflow.

## Find the implementation for a topic

| Topic | Code and reference entry points | Handbook guide |
| --- | --- | --- |
| Installed commands and flags | [CLI catalog](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/cli/help.rs) and [dispatch](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/cli_driver.rs) | [Cheatsheet](cheatsheet.md) |
| Single-file execution and stdout publication | [Source execution](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/cli_driver/source_execution.rs) | [First program](../getting-started/first-program.md) |
| Standalone project creation | [Project creator](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/project/create.rs) | [First project](../getting-started/first-project.md) |
| Linked function and ownership profiles | [HIR linker](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/hir/workspace_link.rs) | [Profiles](../projects/profiles.md) |
| Native law declarations and selection | [Law parser](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/native_law_source.rs), [example manifest](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/examples/native-law-project/semaprax.toml), and [proof binding](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/assurance_manifest/law_set/native_proof.rs) | [Laws](../language/laws.md) |
| Agent runtime, live binding, recovery, and migration | [Runtime v2 facade](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/agent_runtime_v2.rs) and [lifecycle contract](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/docs/AGENT-ITERATIVE-LIFECYCLE-V2.md) | [Agent programs](../agents/programs.md) · [Recovery](../agents/recovery.md) |
| Prepared Rust API discovery and replay | [Compiler-owned index](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/rust_api_index/mod.rs) | [Integrations](../projects/integrations.md) |
| Selected Rust signatures, owners, and views | [Rust binding](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/src/native_rust_binding.rs) and [builder crate](https://github.com/wavect/semaprax/tree/508b851a5fda25002ec27453bb559755a6a0d930/crates/semaprax-native-rust-interop-builder) | [Integrations](../projects/integrations.md) |
| Token comparisons and session aggregation | [Report helper](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/scripts/token_report.py) and [measurement helper](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/scripts/token_measurement.py) | [Context performance](../practices/context-performance.md) |
| Persistent checked-HIR reuse | [Cache contract](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/docs/PERSISTENT-SEMANTIC-CACHE-V1.md) | [Context performance](../practices/context-performance.md) |
| Editor configuration and candidate workflows | [Extension guide](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/editors/vscode/README.md) | [VS Code](../getting-started/editor.md) |
| Existing documentation and example regressions | [Documentation harness](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/tests/documentation.rs) and [example index](https://github.com/wavect/semaprax/blob/508b851a5fda25002ec27453bb559755a6a0d930/examples/README.md) | [Testing](../practices/testing.md) |

The [architecture map](https://github.com/wavect/semaprax/blob/main/docs/ARCHITECTURE.md)
provides the wider module map. The
[completion matrix](https://github.com/wavect/semaprax/blob/main/docs/COMPLETION-MATRIX.md)
and [quality gates](https://github.com/wavect/semaprax/blob/main/docs/QUALITY-GATES.md)
identify the relevant implementation gates. Read those gates for their named
subjects and targets rather than inferring a test result from a module name.

## Keep tutorials useful

Begin each lesson with the task it helps the reader complete. Define new terms
at first use and link the glossary for a longer explanation. Give each runnable
example a filename, working directory, command, and expected result.

Use complete standalone modules for first lessons. Label a fragment when it
belongs in a larger project, and provide the matching project files when the
reader is expected to run it. Keep command templates separate from runnable
shell blocks.

When a source change affects the handbook, follow its meaning through parsing,
checking, execution, and the selected target. A documentation update should not
silently change compiler behavior to make an example fit.

## Run the handbook checks

The new checker uses Python 3.10 or newer. From the repository root:

```sh
python3 scripts/test-check-handbook.py
python3 scripts/check-handbook.py --structure-only
python3 scripts/check-handbook.py --compiler /absolute/path/to/semaprax
```

The structure check validates local file links, chapter coverage, fence closure,
and marked example metadata. The compiler-backed run also formats temporary
copies, checks marked standalone modules and the multi-file tutorial, executes
them, and compares the expected output. It never executes shell fences.

Only blocks carrying a `handbook-smoke` or `handbook-project-file` marker are
compiler-backed smoke subjects. Other reference snippets remain contextual
examples. External URLs, rendered browser layout, additional targets, and the
full compiler suite are separate checks.

The Docs workflow runs the structural and checker-unit tests before building the
book. Run the compiler-backed command when changing a marked runnable example;
the structural-only command explicitly reports that execution was skipped.
