# Full-goal completion matrix

Status: living internal audit; v0.4.0 implementation evidence **HOSTED GREEN**.

Audience: maintainers, contributors, reviewers, and technical evaluators.

This document is the authoritative status audit for the complete SEMAPRAX
objective. It separates the mature product requirement, the implemented bounded
slice, and the functionality or support decision still needed to complete the
requirement. The [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md) owns the
current hosted-green evidence classification.

Historical status transitions belong in the [changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md).
Protocol details, exact known-answer digests, test counts, and historical CI run
IDs belong in the linked versioned specifications. Future sequencing belongs
in the [roadmap](ROADMAP.md). The evidence summaries below describe the current
implementation rather than repeat superseded pre-release local-only ledgers.

## Status rules

| Status | Meaning |
| --- | --- |
| Implemented | The full completion gate is covered by executable evidence on every required target. |
| Partial | Useful executable evidence exists, but the full completion gate remains open. |
| Missing | No qualifying executable evidence exists for the row. |

**HOSTED GREEN** is an evidence classification, not a replacement for these
product-completion statuses. The implemented v0.4.0 slices have the accepted
hosted-green release baseline. Design-only functionality, a proof model, a
private ABI, an unpublished package, and an explicit public-support decision
remain distinct. A private profile tested on hosted CI is still private.

Pre-release labels such as **Authored, unrun**, **Local, partial**, and
"hosted promotion pending" no longer describe the released implementation's
current evidence when the only missing condition was execution of its release
gates. Historical local runs remain valid historical witnesses, not the current
evidence ceiling. Future or separately unimplemented gates are not marked
complete by changing an evidence label.

## Current summary

The separate [Persistent Semantic Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md)
implements authenticated cross-process checked-HIR reuse with independent
source/HIR validation. Its release regressions are HOSTED GREEN; full
incremental compilation and measured task-level performance remain open.

**Release implementation evidence: HOSTED GREEN**

**Overall product objective: Partial**

The long-term contract below contains **55 requirements: 55 Partial, 0
Implemented, 0 Missing**. This is the count of the actual requirement rows:
six semantic-foundation, fifteen language-and-safety, five compiler-and-target,
ten ecosystem, nine application-platform, and ten agent/operations rows.
The previous 50-row dashboard did not include all rows already present in its
own tables. This reconciliation preserves every requirement and corrects the
count; it does not add requirements or change their completion thresholds.
Work-package and release-exit tables are not included in that denominator.

The released implementation includes canonical source and stable-ID HIR;
bounded semantic queries, candidates and replay-checked changes; interpreter,
native C11/Clang and Core-Wasm execution of admitted scalar and owned-data
profiles; generated consumer and private platform integrations; and the current
Agent, generics, collections, I/O, package and installed-tooling additions.

In particular, the current Agent implementation is not limited to one acyclic
read. [Iterative lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md),
[typed effects v3](AGENT-TYPED-EFFECTS-V3.md), and
[Direct Runtime v2](AGENT-RUNTIME-V2.md) implement checked iterative execution
with exact deployment and invocation roots. [Per-operation checkpoints](AGENT-OPERATION-CHECKPOINT-V2.md),
[pure migration](AGENT-STATE-MIGRATION-V2.md),
[durable migration](AGENT-STATE-MIGRATION-V3.md), and
[linked Project roles](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md) are implemented
additions. [Linked migration](PROJECT-LINKED-AGENT-MIGRATION-V1.md) and the
[workspace association](WORKSPACE-EXECUTION-ASSOCIATION-V1.md) /
[migration](WORKSPACE-EXECUTION-MIGRATION-V1.md) profiles retain exact source,
root, currentness, trusted-store and cumulative-accounting boundaries. Their
hosted evidence is green; distributed coordination, live providers and native/
Wasm Agent-stage execution remain separate functionality.

The generic implementation includes [argument inference v3](GENERIC-ARGUMENT-INFERENCE-V3.md),
[authored variants](GENERIC-AUTHORED-VARIANTS-V1.md),
[compiler collections](GENERIC-COMPILER-COLLECTIONS-V1.md),
[record composition v2](GENERIC-OWNED-RECORD-COMPOSITION-V2.md),
[multiple owners](GENERIC-MULTI-OWNER-RECORDS-V1.md),
[owned Result](GENERIC-OWNED-RESULT-V1.md),
[function values v2](FUNCTION-VALUES-V2.md), and
[closures v2](CLOSURES-V2.md). These have hosted-green evidence for their
admitted substitutions, HIR/graph/ProgramRoot replay and backend behavior;
general constraints, owning captures and public generic ABI remain separate.

The largest remaining product gaps are general ownership and lifetime safety,
stable public aggregate/resource/component ABIs, a supported package ecosystem,
production application tooling, broader target conformance, and the final 1.0
validation product. They are not a backlog of unexecuted v0.4.0 hosted gates.

## v0.2 product-exit audit

