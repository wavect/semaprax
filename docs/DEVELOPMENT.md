# SEMAPRAX development documentation

Status: living internal contributor documentation.

Audience: compiler contributors, maintainers, reviewers, and coding agents.

This page is the internal documentation entry point. Public users should start
with the [documentation overview](index.md). Versioned specifications remain
publicly readable, but documents marked private, proof-only, or internal do not
describe supported product surfaces.

New contributors and coding agents should read [first
contribution](FIRST-CONTRIBUTION.md) alongside this page. It states no rule of
its own: it supplies the concrete, ordered commands for a single change against
the read order and change protocol this page owns.

## Read before changing semantics

Read only the documents that own the facts relevant to the change:

1. [RFC 0001](RFC-0001.md) for the long-term language and toolchain contract.
2. [Completion matrix](COMPLETION-MATRIX.md) for the affected product rows and
   their remaining completion gates.
3. [Architecture](ARCHITECTURE.md) for stage ownership and trust boundaries.
4. [Quality gates](QUALITY-GATES.md) for baseline and change-specific checks.
5. The exact versioned specification that owns the changed syntax, protocol,
   ABI, report, or target profile.

Use the [roadmap](ROADMAP.md) for sequencing only. Use the
[changelog](../CHANGELOG.md) for history only. Neither is implementation
evidence.

Additional required references:

| Change area | Owning references |
| --- | --- |
| Records, variants, generics, matching, `Option`, `Result` | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) |
| Standard-library packages under `std/`, their catalogs, tiers, and gates | [Standard Library v1](STANDARD-LIBRARY-V1.md), [Project Manifest v1](PROJECT-MANIFEST-V1.md) |
| Cleanup, resource ownership, callable settlement | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md), [RFC 0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md) |
| Immutable borrowing, loan provenance, or path-sensitive loan edges | [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md), [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md), [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) |
| Single-file semantic changes | [Patch v2](SEMANTIC-PATCH-V2.md), [Impact](SEMANTIC-IMPACT-V1.md), [Review](SEMANTIC-REVIEW-V1.md), and the relevant evidence version |
| Managed multi-file publication | [Workspace Transaction v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md), [Workspace Patch Evidence v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) |
| Cross-file semantic analysis or change | [Workspace overview](SEMANTIC-WORKSPACE-V1.md), [graph](WORKSPACE-SEMANTIC-GRAPH-V1.md), [analysis](WORKSPACE-ANALYSIS-V1.md), [change](SEMANTIC-WORKSPACE-CHANGE-V1.md), [operations](SEMANTIC-WORKSPACE-OPERATIONS-V1.md) |
| Canonical Project-derived semantic workspace revision | [Canonical Semantic Workspace Revision v1](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md), [Project Manifest v1](PROJECT-MANIFEST-V1.md), [Semantic Workspace Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md) |
| Project daemon rename/workflow | [Project Transport v2](PROJECT-AGENT-TRANSPORT-V2.md), [Rename Transaction v1](PROJECT-RENAME-TRANSACTION-V1.md), [Workflow v1](PROJECT-AGENT-WORKFLOW-V1.md) |
| `semaprax.toml` layout, tables, or lowering | [Package Manifest v1](PACKAGE-MANIFEST-V1.md) and the frozen [Project Manifest v1](PROJECT-MANIFEST-V1.md) profile it lowers to |
| `semaprax.lock` render or verification | [Project Lock v1](PROJECT-LOCK-V1.md) |
| Resolving manifest `[dependencies]` against a cache | [Project Dependency Resolution v1](PROJECT-DEPENDENCY-RESOLUTION-V1.md), [Offline Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md) |
| Linking exact SEMAPRAX subjects or exposing exact Rust crates | [Project Dependencies v1](PROJECT-DEPENDENCIES-V1.md), [Package Manifest v1](PACKAGE-MANIFEST-V1.md), [Native Rust Interoperability v1](NATIVE-RUST-INTEROP-V1.md) |
| Native Rust SDK or host integration | [Native Rust Interoperability v1](NATIVE-RUST-INTEROP-V1.md), [Project Manifest v1](PROJECT-MANIFEST-V1.md) |
| Authenticated Project input persistence | [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md), [Project Manifest v1](PROJECT-MANIFEST-V1.md), and the additive manifest profile selected by the subject |
| Offline semantic lock snapshot or fixed-inventory publication | [Published Semantic Lock Snapshot v1](OFFLINE-PUBLISHED-SEMANTIC-LOCK-SNAPSHOT-V1.md), [Offline Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md), and [Offline Semantic Lock v2](OFFLINE-SEMANTIC-PACKAGE-LOCK-V2.md) |

## Documentation classes

Every document has one primary role:

| Class | Owns | Must not own |
| --- | --- | --- |
| Public guide | Concepts, supported workflows, examples, user-facing limits | CI run history, module-level implementation narration |
| Versioned reference | Exact syntax, schema, ABI, diagnostics, admission, compatibility, non-claims | Project-wide status or roadmap priority |
| Internal architecture | Stage ownership, data flow, trust and authority boundaries | Feature history or exhaustive test commands |
| Completion matrix | Current status and the condition for a row to become complete | Historical milestone narration or protocol details |
| Quality gates | Baseline profiles and how to select required evidence | Product marketing or roadmap sequencing |
| Roadmap | Ordered outcomes and exit conditions | Claims that an outcome is already implemented |
| Changelog | Historical repository changes | Current status authority |
| Private/proof contract | Exact experimental or hosted-test boundary | Public API, stability, or production-support claims |

Stable specification paths remain flat under `docs/` to preserve citations.
Audience separation is expressed through this guide and the book structure,
not by moving every established path.

