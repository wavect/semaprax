# SEMAPRAX documentation

Status: living public documentation entry point for v0.4.0; **HOSTED GREEN** implementation baseline.

Audience: language users and integrators.

This is the public documentation entry point for SEMAPRAX. Start here to learn
the language and its supported workflows. Contributor process, implementation
evidence, private experiments, and repository internals live in the separate
[development guide](DEVELOPMENT.md).

> SEMAPRAX is pre-alpha. A versioned document describes an exact bounded
> contract; it does not imply that the broader feature is complete or stable.
> The [completion matrix](COMPLETION-MATRIX.md) is the product-status authority.
> The [v0.4.0 baseline](RELEASE-0.4.0-STATUS.md) records **HOSTED GREEN** for the
> released implementation and supersedes its pre-release local-only status.

The [v0.4.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.4.0)
contains smoke-tested Linux x86-64, Apple Silicon macOS, and Windows x86-64
archives. The implemented release code has **HOSTED GREEN** evidence; the
release-note publication issue is not an outstanding code-evidence gate.
See the [release baseline](RELEASE-0.4.0-STATUS.md) and
[release record and checksums](RELEASE-PROCESS.md#040-hosted-release-evidence).
The release remains unsigned, not notarized, and pre-alpha.
Recent project and tooling notes are summarized in [CHANGELOG.md](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md),
with compact highlights in [CHANGELOG-SUMMARY.md](CHANGELOG-SUMMARY.md),
and full history in [docs/CHANGELOG-ARCHIVE.md](CHANGELOG-ARCHIVE.md).

## Choose a path

| You want to… | Start with… |
| --- | --- |
| Understand the current implementation and evidence stage | [v0.4.0 HOSTED GREEN baseline](RELEASE-0.4.0-STATUS.md) |
| Install a working toolchain | [Install](INSTALL.md) |
| Track recent changes | [CHANGELOG](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md) |
| Try the language | Follow the executable [quickstart](QUICKSTART.md), then explore the root [README](https://github.com/wavect/semaprax/blob/main/README.md) |
| Learn the language itself | Work through the [language tour](LANGUAGE-TOUR.md) |
| Write SEMAPRAX as a coding agent with a small context window | Load the compiler-checked [agent quick reference](AGENT-QUICK-REFERENCE.md) |
| Fix a known `SPX-*` diagnostic without loading the full reference | Run `semaprax help diagnostic <SPX-code>`; `semaprax help diagnostic codes` lists exact supported codes |
| Find a standard-library declaration and its contract | Read the generated [standard library catalog](STANDARD-LIBRARY-CATALOG.md); [Standard Library v1](STANDARD-LIBRARY-V1.md) owns the tiers and required modules |
| Find a minimal example to point a command at | [Examples index](https://github.com/wavect/semaprax/blob/main/examples/README.md) |
| Find or automate a compiler command | [Using the SEMAPRAX CLI](CLI-GUIDE.md) and [Unified CLI](UNIFIED-CLI-V1.md) |
| Highlight `.spx` files in Visual Studio Code | The repository's [VS Code extension](https://github.com/wavect/semaprax/blob/main/editors/vscode/README.md) |
| Understand the language design | [RFC 0001](RFC-0001.md) |
| Work with records, variants, matching, `Option`, or `Result` | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) |
| Understand ownership and cleanup | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) |
| Build a multi-file project | [Project Manifest v1](PROJECT-MANIFEST-V1.md), then the owning additive profile |
| Run a source-defined Agent | [Iterative lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md) and [Direct Runtime v2](AGENT-RUNTIME-V2.md) |
| Use generic collections or callbacks | [Generic compiler collections](GENERIC-COMPILER-COLLECTIONS-V1.md), [function values v2](FUNCTION-VALUES-V2.md), and [closures v2](CLOSURES-V2.md) |
| Call SEMAPRAX from JavaScript | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md) |
| Inspect the unpromoted Project v8 owned-byte SDK boundary | [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) |
| Inspect the unpromoted Project v9 flat-record boundary | [Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md) |
| Inspect the unpromoted Project v10 owned-string boundary | [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) |
| Query program meaning | [Agent Context v2](AGENT-CONTEXT-V2.md) and [Universal Semantic Query](UNIVERSAL-SEMANTIC-QUERY-V1.md) |
| Preview or apply a semantic change | [Semantic Patch v2](SEMANTIC-PATCH-V2.md), then [Impact](SEMANTIC-IMPACT-V1.md) and [Review](SEMANTIC-REVIEW-V1.md) |
| Validate an authority-free Project transaction | [Universal Semantic Transaction v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md) and [composition](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md) |
| Integrate a compiler report or generated artifact | Use the [reference catalog](#reference-catalog) |
| Contribute to the compiler | [First contribution](FIRST-CONTRIBUTION.md), then the [development documentation](DEVELOPMENT.md) |

## Core concepts

### Source and identity

Readable `.spx` source is the canonical Git projection. Public declarations
can carry persistent `@id` identities, while expression identities may change
with a revision. Canonical formatting removes incidental textual differences.

### Checked semantic representation

The compiler parses and verifies source before producing stable-ID HIR and a
versioned semantic graph. Graph queries expose bounded context without making
source text the agent's only representation.

### Evidence and authority

Reports and evidence capsules are deterministic descriptions. They are not
signatures, approvals, or write authority. A mutating route must authenticate
its inputs, replay the relevant evidence, and own the final transaction.

### Shared backend meaning

Native and WebAssembly implementations consume the same verified HIR and
target-neutral cleanup plans. The released implementations have hosted-green
evidence within their admitted target profiles. This does not extend a
feature to targets or shapes excluded by its owning specification.

## Public language and workflow references

- [RFC 0001](RFC-0001.md): complete language and toolchain contract.
- [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md): algebraic data and aggregate ownership.
- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md): cleanup and resource ABI.
- [Mutation](EXPLICIT-MUTATION-V1.md), [field mutation](FIELD-MUTATION-V1.md),
  [while loops](WHILE-LOOPS-V1.md), and
  [refutable matching](REFUTABLE-MATCH-V1.md): bounded language extensions.