This historical audit measures the shipped v0.2.0 objective against the broader
product goal. The annotated tag resolves to
`5f6fb9655fdec92c57ab71615cfd7bfa8cc76051`; all 45 jobs in
[release run 33608662244](https://github.com/wavect/semaprax/actions/runs/33608662244)
passed and the prerelease was published. "Exact-tag hosted" in this historical
table means only the gate selected by that run. It does not imply an ignored,
unprovisioned, broader-browser, physical-device, registry, or production claim.

| Exit criterion | Evidence | Remaining gate at that milestone |
| --- | --- | --- |
| Multi-module calculator project | Exact-tag hosted | Keep Project Manifest admission and source closure green on subsequent release candidates. |
| Same verified calculator logic on native and browser lanes | Exact-tag hosted | Preserve the identical success/failure corpus on subsequent release candidates and broaden browser engines only when claimed. |
| Several stable-ID functions callable from TypeScript and Rust | Exact-tag hosted; builder remains unpublished | Publish an intentionally supported Rust entry point. |
| Browser calculator consumes Project exports | Exact-tag Chromium, including the display-renamed fixture | Add multi-engine evidence only when broader browser compatibility is claimed. |
| Project daemon inspect/derive/preview/apply/rebuild loop | Exact-tag hosted | Preserve Transport v4's bounded authority contract on subsequent release candidates. |
| Stable external API survives a display rename | Exact-tag hosted | Preserve the complete renamed Project and consumer proof on subsequent release candidates. |
| Project tests demonstrate native/Wasm equivalence | Exact-tag hosted | Preserve the full entry/test and consumer corpus on subsequent release candidates. |
| Multi-module line-filter product | Exact-tag hosted native and Node/Core-Wasm | Add real-browser or multi-engine evidence before claiming that breadth. |
| Full promotion CI for every v0.2.0 release claim | Exact-tag hosted and published | Repeat the complete blocking gate for every later release tag. |

The v0.2.0 prerelease completed its artifact milestone, not the full product
contract. Its narrower browser and unpublished-builder limitations remain
historical facts; current evidence is recorded separately below.

Evidence owners: [Project Manifest v1](PROJECT-MANIFEST-V1.md) and its additive
profiles, [Bounded Language Command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md),
[Bounded Language Network I/O](BOUNDED-LANGUAGE-NETWORK-IO-V1.md),
[Project Agent Workflow](PROJECT-AGENT-WORKFLOW-V1.md),
[Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), and
[Native Rust Interoperability](NATIVE-RUST-INTEROP-V1.md).

## v0.4 product-exit audit

The current release is the [SEMAPRAX v0.4.0 prerelease](https://github.com/wavect/semaprax/releases/tag/v0.4.0),
commit `dfc15e2ddc818fa97744b5a9d69fd6108dd6a321`, published at
`2026-09-10T10:31:03Z`. The maintainer-confirmed implementation evidence is
**HOSTED GREEN**. The release-note length problem is not an outstanding code
or hosted-conformance gate. See the [baseline](RELEASE-0.4.0-STATUS.md) and
[release record](RELEASE-PROCESS.md#040-hosted-release-evidence) for provenance
and the exact three-archive inventory and digests.

| Exit criterion | Current evidence | Remaining product or maintenance gate |
| --- | --- | --- |
| Multi-module calculator project | HOSTED GREEN, v0.4.0 | Preserve manifest admission and source closure on later code changes. |
| Same verified calculator logic on native and browser lanes | HOSTED GREEN, v0.4.0 | Preserve the common success/failure corpus; add browser engines only with their own evidence. |
| Several stable-ID functions callable from TypeScript and Rust | HOSTED GREEN, v0.4.0; builder remains unpublished | Make the explicit supported-publication decision. |
| Browser calculator consumes Project exports | HOSTED GREEN for the admitted browser profile and renamed fixture | Broader browser support remains separately scoped. |
| Project daemon inspect/derive/preview/apply/rebuild loop | HOSTED GREEN, v0.4.0 | Preserve the bounded transport and publication authority contracts. |
| Stable external API survives a display rename | HOSTED GREEN, v0.4.0 | Preserve source-bound descriptors and both consumer corpora. |
| Project tests demonstrate native/Wasm equivalence | HOSTED GREEN, v0.4.0 | Preserve the full admitted entry/test and consumer corpus. |
| Multi-module line-filter product | HOSTED GREEN for admitted native and Node/Core-Wasm execution | Do not infer broader browser support from this profile. |
| Implemented v0.4.0 release gates and published archives | HOSTED GREEN; three archives published | Later code changes need their own evidence; release acceptance is not general production support. |

The release milestone is complete. The broader product-exit objective remains
**Partial** because publication/support decisions and functionality beyond the
admitted profiles remain open, not because the implemented v0.4.0 evidence is
local-only or awaiting a tag rerun. Historical workflow attempts and retained
logs keep their original identities and outcomes.

## WP-01–WP-15 implementation and promotion audit

This programme is separate from the 55-row product contract. "HOSTED GREEN"
here refers to the implemented v0.4.0 slice. An explicit registry, API, transport
or platform support decision remains separate from CI execution.

| Work package | Current source state | Evidence owner or implemented scope | Remaining gate |
| --- | --- | --- | --- |
| WP-01 CI decomposition | HOSTED GREEN | Dedicated product/platform matrices and release aggregation; [required checks](CI-REQUIRED-CHECKS-V1.md) | Preserve the closed aggregation and no-mask policy for later code changes. |
| WP-02 deterministic version | Released, v0.4.0 | Version `0.4.0`, exact source label, manifest and unpacked CLI agreement; [release process](RELEASE-PROCESS.md) | Preserve exact version/commit binding; agreement is not a signature. |
| WP-03 release artifacts | Released, v0.4.0 | Linux x86-64, Apple Silicon macOS and Windows x86-64 archives with recorded checksums | Add targets only with their own build-host smoke; no cross-host reproducibility is claimed. |
| WP-04 v0.2 tagged artifact/release promotion | Complete for v0.2.0 | Historical release, gate, archive and checksum record; [v0.2 evidence](RELEASE-PROCESS.md#020-hosted-release-evidence) | Preserve the historical record without reusing its run IDs for a later commit. |
| WP-04 v0.4 tagged artifact/release promotion | Complete for v0.4.0 | Accepted hosted-green code baseline and published three-archive milestone; [v0.4 evidence](RELEASE-PROCESS.md#040-hosted-release-evidence) | No outstanding changelog-length or hosted-evidence task for this code baseline; unrelated product rows are not promoted. |
| WP-05 `doctor` | HOSTED GREEN for the implemented bounded profiles | [Probe](DOCTOR-PROBE-V1.md), [Linux provisioner](DOCTOR-PRODUCTION-PROVISIONER-V1.md), [provisioned Linux gate](DOCTOR-PROVISIONED-LINUX-GATE-V1.md), and [signed install](DOCTOR-SIGNED-INSTALL-V1.md) retain explicit input, role, namespace, cgroup and store boundaries | Complete any still-unimplemented active-generation handoff and explicit support decision; macOS/Windows production confinement and ordinary production profiles remain separate from admitted Linux evidence. |
| WP-06 `new` | HOSTED GREEN, bounded | Generator, scaffold replay, CLI preservation, Project checks and platform publication | Preserve the distinction between full-toolchain staged publication and standalone creation; installed-product breadth is limited to its admitted archive cases. |
| WP-07 quickstart | HOSTED GREEN for the released source workflow | [Quickstart](QUICKSTART.md), checked examples and Project product gates | Broader installation/PATH environments require their own acceptance, not inference from source execution. |
| WP-08 v8 specification | Specified and implemented within its bounded profile | [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) owns identities, admission, lifetime, compatibility and completion gates | Keep the contract synchronized; do not equate specification or CI with public support. |
| WP-09 canonical descriptor | HOSTED GREEN | Validated-HIR derivation, canonical digest, independent replay, stable host names, hostile cases and legacy preservation | Retain exact known-answer and replay coverage on later profile changes. |
| WP-10 direct `Bytes` npm/Wasm | HOSTED GREEN | Carrier, copy-out, tuple admission, intrinsic-brand hostility, private-frame exclusion and settlement | Broader browser-engine support remains an explicit future claim. |
| WP-11 `Option<Bytes>` / `Result<Bytes, i64>` | HOSTED GREEN, bounded | Fixed tags, active payloads, TypeScript mapping, cleanup, retained evaluation and generated-facade hostility | General variant/resource APIs and broader physical/browser profiles remain separate. |
| WP-12 safe native/Rust SDK | HOSTED GREEN; unpublished | Provider/SDK settlement, hostile handles, O0/O2, allocation, sanitizers and locked/offline consumers | Make the explicit registry/support decision; do not call generated developer-preview packages published. |
| WP-13 Project v8 activation | HOSTED GREEN; developer-preview | Manifest parsing, v1–v7 preservation, routing, retained evaluation and Windows full-host npm publication | Record the owning v8 API/package promotion decision. |
| WP-14 frame-payload product | HOSTED GREEN, bounded | Shared interpreter/native/Wasm/npm/Rust corpora, display rename, consumers and sanitizers | Broader browser-engine and installed-archive consumer support remains separately scoped. |
| WP-15 v8 promotion | HOSTED GREEN release evidence; formal public promotion open | [Promotion Receipt v1](PROJECT-V8-PROMOTION-RECEIPT-V1.md) replays independently supplied observations without granting authority | Record the explicit API/package support decision and any additional provisioned scope required by that decision. |
| Agent Transport v5 follow-on | HOSTED GREEN; unpromoted | Read-only descriptor/carrier methods and legacy protocol preservation | Make the explicit transport support decision; preserve its read-only scope. |
| Agent Transport v6 public-API follow-on | HOSTED GREEN; unpublished and unpromoted | Authenticated v8–v11 descriptors, replayed npm carriers, closed profile discriminants, subject binding, zero writes and generated codecs | Package and support released clients intentionally; v9–v11 package and transport promotion decisions remain separate. |
| Project v9 flat owned record follow-on | HOSTED GREEN; unpublished and unpromoted | Descriptor, retained evaluator, Wasm/npm, native/Rust settlement, Revision Store and C/C++ provider-consumer profiles | Complete additional aggregate fault, architecture and support scope required for public v9 promotion. |
| Project v10 owned UTF-8 follow-on | HOSTED GREEN; unpublished and unpromoted | Descriptor/evaluator, String accounting, Wasm/npm, native/Rust settlement and exact-length UTF-8 C consumers | Record prerequisite v9 and explicit v10 promotion decisions; broader profiles remain separately gated. |
| Project v11 nested owned-record follow-on | HOSTED GREEN; unpublished and unpromoted | Separate descriptor/evaluator replay, cumulative-boundary npm and Rust execution, C11 multi-owner settlement | Record prerequisite and v11 support decisions and complete any additional claimed platform/browser scope. |
| Project Revision Store v1 follow-on | HOSTED GREEN for admitted Unix/Windows profiles; unpromoted | Authority, identity, bounded replay, profile round trips and publication regressions | Explicit physical-host and public-support breadth remains bounded by the owning specification. |

The Project v8–v11 generated packages remain developer-preview, non-registry
surfaces unless their owning promotion decision says otherwise. Their hosted
implementation evidence is no longer an outstanding task. An authority-free
promotion receipt is a replay mechanism, not itself a support decision.

## Long-term product contract

Every row below remains **Partial** at the mature-product level. The linked
implemented slices have **HOSTED GREEN** v0.4.0 evidence. A link to a private,
proof-only or bounded specification does not broaden its scope. The "Complete
when" column describes the remaining mature-product threshold, not a claim
that all of that functionality already exists.

### Semantic foundation

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Source Agent and generated Proposal-client execution | Partial; checked source Agent selection, generated Proposal clients, iterative typed execution and retained runtime associations are implemented with hosted-green evidence. | [Agent lowering](LANGUAGE-NATIVE-AGENT-LOWERING-V1.md), [Agent Object](LANGUAGE-NATIVE-AGENT-OBJECT-V1.md), [iterative lifecycle](AGENT-ITERATIVE-LIFECYCLE-V2.md), [Direct Runtime v2](AGENT-RUNTIME-V2.md) | Broader Proposal shapes, compiled model/effect roles, maintained packaging/public ABI, provider transport and all claimed consumer/target profiles are complete. Do not reimplement the already admitted iterative lifecycle or Runtime-v2 association as a missing feature. |
| Agent-native semantic program | Partial; AgentDefinition/AgentGraph/deployment separation, source-owned interaction facts, checked iterative typed execution, per-operation durable recovery, pure and durable migration, and linked Project/workspace associations are implemented. Frozen Runtime v1 remains a compatibility product, not the limit of current execution. | [RFC 0001](RFC-0001.md), [Agent Object](LANGUAGE-NATIVE-AGENT-OBJECT-V1.md), [interaction facts](AGENT-INTERACTION-CONTRACT-FACTS-V1.md), [payment harness](AGENT-PAYMENT-HARNESS-V1.md), [typed effects](AGENT-TYPED-EFFECTS-V3.md), [checkpoints](AGENT-OPERATION-CHECKPOINT-V2.md), [durable migration](AGENT-STATE-MIGRATION-V3.md), [linked lifecycle](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md), [linked migration](PROJECT-LINKED-AGENT-MIGRATION-V1.md) | Finish the broader [graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md), complete persistent/incremental semantic lifecycle and general intentions, target-runtime/provider/conformance evidence, separate raw-source authority and representative validation. Distributed writers, automatic reconciliation, general public ABI and native/Wasm Agent-stage execution remain separate. |
| Human-readable program | Partial; canonical `.spx`, separate source/semantic digests, ProgramRoot v1/v2/v3, exact context and source-owned Agent/contract/test facts are retained and replayed without changing frozen identities. | [RFC 0001](RFC-0001.md), [canonical workspace revision](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md), [ProgramRoot v1](PROGRAM-ROOT-V1.md), [v2](PROGRAM-ROOT-V2.md), [v3](PROGRAM-ROOT-V3.md), [exact context v2](EXACT-PROGRAM-CONTEXT-V2.md), [contracts/tests](CONTRACTS-AND-TESTS-FACTS-V1.md) | Canonical source round-trips every stable language feature with migrations and reviewable diffs; complete source/meaning coverage across the mature language. |
| Verified source semantics | Partial; exact workspace/context selection, retained HIR facts, authority-free transactions, bounded universal queries and the process-resident incremental semantic service are implemented. The stdio and MCP facades preserve their closed authority-free service contracts. | [Architecture](ARCHITECTURE.md), [Universal Query](UNIVERSAL-SEMANTIC-QUERY-V1.md), [Universal Transaction v1](UNIVERSAL-SEMANTIC-TRANSACTION-V1.md), [v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md), [composition](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md), [semantic service](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md), [stdio](PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md), [MCP](PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md) | All admitted language features reach validated HIR only after complete type, effect, contract and ownership checks; broaden shared/durable service transports, cover every mature semantic object family, preserve comments and unrelated trivia, and broaden the transaction algebra without bypassing validation. |
| Cross-backend semantic equivalence | Partial; admitted scalar, numeric-text, String, owned-data, generic and collection corpora execute through their interpreter, C11 and Core-Wasm profiles with hosted-green release evidence. | [Conformance Trace](CONFORMANCE-TRACE-V1.md), [String operations](STRING-OPS-V1.md), [owned-data API](PUBLIC-OWNED-DATA-API-V1.md), [UTF-8 API](PUBLIC-OWNED-UTF8-API-V1.md), [native String settlement](NATIVE-INLINE-STRING-SETTLEMENT-V1.md), [String interpreter](INTERPRETER-INTERNAL-STRINGS-V1.md), [Wasm Strings](WASM-INTERNAL-STRINGS-V1.md) | Every supported backend passes the same complete behavior, failure, cleanup and contract corpus for the mature language; separately specified opt-in or broader target profiles require their own evidence. |
| Atomic agent changes | Partial; exact-old-state rename, ReplaceBlock, AddContract, AddDeclaration and additive body ReplaceExpression validation/replay are implemented. Composition admits its bounded structural diff, rename rebase and ordered sibling-rename merge; validation remains authority-free and does not itself publish. | [Universal Transaction v1](UNIVERSAL-SEMANTIC-TRANSACTION-V1.md), [v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md), [composition](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md), [workflow CLI](UNIVERSAL-SEMANTIC-WORKFLOW-CLI-V1.md), [patch evidence](SEMANTIC-PATCH-EVIDENCE-V1.md), [workspace change](SEMANTIC-WORKSPACE-CHANGE-V1.md), [Candidate Git publication](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md) | General supported single- and multi-file semantic changes replay and publish atomically with recovery and provenance; preserve comments and exact unrelated trivia, broaden typed operations/preconditions, and close the remaining same-principal repository-content and publication-host hostility scope. |

### Language and safety

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial; bounded concrete and generic owned records, nested reconstruction, exact destructuring/update, multiple owners, admitted authored generic variants and owned Result execute with checked cleanup and hosted-green backend evidence. | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [owned records](OWNED-BYTE-RECORD-ALGEBRA-V1.md), [concrete generics](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md), [nested records](NESTED-OWNED-BYTE-RECORDS-V1.md), [destructuring](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md), [update](NESTED-OWNED-RECORD-UPDATE-V1.md), [variants](OWNED-BYTE-VARIANT-ALGEBRA-V1.md), [generic variants](GENERIC-AUTHORED-VARIANTS-V1.md) | Verify general owned propagation, generic package signatures, nested/resource aggregates, variants, matching, cleanup and public generic ABIs. Previously green nested-relay and generic-owned gates remain regression obligations, not unexecuted tasks. |
| Functions, closures, interfaces, implementations, generics | Partial; explicit forwarding, scoped argument inference v3, generic owned-record/result composition, compiler collections, private function values and scalar-snapshot/generic-loop closures are implemented. Graph, cleanup and ProgramRoot replay retain exact instance and mapping identities. | [Function Values v1](FUNCTION-VALUES-V1.md), [v2](FUNCTION-VALUES-V2.md), [Closures v1](CLOSURES-V1.md), [v2](CLOSURES-V2.md), [inference v3](GENERIC-ARGUMENT-INFERENCE-V3.md), [record composition](GENERIC-OWNED-RECORD-COMPOSITION-V2.md), [multiple owners](GENERIC-MULTI-OWNER-RECORDS-V1.md), [owned Result](GENERIC-OWNED-RESULT-V1.md), [forwarding](GENERIC-EXPLICIT-FORWARDING-V1.md), [compiler collections](GENERIC-COMPILER-COLLECTIONS-V1.md) | Complete constraints, broader inference and nested variant/Result composition, owning captures, general interfaces and implementations, public callable/generic ABI and generic package signatures. The admitted inference, closures and GEN-06 hosted evidence are already present. |
| `Option` and `Result`; no null or unchecked exceptions | Partial; admitted owned Result construction, matching, calls and same-typed `?` retain evaluation-once, conditional ownership, sticky failure and cleanup across the claimed engines; generic owned Result profiles are additive. | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [owned variants](OWNED-BYTE-VARIANT-ALGEBRA-V1.md), [generic owned Result](GENERIC-OWNED-RESULT-V1.md), [owned-data API](PUBLIC-OWNED-DATA-API-V1.md) | General nested owned propagation, residual conversion, public ABI and complete target behavior are verified beyond the admitted concrete and generic profiles. |
| Immutable-by-default values and explicit mutation | Partial; bounded scalar, field and immutable nested reconstruction profiles have hosted-green evidence. | [Explicit Mutation](EXPLICIT-MUTATION-V1.md), [Field Mutation](FIELD-MUTATION-V1.md), [Nested Immutable Update](NESTED-OWNED-RECORD-UPDATE-V1.md) | Verify general aggregate, collection, borrowed and concurrency-aware mutation rules. |
| Unique ownership and move safety | Partial; exact cleanup/replay covers admitted records, variants, Bytes buffers, Vec, scalar/Bytes owning iterators, consuming loops, renewal and generic map/filter/fold. Vec v2 and owned iterator payload v2 retain their separate prelude/graph/cleanup versions and no public generic ABI. | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md), [Owning Iterators](OWNING-ITERATORS-V1.md), [owned payloads](OWNING-ITERATOR-PAYLOADS-V2.md), [loops](OWNING-ITERATOR-LOOPS-V1.md), [renewal](OWNING-ITERATOR-RENEWAL-V1.md), [generic operations](GENERIC-ITERATOR-OPERATIONS-V1.md), [byte buffer](OWNED-BOUNDED-BYTE-BUFFER-V1.md), [Vec v1](OWNED-BOUNDED-VEC-V1.md), [Vec v2](OWNED-BOUNDED-VEC-V2.md), [bounded traversal](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md), [shared loans](SHARED-LOAN-PLAN-V1.md) | Verify general owned values and `?`, aliases, control flow, FFI, cleanup and public ABI. Iterator interfaces, lazy adapters and payloads beyond the exact admitted Bytes/scalar profiles remain separate. Existing iterator and generic-owned hosted selectors are regression gates, not pending first execution. |
| Owned allocation and extraction | Partial; frozen scalar Box v1/std.mem and additive `Box<Bytes>` v2 implement allocation, consuming extraction, recursive lexical cleanup, refusal-before-commit and exact graph/ProgramRoot bindings with hosted-green evidence. | [Box v1](OWNED-BOUNDED-BOX-V1.md), [Box v2](OWNED-BOUNDED-BOX-V2.md), [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) | Broader owned payloads/composition, general allocation, public ABI, regions, arenas and shared ownership are complete. Borrowed `box_get<Bytes>` remains rejected by the owning profile. |
| Borrowed views and lifetime safety | Partial; bounded shared loans, projected fields, synchronous borrowed calls and nested paths have hosted-green evidence. | [Useful Text](USEFUL-TEXT-CONSUMER-V1.md), [Shared Loan Plan](SHARED-LOAN-PLAN-V1.md), [Projected Field Borrow](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md), [Nested Records](NESTED-OWNED-BYTE-RECORDS-V1.md), [Nested Destructuring](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md), [Borrowed Calls](PROJECTED-OWNED-BYTES-BORROWED-CALL-V1.md) | Complete general lifetime inference, mutable and escaping borrows, cross-file use and public host ABI behavior. |
| Regions and arenas | Partial; report/model scope remains distinct from runtime placement. | [Region Report](REGION-REPORT-V1.md) | Region inference and runtime placement are implemented and verified; the report alone is insufficient. |
| Shared immutable ARC and managed zones | Partial; proof/model scope is unchanged by hosted execution of its tests. | [ARC Zone Model](ARC-ZONES-V1.md) | Language, runtime, cycle, escape and concurrency semantics execute on supported targets. |
| Restricted `unsafe` and raw memory | Partial | [Unsafe Boundaries](UNSAFE-BOUNDARIES-V1.md) | Raw memory operations, review policy, capability rules and target conformance are implemented and verified. |
| Checked, wrapping, and saturating arithmetic | Partial; admitted arithmetic and checked `usize` correction regressions have hosted-green evidence. | [RFC 0001](RFC-0001.md), [Indexed Byte Data](PORTABLE-INDEXED-BYTE-DATA-V1.md#checked-multiplication-correction) | All numeric widths and named arithmetic modes have complete cross-backend semantics and tests; preserve zero-multiplier and owned-cleanup regressions. |
| Effects and capabilities | Partial; admitted manifests and typed operation bindings preserve explicit authority. | [Capability Manifest](CAPABILITY-MANIFEST-V1.md), [Typed Effects](AGENT-TYPED-EFFECTS-V3.md) | Declared effects and build/runtime capabilities are enforced end to end, including dependencies and hosts. |
| Contracts and progressive verification | Partial | [RFC 0001](RFC-0001.md) | Static discharge, bounded proof, runtime obligations, counterexamples and repair evidence are integrated. |
| Structured concurrency | Partial; the bounded Rust scoped-thread runtime adds borrowed captures, stable-ID starts/reports, cooperative cancellation, panic normalization, mandatory join and invocation-owned HTTPS settlement. This is not general language task lowering. | [Scoped Task Model](SCOPED-TASKS-V1.md), [Structured Tasks Runtime](STRUCTURED-TASKS-RUNTIME-V1.md) | Language syntax, `Sendable`/`Shareable` checking, dependency scheduling, deterministic replay, native/Wasm task lowering and target task execution are verified. |
| Typed hygienic generation | Partial | [Hygienic Generation](HYGIENIC-GEN-V1.md) | General typed synthesis is scoped, hygienic, deterministic and integrated with multi-file semantics and review. |

### Compiler and output targets

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Fast development lane | Partial; interpreter, prepared Project trace and revision-replacement profiles have hosted-green release evidence. | [Interpreter](INTERPRETER-V1.md), [Internal Strings](INTERPRETER-INTERNAL-STRINGS-V1.md), [Prepared Project](PROJECT-PREPARED-INTERPRETER-V1.md), [Revision Replacement](PROJECT-PREPARED-REVISION-REPLACEMENT-V1.md) | Incremental refresh, debugging, hot reload and semantic equivalence meet the development-performance target across the supported language and platforms. |
| Optimizing native lane | Partial | [Architecture](ARCHITECTURE.md) | The production native backend covers the mature language, optimization, debug mapping and supported hosts. |
| WebAssembly core and components | Partial; admitted core/component/String profiles have hosted-green evidence; private Component execution is not a stable public Component ABI. | [Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), [Owned ABI](WASM-OWNED-ABI-V1.md), [UTF-8 API](PUBLIC-OWNED-UTF8-API-V1.md), [Wasm Strings](WASM-INTERNAL-STRINGS-V1.md), [WIT Boundary](WIT-COMPONENT-BOUNDARY-V1.md) | Stable Components, resources, capabilities, multi-engine conformance and packaging are verified for the mature supported surface. |
| Embedded and real-time | Partial | [Freestanding Profile](FREESTANDING-V1.md) | Hardware profiles, linker control, interrupts/RTOS, timing constraints and representative targets are verified. |
| SIMD and GPU | Partial | [SIMD Report](SIMD-REPORT-V1.md) | Vector/GPU lowering, legality, memory behavior, target selection and performance evidence are implemented. |

### Ecosystem interoperability

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Partial; the table manifest lowers onto frozen Project profiles; exact local dependency subjects, semantic locks, source capsules, scalar linking, generated Cargo inputs and bounded lock/resolve routes have hosted-green evidence. Internal generic ownership does not widen cross-package scalar signatures. | [Package Manifest](PACKAGE-MANIFEST-V1.md), [Project Dependencies](PROJECT-DEPENDENCIES-V1.md), [Project Lock](PROJECT-LOCK-V1.md), [Resolution](PROJECT-DEPENDENCY-RESOLUTION-V1.md), [Package Report v2](PACKAGE-REPORT-V2.md), [Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md), [Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md), [Source Capsule](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md), [Linked Wasm Build](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md) | Generic package signatures, general compatibility negotiation, supported publication, trusted provenance, registry and conformance are complete. |
| Portable canonical ABI and native fast ABI | Partial | [ABI Report](ABI-REPORT-V1.md), [Owned Data API](PUBLIC-OWNED-DATA-API-V1.md), [UTF-8 API](PUBLIC-OWNED-UTF8-API-V1.md) | Stable aggregate/resource/borrowed ABIs and cross-language conformance cover supported architectures. |
| C and Objective-C | Partial; admitted C11 provider/consumer profiles cover owned bytes, compiler-owned Option/Result, UTF-8, flat records and nested multi-owner settlement at O0/O2 with hosted-green release evidence. | [C Header](C-HEADER-V1.md), [C/C++ Owned Data Package](PUBLIC-CXX-OWNED-DATA-PACKAGE-V1.md), [Flat Record API](PUBLIC-FLAT-OWNED-RECORD-API-V1.md), [UTF-8 API](PUBLIC-OWNED-UTF8-API-V1.md), [Nested Record API](PUBLIC-NESTED-OWNED-RECORD-API-V1.md) | Broader import/export, authored-variant/resource ownership, Objective-C adapters, cross-platform consumers, compatibility, maintained distribution and supported-host conformance are verified. |
| C++ | Partial; admitted scalar, Project-v8 owned-data and Project-v9 flat-record adapters have hosted-green compiled-consumer evidence. | [C++ Shim](CXX-SHIM-V1.md), [Scalar Package](CXX-PACKAGE-V1.md), [Owned Data Package](PUBLIC-CXX-OWNED-DATA-PACKAGE-V1.md), [Flat Record Adapter](PUBLIC-FLAT-OWNED-RECORD-CXX-ADAPTER-V1.md) | Cross-platform/MSVC consumers, broader aggregate failure and borrowed-lifetime profiles, maintained distribution, compatibility and supported-host conformance are complete. |
| Java and Kotlin | Partial; private JNI/emulator evidence remains private. | [Android JNI Ownership](ANDROID-JNI-OWNERSHIP-V1.md) | Public JVM/JNI artifacts, ownership, exceptions, packaging and conformance are verified. |
| Swift and Apple frameworks | Partial; private framework/simulator evidence remains distinct from public/device support. | [Swift Ownership](APPLE-SWIFT-OWNERSHIP-V1.md) | Public Swift/Objective-C API, distributable frameworks, lifecycle, ownership and device evidence are verified. |
| JavaScript and TypeScript | Partial; admitted Node/TypeScript/browser and generated owned-data/String consumers have hosted-green evidence. | [Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), [Owned Data API](PUBLIC-OWNED-DATA-API-V1.md), [UTF-8 API](PUBLIC-OWNED-UTF8-API-V1.md), [Wasm Strings](WASM-INTERNAL-STRINGS-V1.md), [String Web Package](WASM-INTERNAL-STRINGS-WEB-V1.md) | Stable general bindings, owned resources, async/callbacks, maintained packaging and the claimed multi-engine browser/runtime breadth are verified. |
| WIT and WebAssembly Components | Partial; scalar interface projection and private Component runtime profiles have hosted-green evidence. | [Public Scalar WIT](PUBLIC-SCALAR-WIT-INTERFACE-V1.md), [Private WIT Boundary](WIT-COMPONENT-BOUNDARY-V1.md) | Extend the retained Project-v1 scalar interface artifact into supported Component publication; source-selected interfaces and resources run through a supported Component Model toolchain on multiple runtimes. |
| OpenAPI, Protobuf/gRPC, GraphQL, and SQL | Partial; existing OpenAPI projection does not implement every named schema family. | [OpenAPI](OPENAPI-V1.md) | Import/export, compatibility, live conformance and all named schema families are verified. |
| Standard library | Partial; the current bundled core, portable, alloc, hosted, agent and test packages have hosted-green evidence for their listed profiles. The generated catalog owns the exact inventory. Implemented additions include authenticated Vec/Box aliases, JSON decoding and cursor adapters, Reader/Writer and formatting/logging, typed paths, bounded filesystem/environment/process I/O, byte assertions/snapshots and private linked std.agent roles. The additive `std.io.lines` sibling package (view helpers, borrowed line observers, a preflighted line copy into caller capacity and the consuming line transition, with graph-pinned per-shape cleanup-schema selection) executes on the interpreter, C11 `-O0`/`-O2` and repeated Core Wasm with local evidence only; it is outside the v0.4.0 hosted baseline. The additive `std.path.normalize` sibling package adds lexical normalization of typed Path values (separator-run collapse, `.` removal, `..` cancellation, root and empty-result policy) with the same three-backend local evidence and a graph-pinned cleanup-schema selection. | [Standard Library](STANDARD-LIBRARY-V1.md), [catalog](STANDARD-LIBRARY-CATALOG.md), [JSON Cursors](JSON-CURSORS-V1.md), [IO Cursors](IO-CURSORS-V1.md), [IO Lines](IO-LINES-V1.md), [Path Normalization](PATH-NORMALIZATION-V1.md), [Typed Path](TYPED-PATH-V1.md), [Filesystem v2](FILESYSTEM-IO-V2.md), [Environment I/O](BOUNDED-ENVIRONMENT-IO-V1.md), [Process I/O](BOUNDED-PROCESS-IO-V1.md), [Byte Assertions](TEST-BYTE-ASSERTIONS-V1.md), [Linked Agent Lifecycle](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md), [Project v16](PROJECT-MANIFEST-V16.md), [Project v18](PROJECT-MANIFEST-V18.md) | Every required module exists at its tier with identities, contracts, effects, examples, conformance on every listed target and generated documentation; the Everyday profile and remaining offline templates ship. Full streams/traversal, broader physical providers, general Agent support and richer testing remain outside their bounded current slices. |

### Application platforms

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| First-class application/state/UI dialect | Partial | [UI Schema](UI-SCHEMA-V1.md) | Typed state/update/view, semantic controls, accessibility, navigation, assets and platform escape hatches execute. |
| Web | Partial | [Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md) | Accessible DOM/CSS, SSR/hydration, packaging, multi-engine execution and a deployable sample are verified. |
| iOS | Partial | [Swift Ownership](APPLE-SWIFT-OWNERSHIP-V1.md) | Public framework/app generation, lifecycle, accessibility, signing metadata and device/simulator samples are verified. |
| Android | Partial | [Android JNI Ownership](ANDROID-JNI-OWNERSHIP-V1.md) | Public AAR/app generation, lifecycle, accessibility, packaging and emulator/device samples are verified. |
| macOS | Partial | [Desktop App](DESKTOP-NATIVE-APP-V1.md) | Public host/UI generation, lifecycle, accessibility, packaging, signing/notarization and a sample are verified. |
| Windows | Partial | [Desktop UI](DESKTOP-NATIVE-UI-V1.md) | Public host/UI generation, lifecycle, accessibility, MSIX/signing metadata and a sample are verified. |
| Linux | Partial | [Roadmap](ROADMAP.md) | A supported UI/runtime adapter, accessibility, distribution formats and a representative application are verified. |
| Edge and server | Partial; bounded TCP/TLS/listener and HTTPS operations, fixture replay, aggregate deadlines, caller-selected providers, loopback browser execution and the admitted native libcurl adapter have hosted-green evidence. A cross-compiled Windows branch is not physical Windows execution. | [Language Network I/O](BOUNDED-LANGUAGE-NETWORK-IO-V1.md), [Network Services](BOUNDED-NETWORK-SERVICES-V1.md), [HTTPS Runtime](HTTPS-CLIENT-RUNTIME-V1.md), [HTTPS I/O](HTTPS-CLIENT-IO-V1.md), [Project Manifest](PROJECT-MANIFEST-V1.md) | Live browser service adapters, multi-engine evidence, HTTP/3, cross-platform libcurl provisioning, DNS policy, structured async services, observability, deployment and load/conformance tests are verified. |
| Plugins | Partial | [Plugin Manifest](PLUGIN-MANIFEST-V1.md) | Capability-limited loading, lifecycle, compatibility, resource limits, packaging and hostile-plugin tests are verified. |

### Agent economics, review, and operations

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial; bounded standalone and authenticated Project context use retained typed indexes without requiring full graph transfer. | [Agent Context v2](AGENT-CONTEXT-V2.md), [Economics](AGENT-ECONOMICS-V1.md), [Workspace Image](SEMANTIC-WORKSPACE-IMAGE-V1.md) | Exact model-token budgets, broader semantic edges, persistent indexing and representative measured savings are verified. |
| Impact analysis before modification | Partial | [Semantic Impact](SEMANTIC-IMPACT-V1.md), [Workspace Image](SEMANTIC-WORKSPACE-IMAGE-V1.md) | Repository-wide call/type/contract/test/schema/target/capability consumers are complete and incremental. |
| Typed holes and compiler-generated repairs | Partial; installed SPX-S103 catalog/plans, typed Candidate holes and their admitted repair routes have hosted-green evidence; plans remain authority-free and do not rank or apply arbitrary repairs. | [Diagnostic Repair](DIAGNOSTIC-REPAIR-V1.md), [Installed Fix Plan](INSTALLED-FIX-PLAN-V1.md), [Candidate Holes](PROJECT-CANDIDATE-HOLES-V1.md) | General obligations and composable sound repairs are generated, ranked, reviewed and replay-verified. |
| Proof-carrying patches | Partial | [Patch Evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md) | General semantic claims, tests, targets, capability deltas, provenance and compatibility are independently verified before commit. |
| Semantic human review | Partial | [Semantic Review](SEMANTIC-REVIEW-V1.md) | Complete repository-wide behavioral, API, security, memory, target, migration and unsafe summaries are evidence-backed. |
| Graph-derived documentation | Partial; checked module documentation, generated library/shapes catalogs, bounded topic/diagnostic help and version-matched installed guidance are implemented. Their hosted-green tests do not make the bounded shapes a complete grammar or node catalogue. | [Documentation Projection](DOC-PROJECTION-V1.md), [Guided CLI Help](CLI-HELP-V4.md), [Installed Guidance](INSTALLED-AGENT-GUIDANCE-V1.md) | Document whole projects and their `use` closure, generate bundled agent skills and standard-library catalogs from one complete stable semantic graph document, and expose the projection through editor and workspace sessions. |
| Unified 1.0 command surface | Partial; unified verify/review/agent/query/package/add/fetch/doc, exact installed guidance/diagnostics/fix-plan adapters, bounded service transports and scripted Agent run/replay are implemented. `doctor` is dispatched by the shared driver in both binaries; native Rust package building and the full staged-publication hooks remain private-host operations. | [Unified CLI](UNIFIED-CLI-V1.md), [Workflow CLI](UNIVERSAL-SEMANTIC-WORKFLOW-CLI-V1.md), [Installed Guidance](INSTALLED-AGENT-GUIDANCE-V1.md), [Installed Diagnostics](INSTALLED-DIAGNOSTICS-V1.md), [Fix Plan](INSTALLED-FIX-PLAN-V1.md), [Service Transport](PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md), [Service MCP](PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md) | Admit remaining verbs, including unadmitted Agent resume/reconcile routes, with their own gates; make the private-host split invisible to promoted workflows and drive editor surfaces from the same catalog. |
| Editor integration by meaning | Partial; saved-source diagnostics, stable-identity navigation, Project-routed context, code lenses, semantic review/rename and bounded Agent/candidate views are implemented. The historical local Extension Host witness retains its original subject and platform; current release evidence is hosted green within admitted editor gates. | [VS Code Adapter](VSCODE-SAVED-SOURCE-ADAPTER-V1.md), [Unified CLI](UNIFIED-CLI-V1.md), [VS Code Host Evidence v2](GRAPH-OPERATIONAL-VSCODE-HOST-EXECUTION-EVIDENCE-V2.md) | Drive the remaining editor surfaces, packaging and manual UI from the same catalog and verify the claimed host/platform breadth. A historical single-platform witness does not establish every possible editor or environment. |
| Sandboxed builds and dependencies | Partial; exact held Project subjects, generated Cargo inputs, semantic locks and bounded linked builds have hosted-green release evidence. Held source authority and absence of implicit tool execution are not a hermetic OS sandbox. | [Project Dependencies](PROJECT-DEPENDENCIES-V1.md), [Capability Manifest](CAPABILITY-MANIFEST-V1.md), [Offline Lock](OFFLINE-PACKAGE-LOCK-V1.md), [Resolver](OFFLINE-PACKAGE-RESOLVER-V1.md), [Source Capsule](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md), [Pure Wasm Build](OFFLINE-PURE-WASM-PACKAGE-BUILD-V1.md), [Linked Wasm Build](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md) | Verify reproducible acquired inputs, generic package signatures, supported publication and actual least-authority OS sandbox/dependency enforcement. |
| Debugger, profiler, diagnostics, and operations | Partial; bounded installed static diagnostic inventory and exact identity/provenance explanation have hosted-green evidence. Static presence is not complete runtime reachability, wording, repair knowledge or backend coverage. | [Architecture](ARCHITECTURE.md), [Human Diagnostics](HUMAN-DIAGNOSTICS-V1.md), [Installed Diagnostics](INSTALLED-DIAGNOSTICS-V1.md) | Source-level debugging/profiling, crash and trace mapping, complete runtime diagnostic semantics and repair guidance, observability and deployment diagnostics cover every backend. |

## Final validation product

Completion requires one maintained offline-first product built from a shared
SEMAPRAX codebase with web, iOS, Android, macOS, Windows, and Linux clients;
native notifications and secure storage; local databases; native or WASI
server execution; authentication; background synchronization; a custom
accelerated visual; one C library; one JavaScript package; and one WebAssembly
component.

Every artifact must be built and exercised in CI or on representative
simulators/devices. Platform-specific implementations must be declared rather
than hidden behind false portability. No current narrow prototype satisfies
this final gate. The v0.4.0 hosted-green release advances the implemented
slices without claiming this mature-product completion.
