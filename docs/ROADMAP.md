# Roadmap

Status: living future-sequencing document, reconciled with the **HOSTED GREEN**
v0.4.0 implementation baseline. This roadmap is not execution evidence.

Audience: contributors, maintainers, and project evaluators.

The roadmap orders work that remains. Use the
[completion matrix](COMPLETION-MATRIX.md) for current product claims, the
[v0.4.0 baseline](RELEASE-0.4.0-STATUS.md) for accepted release evidence, and
the [changelog](https://github.com/wavect/semaprax/blob/main/CHANGELOG.md) for
implementation history. Versioned specifications own exact admission,
compatibility, and support boundaries.

SEMAPRAX follows risk rather than feature count. Stable semantic identity,
sound ownership, replayable change authority, and explicit target boundaries
take priority over broad syntax or generated artifact volume. Completing a
bounded implementation does not reduce the mature-product requirement.

## Current release baseline

The separate [Persistent Semantic Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md)
implements authenticated cross-process checked-HIR reuse with independent
source/HIR validation. Its release regressions are HOSTED GREEN; full
incremental compilation and measured task-level performance remain open.

The v0.4.0 code baseline is **HOSTED GREEN**. Its published three-archive
milestone is complete; see the
[release record](RELEASE-PROCESS.md#040-hosted-release-evidence). Completed
release checks are regression obligations for future code changes, not an
unexecuted implementation backlog. Historical local runs retain their original
subjects, counts, and timings; they are no longer the released implementation's
only evidence classification.

The full product remains **Partial**. Public package publication, stable API
support, broader target/provider functionality, and the final validation
application are distinct from the accepted release. An implemented private
profile remains private until its owning support decision changes.

The numbered 0.3 and 0.4 headings below retain their established workstream
names and link anchors. They describe remaining ownership and ecosystem
outcomes, not unpublished release versions or an unsuccessful v0.4.0 release.

## Unified semantic program implementation sequence

Canonical source, ProgramRoot, semantic graph, source Agent, Proposal grammar,
deployment, executable harness, checkpoint, semantic transaction, target
artifact, and evidence must be authenticated projections of one checked
program. Do not introduce a competing static graph or use a runtime receipt
as mutation, execution, or publication authority.

| Workstream | Released foundation, HOSTED GREEN | Next implementation outcome |
| --- | --- | --- |
| GEN-05B/C: semantic closure | Exact concrete generic identities, substitutions, ownership/call facts, cleanup replay, ProgramRoot association, flat/nested composition, and scalar dependency coverage | Preserve the completed internal closure and frozen historical schemas while extending genuinely new generic semantics. |
| GEN-06: internal generic semantics | Owned Result propagation and `?`, explicit nonidentity forwarding, nested record reconstruction, multiple record owners, authored variants, compiler collections, and argument inference through v3 | Broaden constrained generic semantics, evidence expressions and carrier composition without implicit ownership conversion or public ABI widening. |
| AGENT-06: iterative lifecycle | Checked Continue/Complete/Suspend/Fail, multiple typed effects, per-turn authorization, Direct Runtime v2, per-operation durable checkpoints, pure/durable State migration, and generated Proposal clients | Extend provider integration, Proposal/result shapes, target execution and maintained packaging; preserve the implemented durable and linked-role paths rather than rebuilding a one-pass bridge. |
| SEG-04: static/runtime association | ProgramRoot, DeploymentRoot, InstanceRoot, EvidenceRoot and ExecutionRevision associations, exact workspace generation selection, currentness checks, linked Project roles and linked/workspace migration | Complete the broader durable semantic-service lifecycle and shared runtime/tooling projections without making receipts authoritative. |
| LANG-07: collections | Scalar and Bytes Vec/Box profiles, consuming iterators and loops, owned Bytes traversal, conditional renewal, function values, scalar-snapshot/generic closures, and bounded generic map/filter/fold | Add broader owned payloads, iterator interfaces and lazy adapters, owning captures, general lifetime rules and intentionally supported package/ABI surfaces. |
| STD-08: Everyday profile | The bundled library includes bounded Reader/Writer, typed Path, filesystem/environment/process I/O, JSON cursors, formatting/logging, byte assertions/snapshots and private linked `std.agent` roles | Complete each required module's remaining scope and target/provider availability, then ship the complete Everyday profile and remaining offline templates. |
| ABI-09: public generic programme | Existing public profiles remain separately closed; internal generic implementation does not change their signatures | Specify a versioned target-neutral type grammar, ordered template identities, compatibility and ABI-delta evidence before generating and supporting public generic consumers. |

The owning references for these released foundations are collected in the
[release implementation map](RELEASE-0.4.0-STATUS.md#released-implementation-map).
The following sequence starts from those implementations, not their earlier
local-only or specification-only status paragraphs.

<a id="current-priority-post-v02-promotion-boundaries"></a>

## Current priority: post-v0.4 promotion boundaries

Keep three decisions separate: whether a feature exists, whether its admitted
release corpus is green, and whether an API/package/target is publicly supported.
The first two are settled for the implemented v0.4.0 slices. The third remains
explicit where the owning specification still marks a surface private,
unpublished, or unpromoted.

1. Identify the precise public surface to support: generated Rust/npm packages,
   Project v8-v11 profiles, read-only transports v5/v6, or a platform adapter.
   Retain the existing descriptor, HIR, ownership, settlement and legacy
   compatibility contracts. Do not treat registration or publication as a
   consequence of a CI label.
2. Add only the coverage required by a genuinely broader claim: additional
   browser engines, real devices, architecture/toolchain combinations,
   installed-archive workflows, or hostile physical hosts outside the admitted
   release profile. An explicitly ignored test is not selected merely because
   its harness runs. Conversely, an already selected green gate is not pending.
3. Complete missing integration, including any still-unimplemented held-image
   handoff from the signed doctor generation store to the Linux provisioner.
   Equivalent production confinement on macOS and Windows is a separate
   implementation problem, not a failed Linux release check.
4. Record the support/publication decision, its exact version and target scope,
   and any prerequisite profile decisions. Require fresh evidence for later
   code changes rather than attributing them to v0.4.0.

The full-toolchain archives remain pre-alpha and do not publish workspace-private
library crates. The source-selected/private-host distribution boundary must
remain explicit until promoted workflows intentionally hide it from users.

## Developer preview: promote the authored Project v8 slice

The heading retains its existing link anchor; the Project v8 slice is now
implemented and **HOSTED GREEN**, not merely authored. Its canonical descriptor,
reference interpreter, npm/Core-Wasm carrier, safe Rust consumer route,
frame-payload product, and read-only transport retain the exact
[Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) contract.

Next, decide which generated package and runtime/browser breadth will be
supported. Preserve v1-v7 known answers, independent descriptor/carrier replay,
copy-out and failure settlement, private-frame exclusion, baseline/display-rename
identity and no-clobber publication. The direct-Bytes browser fixture and its
hostile-input/capacity/authentication cases remain the owning regression gate;
add engines only for a broader stated support target. Do not infer such breadth
from the frame-format corpus alone.

[Project Agent Transport v5](PROJECT-AGENT-TRANSPORT-V5.md) is the implemented
read-only descriptor/inline-carrier route. [Transport v6](PROJECT-AGENT-TRANSPORT-V6.md)
and its [generated SDK](PROJECT-AGENT-TRANSPORT-V6-SDK-V1.md) extend that narrow
operation class across exactly Project v8-v11. Their implementation evidence is
hosted green. Support still requires an intentional transport/client packaging
decision and the prerequisite Project-profile decisions; no read-only method
acquires write, process, or publication authority.

The additive profiles already implemented after v8 are:

| Profile | Existing bounded implementation | Remaining support boundary |
| --- | --- | --- |
| Project v9 | [Flat owned records](PUBLIC-FLAT-OWNED-RECORD-API-V1.md), ordinary profile admission, descriptor-bound Wasm/npm and native/Rust routes, and the [safe C++ adapter](PUBLIC-FLAT-OWNED-RECORD-CXX-ADAPTER-V1.md) | Explicit v9 promotion, maintained packages and additional aggregate/physical-host scope required by that decision. |
| Project v10 | [Owned UTF-8](PUBLIC-OWNED-UTF8-API-V1.md), exact-length validated strings, native/Wasm ownership and consumer settlement | Prerequisite v9 promotion and an explicit v10 support decision; raw Bytes and validated String remain different contracts. |
| Project v11 | [Nested owned records](PUBLIC-NESTED-OWNED-RECORD-API-V1.md), stable-field-ID paths, private multi-handle carriers and bounded multi-owner settlement | Prerequisite v9/v10 decisions and explicit v11 package/support scope. |

The v8 boundary still excludes authored records/variants, nested algebraic data,
owned UTF-8, allocator transfer, callbacks, async work and general aggregate ABI.
Their existence in later or private profiles does not widen v8. The
[profile-admission dispatcher](PROJECT-PROFILE-ADMISSION-V1.md) remains the
ordinary Phase-A authority-neutral gate; it is not itself promotion.

Native and standalone-Wasm internal String cleanup/contents corrections are
implemented. Preserve their existing physical allocation, failure, exact-length,
embedded-NUL, sanitizer and external-consumer gates. Broaden ordinary or public
String admission only through the owning contract, not to make a fixture pass.

## Graph-operational development foundation

The [graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md) remains the
complete requirement ledger. Its bounded implemented slices have the accepted
release evidence; operation count and generated artifact volume do not complete
the programme.

### Agent execution and revision changes

Build on source-owned Agent declarations, checked interaction facts, generated
Proposal clients and the current [typed iterative runtime](AGENT-RUNTIME-V2.md).
The old one-pass lifecycle and frozen Runtime v1 compatibility adapter remain
compatibility profiles, not the current implementation ceiling.

Per-operation checkpoints, trusted-store recovery, pure State migration,
persisted handoffs, repeated migration chains, imported Project roles and
workspace associations are implemented. Preserve consumed per-turn grants,
exact root/invocation binding, nonrefundable cumulative work, zero redispatch
of recorded observations and refusal of uncertain intent. Next outcomes are
broader providers and nominal Proposal/result shapes, native/Wasm Agent-stage
execution, supported packaging/public ABI, distributed writer coordination,
automatic reconciliation and any stronger cross-store handoff contract.
Ordinary library execution on native/Wasm is not Agent-stage execution there.

### Semantic changes, queries and service lifecycle

Existing image, candidate and draft APIs provide bounded typed changes,
replayable deltas, holes, static conformance, diagnostic repair, signature and
field migration, cross-file moves, rebase/merge, tests and source review. The
universal transaction family is separately scoped: [v2](UNIVERSAL-SEMANTIC-TRANSACTION-V2.md)
adds revision-scoped authored-body ReplaceExpression, and
[composition v1](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md) adds its exact
structural diff, rename rebase and ordered sibling-rename merge. Do not describe
all composition as missing, or those admitted operations as general merging.

Extend these into general ownership-sensitive intentions, broader interfaces,
contracts, semantic conflict handling and incomplete-expression states.
Comments and unrelated trivia must be preserved by any newly claimed editing
route. Exact nominal/field/case identity and caller-independent replay remain
required. Static protocol mappings do not implement runtime interfaces or
dynamic dispatch.

ProgramRoot v2/v3, exact context, query/transaction replay and retained
service-history selection already compose through the implemented semantic
service. The separately versioned [stdio](PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md)
and [MCP](PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md) facades expose their bounded
authority-free routes. Complete candidate-safe dependency-lock integration
where a route still rejects it; do not copy old external facts into a successor.
Broader durable/shared service state, warm checked-HIR reuse, incremental
semantic invalidation/rechecking and measured complete-workflow savings remain
separate from the implemented exact-source frontend cache.

### Recovery, retention and publication

Source-backed images, candidate/draft archives, held private stores, typed
receipts, retention checkpoints, startup selection and the automatic durable
candidate/draft lifecycle are implemented. Their recovery rebuilds checked
source; it does not restore write approval, make historical source current or
constitute warm HIR persistence. Complete workspace-session startup integration,
authorised eviction/garbage collection and measured recovery cost without
weakening no-clobber or no-adoption rules.

Preserve the supported two-session publication workflow: review/export, then
restore and publish through a new independently host-approved commit session.
No request or later approval can relax startup authority, and no retry resets
an external provider's deadline. Managed `ACTIVE` publication remains distinct
from canonical Git publication and from visibility to arbitrary raw-path readers.

The admitted integrated Git workflows and Linux/macOS bare SHA1/SHA256 providers
are implemented with hosted-green release evidence. Maintain exact readback,
content/ref association, held executable/repository identity, failure settlement,
leader reap and group quiescence. Broader checkout/host interoperability,
interactive approval, long-lived publication authority and same-principal
repository-content hostility remain separately scoped work.

### Supported clients and editor workflows

Generated TypeScript, Python and Rust clients, typed application diagnostics,
per-step response accountability, the bounded supported signature workflow,
source-backed recovery handoff, the zero-authority workflow package, and its
MCP adapter are implemented. Their admitted release evidence is hosted green;
local bare-Git transcripts remain local transcripts with their original subject.
A package transport does not by itself establish registry support, every MCP
host or broader workflow semantics.

Complete the explicitly opaque payload references before claiming full
semantic-report validation. Extend editor task execution, packaging and host
breadth deliberately. Preserve the bounded candidate-test task's cooperative
cancellation and source/session invalidation without making publication
cancellable. The signature-change repair catalogue remains empty where the
compiler cannot soundly generate a repair; a suggested new review is not an
executed repair. General scheduling, broader repairs and representative
end-to-end task/token economics remain open.

## 0.3: ownership and fast development

Goal: complete language safety and the fast development loop without widening
public ABIs prematurely. This is a retained workstream name, not a pending
v0.3 release.

### Language and ownership outcomes

The released foundation includes bounded concrete/generic owned records,
recursive destructuring and immutable update, owned variants and Results,
shared/projected loans, synchronous borrowed calls, byte buffers, Vec/Box,
consuming iterators and the current function-value/closure profiles. Preserve
their admitted interpreter, native C11 O0/O2 and Core-Wasm behavior and exact
source/HIR/graph/cleanup/ProgramRoot replay.

Next outcomes:

- Generalize ownership and `?` across additional nested carriers, calls, control
  flow and FFI. Extend generic constraints, inference and result composition
  beyond their explicit current domains; nominal reconstruction is not a
  layout-based cast. Retain all-eight-scalar, multi-owner and nonidentity
  forwarding regressions and the concrete-instance closure bound.
- Extend borrowing with general lifetime inference, mutable/escaping borrows,
  cross-file lifetime meaning and intentionally public borrowed APIs. Preserve
  the implemented projected/nested shared-loan paths and exact no-use-after-move
  diagnostics.
- Extend collections beyond the current scalar/Bytes payloads and exact
  capacity/renewal rules. General iterator interfaces, associated types, lazy
  adapters, owning closure captures, general collection mutation and public
  generic descriptors remain distinct from the implemented bounded loops and
  map/filter/fold helpers.
- Give regions/arenas and shared immutable ARC executable language/runtime
  counterparts. Proof/report models do not perform allocation or physical
  finalization. Define restricted raw-memory operations and an auditable unsafe
  policy before claiming general raw-memory support.

### Development-loop outcomes

Build on the implemented internal-String interpreter/Wasm/Web-package routes,
prepared Project interpreter/source trace, same-worker revision replacement,
revision stores and exact-source frontend-cache reuse. These are not pending
first hosted execution. Their bounded admission and support decisions remain
owned by their specifications.

Complete incremental semantic checking and dependency-aware invalidation,
warm cross-process HIR reuse, source-level debugging/profiling and target-runtime
trace mapping. Broaden context and impact edges and measure useful task-level
savings with representative repositories and actual model tokenizers.

Preserve Unix and Windows revision-store authority as separate explicit
contracts. A held authenticated input store is neither an ambient cache nor a
verifier bypass; Windows SID/DACL/NTFS handling is not inferred from Unix
permissions. Recovery and source refresh must preserve prior state on rejection.

Exit condition: representative owned applications pass the same success,
failure, cleanup and contract corpus through development, native and
WebAssembly lanes, with stable source/graph migrations.

## 0.4: components, packages, and interoperability

Goal: turn the implemented bounded ecosystem into intentionally supported,
versioned package and host surfaces. The v0.4.0 artifact release is complete;
this wider ecosystem objective remains Partial.

### Package outcomes

Package reports, semantic locks, bounded offline resolvers, source capsules,
linked scalar Core-Wasm builds, safe local publication and the table manifest
are implemented with hosted-green evidence. The exact v1/v2/v3 report, lock,
resolver and source-subject relationships remain separately versioned. A held
source subject or deterministic build manifest is not trusted publisher
provenance, an acquired registry package or a hermetic OS sandbox.

Next, deliver generic cross-package signatures, broader semantic compatibility
negotiation, an intentionally supported lock/resolve/build workflow, registry
and offline acquisition/cache policy, licenses/provenance and reproducible
artifact records. Preserve target/capability intersection and strict source/
interface replay. Complete migrations across language, graph, patch, package
and ABI schemas without reassigning frozen identities.

### Standard library outcomes

[Standard Library v1](STANDARD-LIBRARY-V1.md) owns the complete required module
set and tier contracts; the [generated catalogue](STANDARD-LIBRARY-CATALOG.md)
owns exact declarations. The v0.4.0 tree contains 36 packages: nine core,
eighteen portable, three alloc, three hosted, one agent and two test. Their
implemented profiles have hosted-green release evidence, while their full
required module scope remains Partial.

Already implemented are the authenticated scalar Vec/Box aliases, scalar and
Bytes collection extensions, private iterator helpers and operations, JSON
cursor adapters, Reader/Writer, bounded line processing over those cursors,
typed paths, filesystem/environment/process profiles, formatting/logging and
byte assertion/snapshot helpers. Line processing and lexical path
normalization ship as the sibling `std.io.lines` and `std.path.normalize`
packages, `std.format` gained field padding for aligned output, `std.bytes`
gained trimming and delimited-field span cursors, `std.log` gained explicit
level filtering, `std.data.csv` gained quote-aware field cursors, and `std.test`
gained a diagnosable failure-mask discipline; their gates are hosted-green at
1dfe12a6 under the named `std-library-depth` job and the `verify-tests` shards. The private
`std.agent` package supplies ordinary checked records and deterministic roles;
its values do not grant runtime capabilities or complete the full Agent library.

Finish composable streams/traversal, broader filesystem/process/network provider
profiles, missing required modules, richer testing and the complete Everyday
profile. Broaden the bounded bundled `use` path into supported package builds.
Moving operations into `std.*` must preserve stable identity, checked contracts,
effects and target facts, rather than adding handwritten metadata detached from
compiler meaning. Ship the remaining offline `cli|service|web|agent` templates;
the existing `library` and calculator/scaffold routes remain regression baselines.

### ABI and host outcomes

Specify and support general aggregate, resource, borrowed-view, String, error,
callback and async ABIs. Public generic ABI remains its own programme after
internal semantic closure. Generate and exercise Rust, TypeScript/Wasm, C and
C++ consumers against versioned identities, semantic compatibility and
candidate ABI deltas before declaring support.

Build on existing private and bounded C/C++, Rust, Java/Kotlin, Swift/Objective-C,
JavaScript/TypeScript and WIT adapters. Add intentionally supported distribution,
architecture/toolchain coverage and maintained consumers. Complete Component
Model publication, resources and multiple runtime execution, plus
capability-limited plugin loading and hostile-plugin coverage. A private
Component fixture is not a stable public Component ABI; a simulator is not a
physical device.

Exit condition: one versioned package is consumed from every supported host
language and target lane with reproducible builds, compatibility checks and
no undocumented ambient authority.

## 0.5: concurrency and applications

Goal: demonstrate that verified shared meaning supports real applications
without pretending every platform is identical.

### Concurrency and services

The bounded Rust scoped-thread runtime, fixture-backed command/network I/O,
TCP/TLS/listener operations, HTTPS client routes, aggregate deadlines and
caller-selected handlers are implemented. Their admitted release evidence is
hosted green. Explicit host-policy server-side TLS acceptance is implemented
in [Network Services v1](BOUNDED-NETWORK-SERVICES-V1.md); it is not a wholly
future feature. Native libcurl, fixture-backed Web and loopback-browser evidence
retain their exact provider/host scope.

Extend this foundation into language task syntax, task HIR/graph nodes,
`Sendable`/`Shareable` checking, deterministic schedule replay, dependency
scheduling and native/Wasm task lowering. Complete broader live browser service
adapters, server request parsing and production service integration, DNS policy,
HTTP/3, structured async services and cross-platform provider provisioning.
Finish general capability-controlled command, filesystem, network and clock I/O,
then server/edge packaging, observability, deployment diagnostics and
load/conformance tests.

### Application model

Deliver typed state/actions/update/view, semantic controls, navigation,
localization, assets, accessibility and lifecycle. Build accessible DOM/CSS and
SSR/hydration on the web, plus intentionally supported Apple, Android, Windows,
macOS and Linux application adapters. Preserve explicit platform blocks and
custom accelerated-rendering escape hatches.

Create distributable artifacts with permissions, entitlements, manifests and
signing metadata while credentials remain outside compiler authority. Private
framework/JNI/desktop fixtures are foundations, not completed application
platform support.

Exit condition: one shared application has maintained web, iOS, Android,
macOS, Windows and Linux clients with declared platform differences and
representative hosted or device evidence.

## 1.0: validate the complete programming system

The 1.0 gate is the [final validation product](COMPLETION-MATRIX.md#final-validation-product),
not a version-number aspiration. It requires a maintained offline-first product
with all six client platforms from shared SEMAPRAX source; native notifications,
secure storage, local databases, authentication and background synchronization;
native or WASI server execution; a custom accelerated visual; one C library,
one JavaScript package and one WebAssembly component.

Every claimed artifact must be built and exercised on its representative
CI/simulator/device lane with compatibility, migration and reproducibility
evidence. Complete language safety, diagnostics/debugging/profiling, package,
capability and operations gates for the features used. No narrow report,
generated fixture, private adapter or successful release substitutes for this
maintained end-to-end product.

## Research profiles after the core product

Economic-agent work remains optional and subordinate to the language's
authority model. The current injected-host policy and evidence core grants no
built-in provider transport, wallet, key, mainnet, or signing authority. Any
future profile must preserve explicit capabilities, approvals, custody
separation, idempotent settlement, private-data boundaries, and complete audit
traces without weakening the core product gates.
