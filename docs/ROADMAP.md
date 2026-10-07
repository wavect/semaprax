# Roadmap

Status: work sequence, not proof of implementation or support.
Audience: contributors, maintainers, and evaluators.

Use the [completion matrix](COMPLETION-MATRIX.md) for current claims and the
[v0.4.0 release record](RELEASE-0.4.0-STATUS.md) for the accepted hosted-green
baseline. A versioned specification defines a contract; it does not prove that
the feature is available. The [changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md)
records what changed.

The order here follows risk: keep identity, ownership, change authority, and
target behavior sound before widening public APIs.

## Current release baseline

v0.4.0 is the last accepted hosted-green release baseline. The full product is
still Partial. The later v0.7.0 prerelease is downloadable; v0.8.0 is a
source candidate. See [current release status](RELEASE-0.8.0-STATUS.md) and
the [historical v0.6.0 gate record](RELEASE-0.6.0-STATUS.md).

The [persistent semantic cache](PERSISTENT-SEMANTIC-CACHE-V1.md) already reuses
checked HIR across processes under authenticated source/HIR validation. It is
not full incremental compilation. Private profiles remain private even when
their bounded tests pass.

## Unified semantic program implementation sequence

Source, ProgramRoot, graph, Agent, deployment, checkpoints, target artifacts,
and evidence must describe the same checked program. Reports and runtime
receipts never authorize mutation or publication.

| Workstream | Next boundary |
| --- | --- |
| Generic semantics (GEN-05/06) | Extend the existing internal concrete-instance and ownership closure without silently widening a public ABI. |
| Agent lifecycle (AGENT-06) | Broaden providers, Proposal/result forms, and maintained packaging while preserving per-turn authorization and durable replay. |
| Runtime association (SEG-04) | Complete shared durable service/tooling projections without treating a receipt as authority. |
| Collections (LANG-07) | Add broader owned payloads, iterators, lifetimes, and captures beyond the admitted bounded forms. |
| Standard library (STD-08) | Finish the required Everyday modules, provider availability, and offline templates. |
| Public generic ABI (ABI-09) | Complete caller/carrier and physical-adapter evidence, then make a separate support decision. PG-9 currently says unsupported and unpublished. |

