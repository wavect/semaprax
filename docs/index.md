# SEMAPRAX documentation

Status: living public documentation entry point.

Audience: language users and integrators.

This is the public documentation entry point for SEMAPRAX. Start here to learn
the language and its supported workflows. Contributor process, implementation
evidence, private experiments, and repository internals live in the separate
[development guide](DEVELOPMENT.md).

> SEMAPRAX is pre-alpha. A versioned document describes an exact bounded
> contract; it does not imply that the broader feature is complete or stable.
> The [completion matrix](COMPLETION-MATRIX.md) is the status authority.

## Choose a path

| You want to… | Start with… |
| --- | --- |
| Try the language | The root [README](../README.md) and its runnable examples |
| Understand the language design | [RFC 0001](RFC-0001.md) |
| Work with records, variants, matching, `Option`, or `Result` | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) |
| Understand ownership and cleanup | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) |
| Build a multi-file project | [Project Manifest v1](PROJECT-MANIFEST-V1.md) |
| Call SEMAPRAX from JavaScript | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md) |
| Understand the proposed owned-byte SDK boundary | [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) |
| Query program meaning | [Agent Context v2](AGENT-CONTEXT-V2.md) |
| Preview or apply a semantic change | [Semantic Patch v2](SEMANTIC-PATCH-V2.md), then [Impact](SEMANTIC-IMPACT-V1.md) and [Review](SEMANTIC-REVIEW-V1.md) |
| Integrate a compiler report or generated artifact | Use the [reference catalog](#reference-catalog) |
| Contribute to the compiler | [Development documentation](DEVELOPMENT.md) |

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
target-neutral cleanup plans. A feature is not described as implemented across
targets unless its documented executable gate covers those targets.

## Public language and workflow references

- [RFC 0001](RFC-0001.md): complete language and toolchain contract.
- [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md): algebraic data and aggregate ownership.
- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md): cleanup and resource ABI.
- [Mutation](EXPLICIT-MUTATION-V1.md), [field mutation](FIELD-MUTATION-V1.md),
  [while loops](WHILE-LOOPS-V1.md), and
  [refutable matching](REFUTABLE-MATCH-V1.md): bounded language extensions.
- [Project Manifest v1](PROJECT-MANIFEST-V1.md): bounded multi-file input and
  build contract. Later manifest versions are additive specialized profiles.
- [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md): generated JavaScript and
  TypeScript boundary for selected stable-ID scalar functions.
- [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md) and
  [Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md): narrow text
  and byte-data profiles.
- [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md): proposed additive
  Project v8 profile and completion contract for copied owned-byte results in
  JavaScript/TypeScript and safe Rust. It is not yet activated.

## Agent workflow references

The supported conceptual flow is:

```text
graph/context → patch → impact/review → evidence replay → atomic apply
```

- [Agent Context v1](AGENT-CONTEXT-V1.md) and
  [v2](AGENT-CONTEXT-V2.md) define bounded semantic queries.
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

The exhaustive, audience-separated list is in [SUMMARY.md](SUMMARY.md):

- public language and workflow references;
- agent and workspace protocol references;
- target, ABI, schema, and package projections;
- internal architecture, quality, status, roadmap, decisions, and private
  experiment contracts.

## Compatibility and status

- [Protocol migrations](MIGRATIONS.md) records compatibility changes between
  versioned agent-facing formats.
- [Completion matrix](COMPLETION-MATRIX.md) owns current status and completion
  criteria.
- [Changelog](../CHANGELOG.md) owns historical implementation changes.
- [Roadmap](ROADMAP.md) owns future sequencing, not implementation claims.

Keeping these responsibilities separate prevents the same status narrative
from drifting across the README, RFCs, architecture, and roadmap.
