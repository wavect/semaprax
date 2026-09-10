# v0.4.0 implementation and evidence baseline

Status: **HOSTED GREEN** for the implemented v0.4.0 code baseline.

Audience: documentation readers, contributors, maintainers, and coding agents.

Baseline date: 2026-09-10.

## Authoritative release baseline

The current documentation baseline is the published
[SEMAPRAX v0.4.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.4.0),
commit `dfc15e2ddc818fa97744b5a9d69fd6108dd6a321`. The maintainer has confirmed
**HOSTED GREEN** for the implemented code and its release evidence. The
release-note length problem was a publication issue, not an outstanding
implementation or hosted-conformance task.

The documentation reconciliation starts from `main` commit
`49119c8d14c2ff1f6fef582ea80611659fc930ef`. Its changes after the release tag
are documentation-only; the compiler, runtime, packages, tests, and workflow
logic are the same as the release baseline. Subsequent documentation-only
commits do not create a new language or protocol version.

This record establishes the current release acceptance; it does not invent a
new workflow-run identifier or rewrite the conclusions of historical workflow
attempts. Exact older run IDs, logs, test counts, and known-answer values remain
attached to the executions that produced them.

## How to read evidence status throughout docs/

For an already implemented feature included in v0.4.0, pre-release wording
such as "local evidence only", "hosted evidence remains required", "hosted
promotion remains open", or "current-head evidence unobserved" is superseded
by this **HOSTED GREEN** baseline where the missing condition was execution of
the implemented release gates. Do not carry those completed evidence tasks
forward into the roadmap or an implementation backlog.

Three separate facts must remain separate:

1. **Implementation and evidence:** the admitted v0.4.0 implementation has the
   accepted hosted-green release evidence.
2. **Contract scope:** each owning specification still defines exactly which
   syntax, types, operations, effects, targets, and failure paths are admitted.
3. **Product completion or publication:** missing functionality, an unpublished
   SDK, a private ABI, an explicit promotion decision, or a broader completion
   gate is not completed merely by the evidence status changing.

A historical local execution remains a historical local execution. It can be
retained as a reproducible witness alongside the current hosted-green baseline;
it is no longer the sole current evidence classification for the released
implementation. Conversely, a future, proof-only, ignored, or explicitly
unimplemented gate is not silently declared executed by this record.

The [completion matrix](COMPLETION-MATRIX.md) remains the authority for the
full product requirements. Its overall **Partial** status is compatible with a
**HOSTED GREEN** release: the release proves the admitted slices, not every
future feature in the language objective.

## Released implementation map

Use the owning versioned specifications for exact limits and compatibility.
The release includes the current implementations, not just the earlier v1
slices described in historical evidence sections.

| Area | Current owning references |
| --- | --- |
| Source Agent lifecycle, typed operations, and direct execution | [Iterative lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md), [typed effects v3](AGENT-TYPED-EFFECTS-V3.md), [Direct Runtime v2](AGENT-RUNTIME-V2.md) |
| Checkpointed execution and migration | [Operation checkpoints v2](AGENT-OPERATION-CHECKPOINT-V2.md), [state migration v2](AGENT-STATE-MIGRATION-V2.md), [durable migration v3](AGENT-STATE-MIGRATION-V3.md) |
| Project and workspace execution associations | [Project linked lifecycle](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md), [Project linked migration](PROJECT-LINKED-AGENT-MIGRATION-V1.md), [execution roots](EXECUTION-ROOT-ASSOCIATION-V1.md), [workspace association](WORKSPACE-EXECUTION-ASSOCIATION-V1.md), [workspace migration](WORKSPACE-EXECUTION-MIGRATION-V1.md) |
| Generic language composition | [Argument inference v3](GENERIC-ARGUMENT-INFERENCE-V3.md), [authored variants](GENERIC-AUTHORED-VARIANTS-V1.md), [compiler collections](GENERIC-COMPILER-COLLECTIONS-V1.md), [owned record composition v2](GENERIC-OWNED-RECORD-COMPOSITION-V2.md), [owned Result](GENERIC-OWNED-RESULT-V1.md) |
| Collections, callbacks, and traversal | [Vec v2](OWNED-BOUNDED-VEC-V2.md), [Box v2](OWNED-BOUNDED-BOX-V2.md), [function values v2](FUNCTION-VALUES-V2.md), [closures v2](CLOSURES-V2.md), [generic Iterator helpers](GENERIC-ITERATORS-V1.md), [generic Iterator operations](GENERIC-ITERATOR-OPERATIONS-V1.md) |
| Semantic changes and retained program meaning | [Universal transactions v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md), [transaction composition](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md), [semantic query](UNIVERSAL-SEMANTIC-QUERY-V1.md), [incremental semantic service](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md) |
| Installed and generated workflows | [Unified CLI](UNIFIED-CLI-V1.md), [installed guidance](INSTALLED-AGENT-GUIDANCE-V1.md), [scaffold v3](PROJECT-SCAFFOLD-V3.md), [standard library catalog](STANDARD-LIBRARY-CATALOG.md) |

This map is a navigation aid, not a replacement for any versioned contract.
Older versions remain relevant to compatibility and frozen wire identities.
A successor's additional behavior must not be retroactively attributed to its
predecessor's schema or ABI.

## Published artifacts

The release is pre-alpha and has three published toolchain archives. The
following sizes and SHA-256 digests are the release API's artifact metadata.

| Archive | Bytes | SHA-256 |
| --- | ---: | --- |
| `semaprax-v0.4.0-x86_64-unknown-linux-gnu.tar.gz` | 17,922,460 | `21613bed94c9ed41d8ca67cee0924429fff18b1236198c58b16cb4bfd40786f9` |
| `semaprax-v0.4.0-aarch64-apple-darwin.tar.gz` | 15,789,284 | `9b4ebf2bc0e9ca8bdb12db4dea795f9bf7b7b8cf73731f077c3bb80221acda60` |
| `semaprax-v0.4.0-x86_64-pc-windows-msvc.zip` | 18,547,218 | `e175bfc830f189229f0b9881df3afcf0bfb10939cd4a89c5b18bc138def54d07` |

See the [release process](RELEASE-PROCESS.md#040-hosted-release-evidence)
for the release record and [installation guide](INSTALL.md) for use. Checksums
are integrity metadata, not signatures. The release remains unsigned, not
notarized, and not a claim of cross-host byte-reproducible builds.

## Documentation maintenance rules

Current summaries, feature status paragraphs, and roadmap evidence tasks should
use this release baseline rather than repeat pre-release local-only status.
Preserve semantic limits and remaining functionality, including explicit public
promotion requirements. Preserve historical release records and the bytes of
retained evidence under `docs/evidence/`; those are provenance, not mutable
status dashboards. Generated catalogs and diagnostic-help data must continue to
match their owning generators and executable contracts.

A later code change requires evidence for its changed behavior. Do not reuse
this baseline to claim that untested post-v0.4.0 code is hosted green. A
successful documentation update also does not assert that a new compiler test
run was performed during that update.