The [Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
owns PG-1 through PG-9. The cited PG-1–8 hosted run applies to its exact older
commit, not automatically to current main. Local-only hardening of the
separately-scoped `public-generic-wasm-provider.v1` compiled-provider profile
(#287, #288, #301, #292; local candidate `77d68e49` on the unreleased
`wavect/v070` integration branch) has since narrowed the caller/carrier and
physical-adapter evidence gap for that one profile further; it is not a
hosted run, not a #164 candidate freeze, and does not change PG-9. The next
boundary for ABI-09 remains: a fresh hosted three-OS run, an artifact-digest
inventory, and a compatibility/security review at one exact frozen candidate
— all currently open because hosted CI capacity (GitHub Actions credits) is
exhausted for this account, not because the work was skipped.

<a id="current-priority-post-v02-promotion-boundaries"></a>

## Current priority: post-v0.4 promotion boundaries

For each surface, decide separately whether it is implemented, whether its
exact release corpus passed, and whether users may rely on it. Promotion needs
an explicit version, target scope, maintained package/host path, and fresh
evidence. A CI label alone does not publish a private profile.

The full-toolchain archives remain beta releases. Keep source-selected private hosts
visible until a promoted workflow intentionally hides them from users.

## Developer preview: promote the authored Project v8 slice

Project v8's bounded owned-byte path and its direct-browser regression are
implemented. [ADR 0003](decisions/0003-maintained-generated-package-support.md#current-maintainer-decision)
reaffirms ordinary owned-data Rust-only maintenance for #325; npm maintenance
remains deferred to a separate ADR. Registry publication and broader
API/browser/runtime promotion remain separate decisions, not implied by the
Rust scope or genuine local npm execution. Preserve descriptor/carrier replay,
ownership settlement, stable identity, and no-clobber publication. [Transport v5](PROJECT-AGENT-TRANSPORT-V5.md)
and [v6](PROJECT-AGENT-TRANSPORT-V6.md) are read-only; they gain no write or
publication authority.

Project v9 adds [flat owned records](PUBLIC-FLAT-OWNED-RECORD-API-V1.md), v10
adds [owned UTF-8](PUBLIC-OWNED-UTF8-API-V1.md), and v11 adds
[nested owned records](PUBLIC-NESTED-OWNED-RECORD-API-V1.md). Each has its own
prerequisites and explicit support decision. Later private shapes do not widen
the Project v8 contract. The
[profile-admission dispatcher](PROJECT-PROFILE-ADMISSION-V1.md) is a gate, not
a promotion decision.

## Graph-operational development foundation

The [graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md) owns the full
requirement ledger. Its implemented slices are a foundation, not completion.

### Agent execution and revision changes

Build on checked source Agent declarations, generated Proposal clients, the
[iterative runtime](AGENT-RUNTIME-V2.md), per-operation checkpoints, and pure
or durable State migration. Next: broader providers, nominal results,
native/Wasm Agent-stage execution, supported packages, and coordination across
stores. Ordinary native/Wasm library execution is not Agent-stage execution.

### Semantic changes, queries and service lifecycle

Candidate and image APIs already cover bounded changes, replay, holes, repair,
tests, and review. [Universal transaction v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md)
and [composition v1](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md) add
specific expression and merge operations, not general merging. Next: broader
ownership-sensitive edits, interfaces, conflict handling, incremental checking,
and measured workflow savings. New editing routes must preserve comments and
unrelated trivia.

The [stdio](PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md) and
[MCP](PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md) service facades are bounded,
authority-free routes. They do not make external facts current by copying them.

### Recovery, retention and publication

Source-backed images, held stores, receipts, startup selection, and bounded
candidate/draft recovery exist. Recovery rechecks source; it does not restore
approval or make old source current. Next: complete session-startup integration,
authorized eviction, and measured recovery cost.

Review/export and later publication use separate, independently approved
sessions. Managed `ACTIVE` visibility is not Git publication or visibility to
arbitrary raw-path readers. Broader repository hosts and long-lived approval
remain separate work.

### Supported clients and editor workflows

Generated TypeScript, Python, and Rust clients and the bounded signature
workflow exist. Next: full report-payload validation, broader editor tasks,
repairs, host coverage, and representative task/token measurements. A suggested
repair is not an executed one; a transport is not registry support.

## 0.3: ownership and fast development

This is a retained workstream name, not a pending v0.3 release.

### Language and ownership outcomes

Extend the admitted owned records, variants, buffers, collections, iterators,
and closures to more carriers, lifetimes, control flow, and FFI. General
borrowed APIs, raw memory, shared ARC, and physical region/arena behavior need
their own rules and tests. A proof model does not perform a finalizer.

### Development-loop outcomes

Complete dependency-aware incremental checking, debugging/profiling,
cross-process warm HIR reuse, and representative performance measurements.
Preserve exact-source cache validation and separate Unix/Windows store
authority. A rejected refresh must leave prior state intact.

Exit: representative owned applications agree on success, failure, cleanup,
and contracts in development, native, and WebAssembly lanes.

## 0.4: components, packages, and interoperability

The v0.4.0 artifact release is complete. This wider ecosystem goal is not.

### Package outcomes

Existing package reports, locks, offline resolution, source capsules, and
local publication have bounded evidence. Next: supported registry acquisition,
trusted publisher provenance, generic cross-package signatures, compatibility
migration, and reproducible artifacts. A deterministic source bundle is not a
trusted registry package or an OS sandbox.

### Standard library outcomes

[Standard Library v1](STANDARD-LIBRARY-V1.md) owns the required modules; the
[catalog](STANDARD-LIBRARY-CATALOG.md) lists declarations. Its implemented
profiles have bounded evidence, but the complete Everyday profile remains
Partial. Finish remaining modules, providers, streams, tests, and templates
without granting capabilities through ordinary data values.

### ABI and host outcomes

Specify and exercise general aggregate, resource, borrowed-view, String,
error, callback, and async boundaries. Promote maintained consumers only with
versioned identities, compatibility checks, physical-host evidence, and an
explicit support decision. A private Component fixture or simulator is not a
public ABI or physical device.

Exit: one versioned package works from every declared supported host language
and target lane with reproducible builds and no ambient authority.

## 0.5: concurrency and applications

### Concurrency and services

Bounded Rust scoped threads and selected command/network/TLS routes exist.
Next: language task syntax and checked task graphs, deterministic schedules,
native/Wasm lowering, broader providers, and production services. Live network
support is always limited to the named policy and host path.

### Application model

Build typed state/action/view workflows and maintained accessible clients for
web, iOS, Android, macOS, Windows, and Linux. Private framework/JNI/desktop
fixtures are not supported application platforms.

[Issue #241](https://github.com/wavect/semaprax/issues/241) tracks two current
compiler capacity ceilings: `SPX-G171` limits a whole-project workspace graph
to 67,108,864 builder bytes; `SPX-H006` limits cleanup replay to 32,000,000
program-wide work units. The former 65,536-terminal-path limit per function
now applies only to enumerated replay: above 4,096 paths replay compares the
function factored by cleanup state, whose cost follows distinct cleanup
states rather than independent decision combinations. Raising either budget
needs evidence that replay and cache behavior stay finite at the new bound.

## 1.0: validate the complete programming system

The [final validation product](COMPLETION-MATRIX.md#final-validation-product)
is a maintained offline-first application across all six client platforms,
native/WASI server execution, custom accelerated visuals, and C, JavaScript,
and WebAssembly integrations. Every claimed artifact needs representative
build, execution, migration, and reproducibility evidence. Smaller fixtures or
private adapters cannot replace this product.

## Research profiles after the core product

Economic-agent work is optional. The current language grants no ambient
wallet, key, signing, provider, or mainnet authority. Any future profile must
retain explicit capabilities, custody separation, approvals, idempotent
settlement, privacy, and auditability.
