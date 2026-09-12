# SEMAPRAX agent guide

SEMAPRAX is an agent-native systems programming language: **meaning in,
verified machine code out**. Human-readable `.spx` source is the canonical Git
projection; the versioned semantic graph is the preferred agent interface.

This file contains repository operating invariants. The internal documentation
map and change protocol live in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md); the
module map lives only in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Subagent model selection

Prefer a cheaper, less capable model for delegated work whenever it can handle
that bounded task reliably. Use Luna for straightforward tests, documentation,
and mechanical changes; use Terra for implementation that needs more reasoning.
Reserve the strongest models for complex semantic work, difficult debugging, or
review where a cheaper model is insufficient. Give subagents concise, relevant
context and escalate only when needed. This preference applies across sessions
unless the user explicitly overrides it.

## Read order

Before changing semantics, read:

1. [RFC 0001](docs/RFC-0001.md) for the language and toolchain contract.
2. [Completion matrix](docs/COMPLETION-MATRIX.md) for affected product rows.
3. [Architecture](docs/ARCHITECTURE.md) for data flow and trust boundaries.
4. [Quality gates](docs/QUALITY-GATES.md) for required verification.
5. The versioned specification that owns the changed syntax, protocol, ABI,
   report, workspace route, or target profile.

The [development guide](docs/DEVELOPMENT.md#read-before-changing-semantics)
maps change areas to their additional required references. The
[roadmap](docs/ROADMAP.md) is sequencing, not a reduction of the full goal.

## Non-negotiable invariants

- A safe source program has equivalent checked behavior on every backend that
  claims to implement the admitted feature.
- Evaluation order is left to right. Lazy boolean operands execute only when
  required.
- Public declarations have persistent `@id` identities. Expression identities
  may be revision-scoped.
- Source formatting, graph JSON, Wasm bytes, diagnostics, semantic patches,
  and contracted generated artifacts are deterministic.
- Failed or stale semantic transactions leave authoritative source unchanged.
- A successful managed-workspace transaction publishes one complete immutable
  generation through `ACTIVE`. It does not rewrite original source files or
  grant atomic visibility to Git, editors, or arbitrary raw-path readers.
- Evidence capsules carry no authority. Evidence-gated routes acquire their
  ordinary lock/authority first, replay before staging or candidate creation,
  and let only the live invocation perform the final commit or `ACTIVE` pivot.
- Semantic impact and review are read-only and bound to exact source and patch
  bytes. Source drift fails closed.
- Capabilities are explicit. Compiler and generated code gain no ambient
  filesystem, process, network, home, secret, key, wallet, or signing authority.
- Ownership errors are compile-time diagnostics, never backend accidents.
- Cleanup inventory order is structural metadata. Cleanup-plan vectors are
  canonical runtime order and must never be sorted or repaired downstream.
- An owned call stages arguments left to right and transfers them together at
  its declared commit boundary.
- Failure selection is sticky. Cleanup cannot replace the selected status, and
  result publication follows postconditions and non-result cleanup.
- A settlement or concurrency model is proof data, not permission to perform a
  physical finalizer, spawn runtime work, or publish an artifact.
- No feature is “implemented” without the completion matrix's executable gate.

## Change protocol

1. Identify affected completion rows, invariants, and owning specifications.
2. Add a success case and stable diagnostic regression before or with the
   implementation.
3. When syntax carries runtime meaning, update parser, canonical formatter,
   resolver/HIR, verifier, semantic graph, native backend, and Wasm backend
   together.
4. Exercise both human and agent projections: canonical round-trip plus graph
   assertions.
5. Run `scripts/quality.sh full` on Unix, or reproduce the full profile in
   [Quality gates](docs/QUALITY-GATES.md).
6. Update architecture only for implementation ownership or trust-boundary
   changes, the matrix only for status/gate changes, the roadmap only for
   sequencing, and the changelog for history.

## Repository navigation

Writing or fixing `.spx` source starts with the compiler-checked
[agent quick reference](docs/AGENT-QUICK-REFERENCE.md): the admitted shapes,
the diagnostics that habits from other languages trigger, and their fixes, in
one page. Read the tour or an RFC only for a rule the reference does not state.

Use the repository's semantic tools for bounded questions about one declaration
rather than reconstructing SEMAPRAX meaning from source text:

```sh
cargo run --locked -p semaprax -- context <file> <stable-id> --depth 1 --filters contracts,ownership --max-bytes 4096
cargo run --locked -p semaprax -- graph <file>
```

`context` answers one question within a byte budget. `graph` emits the whole
module including cleanup plans and expression trees, roughly forty times the
source bytes on the committed examples; read the source instead when it fits,
and reserve `graph` for tools and snapshot gates that need the complete
document.

Use bounded source tools such as `rg` and `rg --files` for Rust and host-code
navigation. Read [ADR 0001](docs/decisions/0001-graphify.md) before adding a
repository-wide graph index.

A Rust source file may not exceed 1500 lines unless `tests/module-size-budget.tsv`
records it, and a recorded file may not grow past its recorded size. Prefer a new
submodule over a larger file. [Architecture](docs/ARCHITECTURE.md#module-size)
owns the rule and the two standing exceptions.

Add an integration test as a module of the harness that owns its subject, not as
a new top-level file in `tests/`. Each top-level file is a separate binary that
statically links the whole compiler.
[Architecture](docs/ARCHITECTURE.md#integration-test-harnesses) owns the
convention and the cases that must stay standalone.

## Prohibited shortcuts

- Do not edit generated files under `target/` or commit tool caches.
- Do not run `git stash` here. The stash is a single stack shared by every
  worktree of this repository, and dozens are usually registered, so a push or
  pop reaches another agent's uncommitted work rather than your own. Commit to a
  scratch branch, or use a worktree, instead.
- Do not point a worktree's Cargo `target-dir` (in `.cargo/config.toml` or
  `CARGO_TARGET_DIR`) at another worktree's `target/` or at any path a different
  checkout's tests depend on. Cargo fingerprints by source path, so one
  worktree's build of older source overwrites the other's binaries and
  fingerprints mid-test. When several agents build on one machine, use a
  private `CARGO_TARGET_DIR` under the worktree's own `target/`;
  [development](docs/DEVELOPMENT.md#verification) gives the variables.
- Do not introduce build-time network access or ambient authority.
- Do not bypass verification in a backend or report generator.
- Do not sort, repair, or reinterpret canonical cleanup plans downstream.
- Do not weaken a test, diagnostic, golden, or hostile-input case merely to
  make a gate pass. Relocating audited source weakens one silently: a gate that
  reads a module's text keeps passing against the smaller root while covering
  less. Splitting a module means joining its submodules back into every such
  contract.
- Do not dedent relocated code. Moving a body out of an inline module removes a
  level of indentation from the interior lines of multi-line string literals,
  where leading whitespace is content rather than formatting. Move bodies
  verbatim and let `cargo fmt` reindent; it never rewrites literal contents.
- Do not describe private, local, proof-only, simulator, or prior-head evidence
  as public, hosted, physical-device, current-head, or production support.

<!-- graft:start -->
## Graft — repo context graph

The graph in `graft/` is a per-developer local cache and is NOT committed --
run `graft build src`, then move `src/graft` to `./graft`, before your first
query. It covers `src/` only; use `rg` for `tests/`, `docs/`, `std/`,
`examples/` and `scripts/`, and treat it as a snapshot, not authority.

This repo is indexed in `graft/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here — understanding how something works, finding where code lives,
or scoping a change — get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `graft map` first — a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `graft ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer — cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete — run `graft grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `graft skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `graft callers <symbol>` gives precomputed, exact edges — who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `graft/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects —
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `graft ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to — never
re-read whole files.

After big code changes, refresh the graph with `graft build` (deterministic,
no API key, $0).
<!-- graft:end -->
