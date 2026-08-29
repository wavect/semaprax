# SEMAPRAX development documentation

Status: living internal contributor documentation.

Audience: compiler contributors, maintainers, reviewers, and coding agents.

This page is the internal documentation entry point. Public users should start
with the [documentation overview](index.md). Versioned specifications remain
publicly readable, but documents marked private, proof-only, or internal do not
describe supported product surfaces.

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
| Cleanup, resource ownership, callable settlement | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md), [RFC 0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md) |
| Immutable borrowing, loan provenance, or path-sensitive loan edges | [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md), [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) |
| Single-file semantic changes | [Patch v2](SEMANTIC-PATCH-V2.md), [Impact](SEMANTIC-IMPACT-V1.md), [Review](SEMANTIC-REVIEW-V1.md), and the relevant evidence version |
| Managed multi-file publication | [Workspace Transaction v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md), [Workspace Patch Evidence v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) |
| Cross-file semantic analysis or change | [Workspace overview](SEMANTIC-WORKSPACE-V1.md), [graph](WORKSPACE-SEMANTIC-GRAPH-V1.md), [analysis](WORKSPACE-ANALYSIS-V1.md), [change](SEMANTIC-WORKSPACE-CHANGE-V1.md), [operations](SEMANTIC-WORKSPACE-OPERATIONS-V1.md) |
| Project daemon rename/workflow | [Project Transport v2](PROJECT-AGENT-TRANSPORT-V2.md), [Rename Transaction v1](PROJECT-RENAME-TRANSACTION-V1.md), [Workflow v1](PROJECT-AGENT-WORKFLOW-V1.md) |
| Native Rust SDK or host integration | [Native Rust Interoperability v1](NATIVE-RUST-INTEROP-V1.md), [Project Manifest v1](PROJECT-MANIFEST-V1.md) |

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

## Verification

On Unix, run the complete gate with:

```sh
scripts/quality.sh full
```

For documentation-only changes, the routed gate still checks formatting,
examples, rustdoc, and local links. See [Quality gates](QUALITY-GATES.md) for
profiles and change-specific evidence ownership.

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