## Change protocol

1. Identify the completion-matrix rows and semantic invariants affected.
2. Update or add the owning specification before broad implementation prose.
3. Add a success case and a stable diagnostic regression before or with the
   implementation.
4. When syntax carries runtime meaning, update parser, canonical formatter,
   resolver/HIR, verifier, graph, native backend, and Wasm backend together.
5. Exercise both projections: canonical source round-trip and semantic graph
   assertions.
6. Run the baseline gate plus the owning specification's focused evidence.
7. Update the completion matrix only if the row's stated gate changes status;
   record implementation history in the changelog.

## Repository navigation

Use semantic tools before reconstructing program meaning from source text:

```sh
cargo run --locked -p semaprax -- graph <file>
cargo run --locked -p semaprax -- context <file> <stable-id> --depth 1
```

Use `rg`/`rg --files` for bounded source navigation. See
[ADR 0001](decisions/0001-graphify.md) before adding another repository-wide
graph index.

The [architecture](ARCHITECTURE.md) is the single repository module map.
`AGENTS.md` contains operating invariants and routes contributors here instead
of duplicating that map.

Before splitting a module, check whether a gate binds its text. `rg` the module's
path across `tests/` and `crates/*/src` for `include_str!` and path reads: a hit
means a [source-locked contract](ARCHITECTURE.md#source-locked-contracts) whose
join must follow the code, or it will keep passing while covering less.
`tests/source_locked_contracts.rs` fails when a reader binds a module root but
not its submodules, and `tests/module_size.rs` fails when a module grows past its
recorded size.

## Windows checkouts

Archived evidence under `docs/evidence/` nests a subject digest inside a commit
digest, and twenty-four of those paths exceed the 260-character limit Windows
applies by default — the longest reaches 279 characters once a CI runner's
workspace prefix is added. Git refuses to create them with `Filename too long`
and the checkout fails before any build starts.

Enable long paths before cloning on Windows:

```sh
git config --global core.longpaths true
```

CI does this for every job, guarded by `runner.os == 'Windows'`, in a step that
runs before `actions/checkout`; the setting has to exist before the clone, not
after it. Keep new evidence paths short enough that this remains a safety net
rather than a requirement.

## Verification

The standalone `semaprax` registry package has no private-host dependency.
The unpublished `crates/semaprax-toolchain` package builds `semaprax-full`
using the same compiler and CLI driver. `new` and `doctor` are standalone
routes; use the full toolchain for `build --target rust`; Windows revision-store persistence/loading live in
its library. Source installs retain the distinct binary name. Tag archives
package that binary as `semaprax`, alongside `semapraxd`.

Windows Project v8–v10 npm/Web publication also requires `semaprax-full`;
standalone publication rejects before output effects. The private route requires
an existing parent and uses held handles, not the legacy CLI parent helper.
See [Windows owned npm publication](WINDOWS-OWNED-NPM-PUBLICATION-V1.md).

Do not add private crates to the root package's normal or optional dependency
closure, including test-only dependencies. Private-host tests belong to the
private toolchain package, whose path dependencies retain exact version pins.
The package gate must verify the actual archive; disabling verification is not
a packaging fix.

The quickstart, frame-payload product, and root owned-data/UTF-8 SDK tests
share `tests/support/full_toolchain.rs` to build the unpublished CLI locked
and offline. The helper selects the unique `semaprax-full` binary from
[Cargo's artifact messages](https://doc.rust-lang.org/cargo/reference/external-tools.html#artifact-messages),
bound to the expected toolchain manifest and a non-test binary target. It
requires successful Cargo exit and `build-finished`, then uses the reported
absolute executable path rather than guessing `target/debug`. A configured
Cargo target must not cause a stale binary at the guessed path to substitute
for the reported output. Up-to-date (`fresh: true`) output remains valid under
Cargo's own freshness decision; this is not independent artifact attestation
or cross-compilation support.

`tests/full_toolchain_artifact_v1.rs` authors literal-message regressions with
real pathname witnesses for configured-target output, stale guessed paths,
duplicate/missing/foreign artifacts, malformed streams and unsuccessful or
missing completion. These checks remain unrun; they neither compile nor
execute a toolchain when eventually selected. The existing product tests
separately own actual Cargo and CLI execution.

On Unix, run the complete gate with:

```sh
scripts/quality.sh full
```

For documentation-only changes, the routed gate still checks formatting,
examples, rustdoc, and local links. See [Quality gates](QUALITY-GATES.md) for
profiles and change-specific evidence ownership.

When several worktrees build on one machine, give each its own target
directory and keep debug data out of it:

```sh
export CARGO_TARGET_DIR="$PWD/target/private"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
```

`CARGO_TARGET_DIR` must stay under the worktree it serves; a `target-dir`
shared with another checkout lets that checkout's build replace this one's
binaries and fingerprints while its tests run. The `full` profile's
`--workspace --all-targets` test build links several hundred integration
binaries and needs well over 10 GB in that directory even with the variables
above; a `-p semaprax` harness build needs roughly 1 GB. Remove the private
directory when the work is done.

## Documentation maintenance rules

- Put every document's audience and status within its first 12 lines.
- Link to the owner of a fact instead of copying its full explanation.
- Keep exact commands and known-answer digests in the owning versioned
  reference or test, not in the roadmap or README.
- Describe a boundary once, then use a short link elsewhere.
- Use “implemented” only when the completion gate has executable evidence.
- Describe local, hosted, private, public, and proof-only evidence explicitly;
  none implies another.
- Keep local Markdown links resolvable and catalog every document in
  `SUMMARY.md`; `tests/documentation.rs` enforces links, metadata, and catalog
  coverage.