- [Project Manifest v1](PROJECT-MANIFEST-V1.md): bounded multi-file input and
  build contract. Later manifest versions are additive specialized profiles,
  including [Useful Data v2 / Project v16](PROJECT-MANIFEST-V16.md) and
  [Process I/O / Project v18](PROJECT-MANIFEST-V18.md).
- [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md): generated JavaScript and
  TypeScript boundary for selected stable-ID scalar functions.
- [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md) and
  [Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md): narrow text
  and byte-data profiles.
- [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md): additive Project v8
  implementation and completion contract for copied owned-byte results in
  JavaScript/TypeScript and safe Rust. Release regression evidence is hosted
  green; the generated packages remain unpublished and formal promotion is open.
- [Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md):
  additive Project v9 descriptor and physical JavaScript/safe-Rust flat-record
  boundary. Release regression evidence is hosted green; it remains unpublished
  and unpromoted.
- [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md): additive Project
  v10 descriptor and physical JavaScript/safe-Rust string boundary. Release
  regression evidence is hosted green; it remains unpublished and unpromoted,
  and depends on an explicit Project v9 promotion decision.

## Agent workflow references

The supported conceptual flow is:

```text
graph/context → patch → impact/review → evidence replay → atomic apply
```

The additive Project transaction APIs also provide authority-free validation
and replay; validation alone does not perform the final apply operation.

- [Agent Context v1](AGENT-CONTEXT-V1.md) and
  [v2](AGENT-CONTEXT-V2.md) define bounded semantic queries.
- [Project Agent Transport v5](PROJECT-AGENT-TRANSPORT-V5.md) defines the
  opt-in, read-only Project v8 descriptor and inline npm carrier methods. Its
  released implementation has hosted-green evidence; public promotion remains
  a separate decision.
- [Iterative lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md),
  [typed effects v3](AGENT-TYPED-EFFECTS-V3.md), and
  [Direct Runtime v2](AGENT-RUNTIME-V2.md) define bounded source Agent execution.
  [Checkpoints v2](AGENT-OPERATION-CHECKPOINT-V2.md) and
  [durable migration v3](AGENT-STATE-MIGRATION-V3.md) retain their explicit
  authorization, replay, and recovery boundaries.
- [Semantic Patch v2](SEMANTIC-PATCH-V2.md) defines the supported single-file
  operation format.
- [Diagnostic Repair v1](DIAGNOSTIC-REPAIR-V1.md) defines repair discovery and
  the sole Patch v3 operation.
- [Semantic Impact v1](SEMANTIC-IMPACT-V1.md) and
  [Semantic Review v1](SEMANTIC-REVIEW-V1.md) are read-only previews.
- [Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md),
  [v2](SEMANTIC-PATCH-EVIDENCE-V2.md), and
  [Target Evidence v1](SEMANTIC-TARGET-EVIDENCE-V1.md) define independently
  replayable evidence formats.
- The workspace references extend the same principles to a bounded managed
  immutable-generation workspace; they do not make raw Git or editor paths
  atomically visible.

## Reference catalog

Versioned reference documents are intentionally precise. They define one wire
format, report, ABI, admission profile, or evidence boundary. They are useful
to tool and host authors but are not the recommended introduction to SEMAPRAX.

The exhaustive, audience-separated list is in [source catalog](https://github.com/wavect/semaprax/blob/main/docs/SUMMARY.md):

- public language and workflow references;
- agent and workspace protocol references;
- target, ABI, schema, and package projections;
- internal architecture, quality, status, roadmap, decisions, and private
  experiment contracts.

## Compatibility and status

- [v0.4.0 baseline](RELEASE-0.4.0-STATUS.md) owns the current hosted-green release
  evidence classification and its relationship to historical evidence.
- [Protocol migrations](MIGRATIONS.md) records compatibility changes between
  versioned agent-facing formats.
- [Completion matrix](COMPLETION-MATRIX.md) owns product status and completion
  criteria.
- [Changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md) owns historical implementation changes.
- [Changelog summary](CHANGELOG-SUMMARY.md) gives a compact latest-notes view.
- [Roadmap](ROADMAP.md) owns future sequencing, not implementation claims.

Keeping these responsibilities separate prevents the same status narrative
from drifting across the README, RFCs, architecture, and roadmap.
