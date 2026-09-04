# First contribution

Status: living internal contributor documentation.

Audience: new contributors and coding agents making a first change here.

This page is only the sequenced "what do I actually type" layer. It adds no
rule and repeats no policy. [`AGENTS.md`](../AGENTS.md) owns the operating
invariants and the change protocol, the [development guide](DEVELOPMENT.md)
owns the [read order](DEVELOPMENT.md#read-before-changing-semantics) and the
change-area reference table, [quality gates](QUALITY-GATES.md) owns
verification policy, and the [completion matrix](COMPLETION-MATRIX.md) is the
only status authority. Read those. This walks one change past them in order.

## 1. Pick a change the router keeps small

`changed` narrows the gate set only for path shapes it classifies. Everything
else widens the whole run to `full`. The classifications live in
`src/quality_route.rs`:

| Classification | Paths |
| --- | --- |
| `documentation-truth` | `README.md`, `CHANGELOG.md`, any path under `docs/` ending `.md` |
| `agent-context-economics` | `src/agent_economics.rs`, `tests/agent_economics.rs`, `benchmarks/agent-context-v1/`, `tests/snapshots/agent_context_*`, `tests/snapshots/agent_economics.*` |
| `cli-surface` | `src/cli/`, `src/bin/`, `src/cli_driver.rs`, `src/main.rs`; adds the `test-cli` gate |
| `editor-adapter` | `editors/`; adds the `test-editor` gate |
| `broad-compiler-or-graph-dispatch` | `src/graph.rs` |
| `unmapped-or-wide` | every other path |

So a first change confined to documentation, an example, the CLI surface, or
the editor extension costs the narrow route; any other `.rs` edit costs a
full-workspace run. Cheap first shapes:

- a broken local link, a missing `Status:`/`Audience:` line, or a missing
  `SUMMARY.md` entry — `tests/documentation.rs` already names the failure;
- an example project under `examples/` plus its entry in the `example-checks` and
  `example-fmt` loops in `scripts/quality.sh`; keep the project's `README.md`
  in sync with any new build/run/test instructions.
- one diagnostic's wording together with its regression;
- a module split that lowers an entry in `tests/module-size-budget.tsv`.

Do not start with anything the [non-negotiable
invariants](../AGENTS.md#non-negotiable-invariants) name — evaluation order,
cleanup-plan order, evidence capsules, capabilities. Change protocol item 3
requires parser, canonical formatter, resolver/HIR, verifier, semantic graph,
native backend and Wasm backend to move together, which is not a first change.

## 2. Find the specification that owns the area

Three lookups, in this order:

```sh
rg -n -i '<topic>' docs/DEVELOPMENT.md      # change area -> owning references
rg -n -i '<topic>' docs/QUALITY-GATES.md    # change shape -> minimum evidence
rg --files docs | rg -i '<topic>'           # candidate versioned specs by name
```

Worked example. A diagnostic code is the fastest thing to trace, because the
owner and the emit site both carry it:

```sh
rg -n 'SPX-U105' docs src
```

That reports `docs/EXPLICIT-MUTATION-V1.md` (the owning table entry that
defines the code) and `src/hir/resolve_expr.rs` (the site that emits it).
Wording changes in one without the other are a drift, not a fix.

## 3. Find the completion-matrix rows

```sh
rg -n -i '<topic>' docs/COMPLETION-MATRIX.md
```

Read the row's status word against [status
rules](COMPLETION-MATRIX.md#status-rules) before you claim anything. Local,
hosted, private, public and proof-only evidence are distinct there, and none
implies another. You edit the matrix only if your change moves a row's stated
gate; that is step 8, not now.

## 4. Read the meaning, not the source text

The compiler projects the semantics you are about to change. Use that instead
of reconstructing it from `.spx` text:

```sh
cargo run --locked -p semaprax -- graph examples/meaning.spx
cargo run --locked -p semaprax -- context examples/meaning.spx math.add --depth 1
```

Both print one line of JSON, so pipe them through a filter. `graph` reports the
schema, a source-bound `revision` digest, and a node per declaration; against
`examples/meaning.spx` it opens with
`{"schema":"semaprax.graph.v10","revision":"sha256:...`. The stable id in the
`context` call is a declaration's `@id("...")` — `math.add` above — or any node
`id` the graph printed. `context` additionally reports the `budget` and
`truncation` it applied, so a truncated answer tells you to raise `--depth`
rather than to guess.

Use `rg` and `rg --files` for Rust and host-code navigation. Read
[ADR 0001](decisions/0001-graphify.md) before adding a repository-wide index.

## 5. Put the test in the harness that owns its subject

`tests/` holds harness roots, not one file per case. A new top-level file
statically links the whole compiler again, which is what the harness
convention exists to avoid; [architecture](ARCHITECTURE.md#integration-test-harnesses)
owns the rule and the cases that must stay standalone.

```sh
ls tests/*.rs                 # the harness roots; pick the one owning the subject
ls tests/language/            # that harness's modules
rg -n '^mod |^#\[path' tests/language.rs
```

Add the body as `tests/<group>/<name>.rs`, declare it in `tests/<group>.rs`, and
keep the `#[path]` — a bare `mod foo;` in a test crate root resolves to
`tests/foo.rs`, not into the directory:

```rust
#[path = "<group>/<name>.rs"]
mod <name>;
```

Then run just your module. The `module::` prefix matters, because a bare second
positional is read as a second libtest filter:

```sh
cargo test --locked -p semaprax --test <group> <name>::<case>
```

Two constraints follow from sharing one binary: every fixture prefix in a
harness must be distinct, and a `tests/support/*.rs` file is declared once in
the harness root and used as `crate::<name>`. `tests/harness_isolation.rs`
checks the first.

## 6. Run a profile

Preview the route before running it. `--plan` prints and validates the plan and
exits without dispatching a gate:

```sh
scripts/quality.sh changed --plan
```

The plan is a `semaprax.quality-route.v2` record set naming the effective
profile, the reason it was chosen, and each gate in order. A documentation-only
change set plans as `effective changed` with reason
`complete-git-state-has-narrow-mappings`; add one line to
`src/quality_route.rs` and the same change set plans as `effective full` with
reason `git-state-includes-wide-or-unmapped-path`.

| Profile | Gate ids | What it costs |
| --- | --- | --- |
| `quick` | `diff-check`, `fmt-check`, `check-workspace`, `test-advisory` | A workspace check and the four advisory test targets. No Clippy, no rustdoc, no release build |
| `changed` | `quick`'s four plus `clippy-package`, `test-agent-context`, `rustdoc-package`; then `test-cli` when the change set touches `cli-surface` paths and `test-editor` when it touches `editor-adapter` paths | Adds strict package Clippy, the compiler and agent-context integration targets, and package rustdoc; the CLI harnesses (including the full toolchain's help surface) or the extension's `node --test` run only for their own paths |
| `full` | `diff-check`, `fmt-check`, `check-workspace`, `test-advisory`, `clippy-workspace`, `test-workspace`, `doctest-workspace`, `rustdoc-workspace`, `build-release`, `package`, `example-checks`, `example-fmt` | Adds workspace Clippy, the whole workspace test and doctest run, workspace rustdoc, a release build, the package check, and the canonical example loops. `test-workspace` is the disk hazard below |

`scripts/quality.sh` is the source of truth for each gate's exact command; do
not copy that sequence elsewhere. During a run the script writes each gate name
to standard error before starting it, so a long gate stays attributable.

Two routing surprises to expect from `changed`:

- It needs a base. With none configured it refuses: `changed quality routing
  requires SEMAPRAX_QUALITY_BASE, SEMAPRAX_QUALITY_TARGET_REF, or configured
  origin/HEAD`. `SEMAPRAX_QUALITY_BASE` takes a full commit id that is an
  ancestor of `HEAD`; `SEMAPRAX_QUALITY_TARGET_REF` must be an exact
  `refs/remotes/` reference. With `origin/main` fetched, the simplest working
  form is `SEMAPRAX_QUALITY_BASE=$(git merge-base origin/main HEAD)`.
- On a clean worktree it widens to `full` with reason
  `changed-worktree-is-empty`. Run it after you have edits, not before.

For a documentation-only change, [quality
gates](QUALITY-GATES.md#documentation-changes) names the minimum, and
[change-specific evidence](QUALITY-GATES.md#change-specific-evidence) names the
focused evidence your owning specification adds on top of the profile. Editing
prose is not evidence for a technical claim.

## 7. Local hazards you will otherwise hit

- **A library test aborts on an unmodified tree.**
  `cargo test --locked -p semaprax --lib` overflows the stack in
  `wasm::internal_strings::tests::nesting::nested_if_compile_on_default_stack`
  on a default-stack debug build. It is not your change; skip it with
  `-- --skip nested_if_compile_on_default_stack`. See
  [`CLAUDE.md`](../CLAUDE.md).
- **Disk, not time, is the binding limit on `full`.** A
  `--workspace --all-targets` test build links several hundred integration
  binaries and needs well over 10 GB. Build with `CARGO_INCREMENTAL=0` and
  `CARGO_PROFILE_TEST_DEBUG=0` when disk is short.
- **Never `git stash` in this repository.** The stash is one stack shared by
  every worktree, and dozens are usually registered, so a push or pop reaches
  another agent's uncommitted work. Commit to a scratch branch, or use a
  worktree.
- **Clippy runs with `-D warnings`.** One unused import fails the build. Run
  `cargo fmt --all` before any gate; `fmt-check` is the second gate in every
  profile, so a formatting slip wastes the whole run.
- **1500 lines per Rust file.** `tests/module_size.rs` fails above that unless
  `tests/module-size-budget.tsv` records the file, and a recorded file may not
  grow past its recorded size. Prefer a new submodule. Regenerate the ledger
  only after a legitimate split, with
  `cargo test --locked -p semaprax --test module_size -- --ignored regenerate`.
  Before splitting, `rg` the module's path across `tests/` and `crates/*/src`
  for `include_str!` and path reads: a hit is a [source-locked
  contract](ARCHITECTURE.md#source-locked-contracts) whose join must follow the
  code, or it keeps passing while covering less. Move bodies verbatim — dedenting
  a relocated body silently rewrites the interior of multi-line string literals,
  where leading whitespace is content.
- **Windows checkouts need long paths before cloning**, not after; see
  [Windows checkouts](DEVELOPMENT.md#windows-checkouts).

## 8. What to update at the end

Update exactly the owner of each fact you changed, and nothing else:

| Update | Only when |
| --- | --- |
| [Architecture](ARCHITECTURE.md) | Implementation ownership or a trust boundary moved |
| [Completion matrix](COMPLETION-MATRIX.md) | A row's status or its stated gate changed |
| [Roadmap](ROADMAP.md) | Sequencing changed — never to assert something is done |
| [Changelog](../CHANGELOG.md) | Always: this is where history goes |
| The owning versioned specification | Its exact syntax, schema, ABI, diagnostics or admission changed |

New or renamed `docs/*.md` files also need a `docs/SUMMARY.md` entry, an H1 as
the first line, and `Status:` and `Audience:` within the first 12 lines.
`tests/documentation.rs` enforces links, metadata and catalog coverage.

Do not describe local, private, proof-only, simulator or prior-head evidence as
public, hosted, physical-device, current-head or production support.

## 9. Before you open the pull request

Walk the [change protocol](../AGENTS.md#change-protocol) items in order; it is
the checklist, and this page only sequenced the tooling around it. Then confirm
the two things that belong to this page: the routed profile passed together
with the focused evidence the owning specification names, and nothing outside
the owners in step 8 was edited.
