# Full-goal completion matrix

Status: living internal audit of the current product contract.

Audience: maintainers, contributors, reviewers, and technical evaluators.

This document is the authoritative status audit for the complete SEMAPRAX
objective. It deliberately separates three questions:

1. What must the mature product do?
2. What bounded evidence exists today?
3. What still has to be proven before a row is complete?

Historical status transitions belong in the [changelog](../CHANGELOG.md).
Protocol details, exact known-answer digests, test counts, and CI run IDs belong
in the linked versioned specifications. Future sequencing belongs in the
[roadmap](ROADMAP.md).

## Status rules

| Status | Meaning |
| --- | --- |
| Implemented | The full completion gate is covered by executable evidence on every required target. |
| Partial | Useful executable evidence exists, but the full completion gate remains open. |
| Missing | No qualifying executable evidence exists for the row. |

Design text, generated placeholders, source compilation alone, a proof-only
model, or a narrower private target does not complete a broader row. Local,
hosted, private, public, and proof-only evidence are distinct.

In the programme audit below, **Authored, unrun** means that implementation and
executable evidence are present but the row's required programme has not been
executed. **Local, partial** records only the selected local gates identified
by the evidence owner, not an entire work package or the current hosted head.
Neither state supports a hosted or public promotion claim.

## Current summary

**Overall product objective: Partial**

The long-term contract below contains **50 requirements: 50 Partial, 0
Implemented, 0 Missing**. Each row has at least one bounded executable slice,
but none satisfies its full gate.

The previous “56 Partial / 0 Missing” dashboard mixed the earlier 49 long-term
requirements with seven later milestone rows; the standard library row was
added when its first executable slice landed. That made the denominator change
as work was added. This matrix now keeps the product contract fixed and tracks
release-specific evidence separately.

The strongest current vertical slices are:

- canonical source, stable-ID HIR, and deterministic semantic graph queries;
- bounded replay-checked single-file and managed-workspace changes;
- scalar and selected Copy/owned-data execution through interpreter, native
  C11/Clang, and Core WebAssembly/Node lanes, including the closed flat
  [Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) slice;
- bounded multi-file Project manifests through the additive Project v8
  `owned-data-api.v1` implementation, with one canonical descriptor driving
  Web/npm and unpublished safe Rust package generation;
- the authored frame-payload corpus across interpreter, native C11 O0/O2,
  Core Wasm/Node, generated npm, and generated Rust consumer lanes, plus the
  read-only Project Agent Transport v5 descriptor/carrier surface;
- private desktop/mobile and host-integration evidence.

The largest remaining gaps are general ownership and lifetime safety, stable
public aggregate/resource/component ABIs, a package and dependency ecosystem,
production application tooling, broad target conformance, and the final 1.0
validation product.

## v0.2 product-exit audit

This audit measures the shipped v0.2.0 objective against the broader product
goal. The annotated tag resolves to
`5f6fb9655fdec92c57ab71615cfd7bfa8cc76051`; all 45 jobs in
[release run 33608662244](https://github.com/wavect/semaprax/actions/runs/33608662244)
passed and the prerelease was published. “Exact-tag hosted” below means only
the gate actually selected by that run. It does not imply an ignored,
unprovisioned, broader-browser, physical-device, registry, or production claim.

| Exit criterion | Evidence | Remaining gate |
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

The v0.2.0 prerelease was successfully produced, but this broader product-exit
audit remains **Partial**: the line-filter still lacks the stated browser
breadth and the Rust builder remains unpublished. The tag run executes the
repository's current nonignored test inventory and explicitly selected release
jobs. It does not by itself complete work-package gates that require separate
provisioning, ignored cases, more browser engines, registry publication, or an
explicit support decision.

Evidence owners: [Project Manifest v1](PROJECT-MANIFEST-V1.md) and its additive
v2–v7 references, [Bounded Language Command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md),
[Project Agent Workflow](PROJECT-AGENT-WORKFLOW-V1.md),
[Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), and
[Native Rust Interoperability](NATIVE-RUST-INTEROP-V1.md).

## WP-01–WP-15 implementation and promotion audit

This table tracks the bounded developer-preview programme separately from the
49-requirement product contract. It distinguishes authored evidence from
selected local execution; exact artifact/source labels belong to the linked
evidence owners. No row below changes a long-term status to Implemented.

| Work package | Current source state | Authored evidence | Remaining gate |
| --- | --- | --- | --- |
| WP-01 CI decomposition | Exact-tag hosted | Dedicated product/platform matrices and the blocking release aggregation all passed in the v0.2.0 tag run | Preserve this closed aggregation and no-mask policy for later release candidates. |
| WP-02 deterministic version | Exact-tag released | Every archive embedded and reported tag version `0.2.0` and exact commit `5f6fb965`; unpacked human/JSON smokes passed on all three build hosts | Repeat exact tag/version/commit binding for each release; agreement is not a signature. |
| WP-03 release artifacts | Exact-tag released | Linux x86-64, Apple Silicon macOS, and Windows x86-64 archives passed host-local package/unpack smoke and were published with exact checksums | Add targets only with their own build-host smoke; no cross-host reproducibility is claimed. |
| WP-04 v0.2 tagged artifact/release promotion | Complete for v0.2.0 | The release gate, three artifact jobs, closed `SHA256SUMS` inventory, and publication job passed at one exact tag; see [release evidence](RELEASE-PROCESS.md#v020-hosted-release-evidence) | A later release requires a new exact-tag record; this completion does not promote unrelated product rows. |
| WP-05 `doctor` | Exact-tag hosted regression coverage; deterministic signed-release packaging and the Unix signed-generation store have focused local evidence; the Linux provisioner and role-specific worker policies remain unexecuted on an admitted Linux host; ordinary production profiles remain unavailable | [Explicit bounded offline-profile selection](DOCTOR-PROBE-V1.md), sealed-input/bundle parsers, injected-host/version checks, and lower-level settlement tests ran where selected by CI. [Linux Production Provisioner v1](DOCTOR-PRODUCTION-PROVISIONER-V1.md) owns exact release-capsule admission, private namespaces, detached read-only tmpfs root, cgroup limits, held static images, deterministic closed-environment archive construction, and whole-cgroup settlement. [Signed Install v1](DOCTOR-SIGNED-INSTALL-V1.md) locally proves held-root signed-byte installation, cooperative activation/rollback and authenticated inert-stage recovery without execution authority. | Connect the exact active generation to the provisioner without reopening paths, execute the unpacked signed Linux distribution with real tools and hostile authority/settlement cases, record an explicit support handoff, and complete equivalent tool/input, filesystem/broker, network and descendant closure on macOS/Windows; local archive, structural, parser and unit evidence does not establish production confinement. |
| WP-06 `new` | Exact-tag hosted, partial | Generator, scaffold replay/CLI preservation, Project checks, and platform publication regressions ran in the tagged repository suite | Execute the explicitly provisioned unpacked-archive onboarding cases on each advertised archive before claiming installed-product breadth. |
| WP-07 quickstart | Exact-tag hosted source workflow; archive onboarding open | Documentation/examples and Project product jobs exercised the checked source workflow | Execute the documented installation and PATH sequence against each candidate distribution before claiming that end-user path. |
| WP-08 v8 specification | Specified | [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) freezes identifiers, admission, lifetime, compatibility, and twelve completion gates | Keep the specification synchronized with the additive implementation and promotion evidence. |
| WP-09 canonical descriptor | Exact-tag hosted regression coverage | Validated-HIR derivation, canonical bytes/digest, independent replay, stable host naming, hostile descriptor cases, and legacy preservation ran in the tagged suite | Retain exact KAT/replay coverage on later profile changes. |
| WP-10 direct `Bytes` npm/Wasm | Exact-tag hosted regression coverage | Profile-specific carrier, copy-out, tuple admission, intrinsic-brand hostility, private-frame exclusion, Node and settlement cases ran where selected | Complete any explicitly provisioned browser-engine cases required by a future public support claim. |
| WP-11 `Option<Bytes>` / `Result<Bytes, i64>` | Exact-tag hosted, partial | Fixed tags, active-payload handling, TypeScript mapping, cleanup, retained evaluation, generated-facade hostility, and inactive-storage cases ran in the tagged inventory | Close any separately provisioned browser/physical equivalence cases required by the intended support claim. |
| WP-12 safe native/Rust SDK | Exact-tag hosted regression and consumer coverage; unpublished | Root-owned provider/SDK, settlement, hostile handles, O0/O2, physical allocation, sanitizer and locked/offline consumer cases ran across the tagged matrices where selected | Make an explicit registry/support decision and execute any still-ignored or separately provisioned cases before that claim. |
| WP-13 Project v8 activation | Exact-tag hosted, partial | Manifest/profile parsing, v1–v7 preservation, routing, retained evaluation, and Windows full-host npm publication regressions ran at the tag | Keep the profile developer-preview until the owning v8 promotion decision explicitly accepts its remaining browser/publication gates. |
| WP-14 frame-payload product | Exact-tag hosted, partial | Shared interpreter/native/Wasm/npm/Rust corpora, display rename, selected consumers, and sanitizer jobs ran in the release matrix | Establish any broader browser-engine and installed-archive consumer breadth before claiming it. |
| WP-15 v8 promotion | Exact-tag hosted release coverage; formal promotion open | The v0.2.0 blocking Project, Rust, browser, sanitizer, host, and hostile suites passed at one exact tag; [Promotion Receipt v1](PROJECT-V8-PROMOTION-RECEIPT-V1.md) now supplies bounded authority-free exact-head observation replay without asserting that any gate ran | Feed independently owned current-head observations into the closed receipt, record an explicit API/package support decision, and close specification-owned provisioned/ignored gaps before calling the generated packages supported or published. |
| Agent Transport v5 follow-on | Exact-tag hosted regression coverage; unpromoted | The focused read-only descriptor/carrier suite and legacy protocol preservation ran in the tagged Rust inventory | Complete cross-language client validation and make an explicit transport promotion decision. |
| Agent Transport v6 public-API follow-on | Focused local executable and generated-codec evidence; unpublished and unpromoted | One opt-in read-only profile returns authenticated Project v8-v11 descriptors and replayed npm carriers with closed profile discriminants; direct equivalence, subject binding, authority exclusion, zero writes, and generated Python/Rust/provisioned-TypeScript codec conformance run locally with cross-profile hostile pairs | Run complete current-tree and hosted gates, package and execute released clients across supported hosts, and make explicit v9-v11 package and transport promotion decisions. |
| Project v9 flat owned record follow-on | Exact-tag hosted regression coverage; unpublished and unpromoted | Descriptor, retained evaluator, Wasm/npm, native provider, safe Rust settlement, Revision Store, and hostile cases ran where selected by the tagged suite | Close separately provisioned physical-consumer gaps and make an explicit v9 publication/promotion decision before dependent profiles. |
| Project v10 owned UTF-8 follow-on | Exact-tag hosted regression coverage; unpublished and unpromoted; blocked on v9 decision | Descriptor/evaluator, cumulative String accounting, Wasm/npm, native owner/provider, safe Rust, UTF-8 hostility and preservation cases ran where selected | First promote v9, then close remaining provisioned/ignored physical and browser gates and make an explicit v10 decision. |
| Project v11 nested owned-record follow-on | Focused local executable evidence; unpublished and unpromoted; blocked on v9/v10 decisions | Separate canonical descriptor and retained evaluator replay passed; generated Core-Wasm/npm ran under Node at the exact cumulative boundary and recovered after +1 rejection; an offline external Rust consumer compiled and ran the generated native package after the same atomic rejection | Run the complete current-tree and hosted gates, add the specification-owned O0/O2/fault/physical/browser corpus, then make explicit prerequisite and v11 promotion decisions. |
| Project Revision Store v1 follow-on | Exact-tag hosted nonignored coverage; unpromoted | Unix and Windows-entry authority, identity, bounded replay, selected-profile round trips and publication regressions ran in the tagged matrices | Complete opt-in provisioned-host/physical cases and the remaining hostile programme before support promotion. |

The Project v8 implementation now has exact-tag hosted regression evidence but
remains **unpromoted**. The generated npm and Rust packages remain
developer-preview, non-registry surfaces. WP-15's remaining explicit
publication/support decision and specification-owned provisioned gaps still
block describing the bounded owned-data API as supported or published. The
authority-free promotion receipt makes those observations replayable but is
not itself hosted evidence or a support decision.

## Long-term product contract

The “Evidence owner” column points to the document that defines the strongest
current bounded slice. It is not a claim that the linked slice completes the
row.

### Semantic foundation

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Agent-native semantic program | Partial; canonical AgentDefinition v1 now assigns stable identities to the six agent types and six harness operations, emits deterministic digest-bound AgentGraph v1, and preserves an admitted Agent Runtime Profile v1 byte-for-byte through the existing kernel, but has no `.spx` syntax, generated proposal grammar, transition execution, typed effects, deployment split, or durable lifecycle. Same-subject selected local Phase 0 v2 evidence includes managed `ACTIVE` and injected result loss after a real local Git ref update; the bounded `function_signature_review_publish_v1` workflow passed its own exact-subject local evidence gate across generated TypeScript/Python/Rust clients and real local SHA-256 Git publication; later focused work adds closed typed SDK failures, per-call authority/blind-spot transcripts, a generated-client contract digest, one installed zero-authority TypeScript workflow package with a passed raw-v5 Unix gate and an authored/unrun pinned-MCP review/publication gate, one session-scoped cancellable candidate-test task exposed through v5/MCP/editor, and authored/unrun generated-file and external-API declaration attachments plus an exact three-declaration analysis-boundary bundle exposed through v5 schemas, generated clients and MCP. The individual routes advance only their owned blind-spot area to partial; the bundle independently replays deployment, generated-file and external-API declarations and advances exactly those three areas to partial without observation or authority. | [RFC 0001](RFC-0001.md), [Language-Native Agent Object v1](LANGUAGE-NATIVE-AGENT-OBJECT-V1.md), [Agent Context v2](AGENT-CONTEXT-V2.md), [Project Agent Transport v5](PROJECT-AGENT-TRANSPORT-V5.md), [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md), [Semantic Workspace Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md), [Candidate Protocol v2](IMAGE-CANDIDATE-PROTOCOL-V2.md), [Typed Holes v1](PROJECT-CANDIDATE-HOLES-V1.md), [Blind-Spot Declarations v1](PROJECT-CANDIDATE-BLIND-SPOT-DECLARATIONS-V1.md), [Phase 0 Evidence v2](GRAPH-OPERATIONAL-PHASE0-EXECUTION-EVIDENCE-V2.md), [Product Workflow v1](IMAGE-SUPPORTED-PRODUCT-WORKFLOW-V1.md), [Application Error Data v1](IMAGE-AGENT-APPLICATION-ERROR-DATA-V1.md), [Response Accountability v1](IMAGE-SUPPORTED-WORKFLOW-RESPONSE-ACCOUNTABILITY-V1.md), [Packaged TypeScript Workflow SDK v1](IMAGE-PACKAGED-TYPESCRIPT-WORKFLOW-SDK-V1.md), [Candidate Test Tasks v1](IMAGE-CANDIDATE-TEST-TASKS-V1.md), [Phase 1 Product Workflow Evidence v1](GRAPH-OPERATIONAL-PHASE1-PRODUCT-WORKFLOW-EXECUTION-EVIDENCE-V1.md) | Execute the authored declaration, bundle, protocol, generated-client and MCP regressions, preserve the MCP package and real Extension Host gates, add language syntax, generated proposal grammar, compiled transitions, typed authority and durable lifecycle, broaden beyond the named scalar workflow, then finish the [graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md), persistent and incremental graph lifecycle, general intentions, target-runtime/provider/conformance evidence, separate raw-source authority, and representative validation. |
| Human-readable program | Partial | [RFC 0001](RFC-0001.md) | Canonical source round-trips every stable language feature with migrations and reviewable diffs. |
| Verified source semantics | Partial | [Architecture](ARCHITECTURE.md) | All admitted language features reach validated HIR only after complete type, effect, contract, and ownership checks. |
| Cross-backend semantic equivalence | Partial; exact-tag nonignored owned-data/String coverage | [Conformance Trace v1](CONFORMANCE-TRACE-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md), [Native Inline String Settlement v1](NATIVE-INLINE-STRING-SETTLEMENT-V1.md), [Native String Contents v1](NATIVE-STRING-CONTENTS-V1.md), [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md), [Standalone Wasm String Settlement v1](WASM-INTERNAL-STRINGS-V1.md#local-validation-record) | Every supported backend passes the same complete behavior, failure, cleanup, and contract corpus; ordinary Wasm String settlement, full native-correction coverage, and separately provisioned opt-in interpreter/standalone-Wasm gates remain open. |
| Atomic agent changes | Partial; the current candidate Git slice authors held executable/cwd launch, exact child ambient state and fail-stop process-group settlement without promoting its unexecuted cross-platform gate | [Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md), [Workspace Change v1](SEMANTIC-WORKSPACE-CHANGE-V1.md), [Candidate Git Publication v1](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md) | General supported single- and multi-file semantic changes replay and publish atomically with recovery and provenance; execute the Git substitution, descriptor, deadline and descendant-settlement matrix and close same-principal repository-content hostility. |

### Language and safety

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [Owned Byte Record Algebra](OWNED-BYTE-RECORD-ALGEBRA-V1.md), [Acyclic Nested Owned-Byte Records](NESTED-OWNED-BYTE-RECORDS-V1.md), [Nested Exact Destructuring](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md), [Nested Immutable Update](NESTED-OWNED-RECORD-UPDATE-V1.md), [Owned Byte Variant Algebra](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) | Execute and promote the bounded nested record/destructuring/update corpus, then verify general nested/generic/resource aggregates, variants, matching, layout, cleanup, and public ABIs. |
| Functions, closures, interfaces, implementations, generics | Partial | [RFC 0001](RFC-0001.md) | Closures, interfaces/implementations, inference, constraints, specialization, and cross-target execution are complete. |
| `Option` and `Result`; no null or unchecked exceptions | Partial | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [Owned Byte Variant Algebra](OWNED-BYTE-VARIANT-ALGEBRA-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) | General Copy and owned propagation/matching, residual conversion, ABI, and target behavior are verified. |
| Immutable-by-default values and explicit mutation | Partial | [Explicit Mutation v1](EXPLICIT-MUTATION-V1.md), [Field Mutation v1](FIELD-MUTATION-V1.md), [Nested Immutable Update](NESTED-OWNED-RECORD-UPDATE-V1.md) | Execute and promote bounded immutable nested reconstruction, then verify general aggregate, collection, borrowed, and concurrency-aware mutation rules. |
| Unique ownership and move safety | Partial | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md), [Owned Byte Variant Algebra](OWNED-BYTE-VARIANT-ALGEBRA-V1.md), [Acyclic Nested Owned-Byte Records](NESTED-OWNED-BYTE-RECORDS-V1.md), [Nested Exact Destructuring](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md), [Nested Immutable Update](NESTED-OWNED-RECORD-UPDATE-V1.md), [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md), [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) | Execute and promote the bounded nested record, destructuring, update, and loan corpus, then verify general owned values, aliases, variants, control flow, FFI, cleanup, and target execution. |
| Borrowed views and lifetime safety | Partial | [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md), [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md), [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md), [Acyclic Nested Owned-Byte Records](NESTED-OWNED-BYTE-RECORDS-V1.md), [Nested Exact Destructuring](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md), [Projected Owned-Bytes Synchronous Borrowed Call v1](PROJECTED-OWNED-BYTES-BORROWED-CALL-V1.md) | Execute and promote bounded nested field-path loans and borrowed destructuring, then complete general lifetime inference, mutable and escaping borrows, cross-file use, and public host ABI behavior. |
| Regions and arenas | Partial | [Region Report v1](REGION-REPORT-V1.md) | Region inference and runtime placement are implemented and verified; the report alone is insufficient. |
| Shared immutable ARC and managed zones | Partial | [ARC Zone Model v1](ARC-ZONES-V1.md) | Language, runtime, cycle, escape, and concurrency semantics execute on supported targets. |
| Restricted `unsafe` and raw memory | Partial | [Unsafe Boundaries v1](UNSAFE-BOUNDARIES-V1.md) | Raw memory operations, review policy, capability rules, and target conformance are implemented and verified. |
| Checked, wrapping, and saturating arithmetic | Partial; exact-tag nonignored correction coverage | [RFC 0001](RFC-0001.md), [checked `usize` multiplication correction](PORTABLE-INDEXED-BYTE-DATA-V1.md#checked-multiplication-correction) | Retain the tagged ordinary/aggregate Wasm zero-multiplier and owned-cleanup regressions and complete any separately provisioned gates; all numeric widths and named arithmetic modes still require complete cross-backend semantics and tests. |
| Effects and capabilities | Partial | [Capability Manifest v1](CAPABILITY-MANIFEST-V1.md) | Declared effects and build/runtime capabilities are enforced end to end, including dependencies and hosts. |
| Contracts and progressive verification | Partial | [RFC 0001](RFC-0001.md) | Static discharge, bounded proof, runtime obligations, counterexamples, and repair evidence are integrated. |
| Structured concurrency | Partial | [Scoped Task Model v1](SCOPED-TASKS-V1.md) | Language syntax, checking, runtime scheduling, cancellation, cleanup, and target execution are verified. |
| Typed hygienic generation | Partial | [Hygienic Generation v1](HYGIENIC-GEN-V1.md) | General typed synthesis is scoped, hygienic, deterministic, and integrated with multi-file semantics and review. |

### Compiler and output targets

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Fast development lane | Partial; exact-tag nonignored interpreter and prepared Project trace coverage | [Interpreter v1](INTERPRETER-V1.md), [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md), [Prepared Project Interpreter and Source Trace v1](PROJECT-PREPARED-INTERPRETER-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) | Complete separately provisioned cross-platform String admission/replay/conformance gates; then incremental refresh, debugging, hot reload, and semantic equivalence must meet the development-performance target. |
| Optimizing native lane | Partial | [Architecture](ARCHITECTURE.md) | The production native backend covers the mature language, optimization, debug mapping, and supported hosts. |
| WebAssembly core and components | Partial; exact-tag nonignored core/component/String coverage | [Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), [Wasm Owned ABI](WASM-OWNED-ABI-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md), [Standalone Wasm String Settlement v1](WASM-INTERNAL-STRINGS-V1.md#local-validation-record) | Complete separately provisioned standalone String gates; stable Components, resources, capabilities, multi-engine conformance, and packaging must still be verified. |
| Embedded and real-time | Partial | [Freestanding Profile v1](FREESTANDING-V1.md) | Hardware profiles, linker control, interrupts/RTOS, timing constraints, and representative targets are verified. |
| SIMD and GPU | Partial | [SIMD Report v1](SIMD-REPORT-V1.md) | Vector/GPU lowering, legality, memory behavior, target selection, and performance evidence are implemented. |

### Ecosystem interoperability

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Partial; exact-tag nonignored semantic-lock, dependency-selection, capsule, and build-v2 coverage; one extensible `semaprax.manifest.v1` table manifest with dependency and target-matrix grammars lowers onto every frozen project profile, `semaprax lock` renders, verifies, and compatibility-compares a deterministic project `semaprax.lock` (coarse lock-level and fine-grained per-export scalar interface), and `semaprax resolve` selects manifest `[dependencies]` against a local content-addressed cache through the offline resolver and pins the per-target resolution, each with a local executable gate (`tests/project.rs::package_manifest_v1`, `::project_lock_v1`, `::dependency_resolution_v1`); unpromoted | [Package Manifest v1](PACKAGE-MANIFEST-V1.md), [Project Lock v1](PROJECT-LOCK-V1.md), [Project Dependency Resolution v1](PROJECT-DEPENDENCY-RESOLUTION-V1.md), [Package Report v1](PACKAGE-REPORT-V1.md), [Semantic Package Report v2](PACKAGE-REPORT-V2.md), [Offline Package Lock v1](OFFLINE-PACKAGE-LOCK-V1.md), [Offline Semantic Lock v2](OFFLINE-SEMANTIC-PACKAGE-LOCK-V2.md), [Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md), [Compatibility Evidence v1](OFFLINE-PACKAGE-COMPATIBILITY-EVIDENCE-V1.md), [Offline Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md), [Offline Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md), [Published Semantic Lock Snapshot v1](OFFLINE-PUBLISHED-SEMANTIC-LOCK-SNAPSHOT-V1.md), [Multi-Package Source Capsule v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md), [Effect-Free Wasm Package Build v1](OFFLINE-PURE-WASM-PACKAGE-BUILD-V1.md), [Linked Scalar Wasm Package Build v2](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md) | Complete separately provisioned publication/consumer gates; then add general compatibility negotiation, supported publication, trusted provenance, registry, and conformance. |
| Portable canonical ABI and native fast ABI | Partial | [ABI Report v1](ABI-REPORT-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) | Stable aggregate/resource/borrowed ABIs and cross-language conformance cover supported architectures. |
| C and Objective-C | Partial | [C Header v1](C-HEADER-V1.md) | Import/export, ownership, errors, compiled consumers, Objective-C adapters, and compatibility are verified. |
| C++ | Partial; local compiled scalar and Project-v8 owned-data package evidence | [C++ Shim v1](CXX-SHIM-V1.md), [C++ scalar package v1](CXX-PACKAGE-V1.md), [Project v8 C++ owned-data package v1](PUBLIC-CXX-OWNED-DATA-PACKAGE-V1.md) | Cross-platform/MSVC compiled consumers, broader aggregates/resources and borrowed lifetimes, maintained distribution, compatibility, packaging, and supported-host conformance are verified. |
| Java and Kotlin | Partial | [Android JNI Ownership v1](ANDROID-JNI-OWNERSHIP-V1.md) | Public JVM/JNI artifacts, ownership, exceptions, packaging, and conformance are verified. |
| Swift and Apple frameworks | Partial | [Swift Ownership v1](APPLE-SWIFT-OWNERSHIP-V1.md) | Public Swift/Objective-C API, distributable frameworks, lifecycle, ownership, and device evidence are verified. |
| JavaScript and TypeScript | Partial; exact-tag nonignored Node/TypeScript/Chromium coverage | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md), [Standalone Wasm String Settlement v1](WASM-INTERNAL-STRINGS-V1.md), [Standalone String Web package](WASM-INTERNAL-STRINGS-WEB-V1.md#local-validation-record) | Complete separately provisioned host and multi-engine browser/runtime breadth; stable general bindings, owned resources, async/callbacks and packaging must still be verified. |
| WIT and WebAssembly Components | Partial | [Public Scalar WIT Interface v1](PUBLIC-SCALAR-WIT-INTERFACE-V1.md), [private WIT Boundary v1](WIT-COMPONENT-BOUNDARY-V1.md) | Extend the retained Project-v1 scalar interface artifact into supported Component publication; source-selected interfaces and resources run through a supported Component Model toolchain on multiple runtimes. |
| OpenAPI, Protobuf/gRPC, GraphQL, and SQL | Partial | [OpenAPI v1](OPENAPI-V1.md) | Import/export, compatibility, live conformance, and all named schema families are verified. |
| Standard library | Partial; ten packages (eight `core`, one `portable`, one `test`) with local interpreter, native C11, and Core Wasm conformance; bundled `std.*` manifest dependencies, cross-file borrowed-text linking, and both hardened built-in templates have focused local evidence | [Standard Library v1](STANDARD-LIBRARY-V1.md), [catalog](STANDARD-LIBRARY-CATALOG.md) | Every required module exists at its tier with identities, contracts, effects, examples, conformance on every listed target, and generated documentation; ordinary resolved-package builds, the Everyday profile, and the remaining offline templates ship. |

### Application platforms

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| First-class application/state/UI dialect | Partial | [UI Schema v1](UI-SCHEMA-V1.md) | Typed state/update/view, semantic controls, accessibility, navigation, assets, and platform escape hatches execute. |
| Web | Partial | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md) | Accessible DOM/CSS, SSR/hydration, packaging, multi-engine execution, and a deployable sample are verified. |
| iOS | Partial | [Swift Ownership v1](APPLE-SWIFT-OWNERSHIP-V1.md) | Public framework/app generation, lifecycle, accessibility, signing metadata, and device/simulator samples are verified. |
| Android | Partial | [Android JNI Ownership v1](ANDROID-JNI-OWNERSHIP-V1.md) | Public AAR/app generation, lifecycle, accessibility, packaging, and emulator/device samples are verified. |
| macOS | Partial | [Desktop App v1](DESKTOP-NATIVE-APP-V1.md) | Public host/UI generation, lifecycle, accessibility, packaging, signing/notarization, and a sample are verified. |
| Windows | Partial | [Desktop UI v1](DESKTOP-NATIVE-UI-V1.md) | Public host/UI generation, lifecycle, accessibility, MSIX/signing metadata, and a sample are verified. |
| Linux | Partial | [Roadmap](ROADMAP.md) | A supported UI/runtime adapter, accessibility, distribution formats, and a representative application are verified. |
| Edge and server | Partial | [Bounded Language Command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md) | General I/O, async services, HTTP/data adapters, observability, deployment, and load/conformance tests are verified. |
| Plugins | Partial | [Plugin Manifest v1](PLUGIN-MANIFEST-V1.md) | Capability-limited loading, lifecycle, compatibility, resource limits, packaging, and hostile-plugin tests are verified. |

### Agent economics, review, and operations

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial | [Agent Context v2](AGENT-CONTEXT-V2.md), [Economics v1](AGENT-ECONOMICS-V1.md), [Semantic Workspace Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md) (exact-tag nonignored regression coverage) | Exact model-token budgets, broader semantic edges, persistent indexing, and representative measured savings are verified. |
| Impact analysis before modification | Partial | [Semantic Impact v1](SEMANTIC-IMPACT-V1.md), [Semantic Workspace Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md) (exact-tag nonignored regression coverage) | Repository-wide call/type/contract/test/schema/target/capability consumers are complete and incremental. |
| Typed holes and compiler-generated repairs | Partial | [Diagnostic Repair v1](DIAGNOSTIC-REPAIR-V1.md) | General obligations and composable sound repairs are generated, ranked, reviewed, and replay-verified. |
| Proof-carrying patches | Partial | [Patch Evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md) | General semantic claims, tests, targets, capability deltas, provenance, and compatibility are independently verified before commit. |
| Semantic human review | Partial | [Semantic Review v1](SEMANTIC-REVIEW-V1.md) | Complete repository-wide behavioral, API, security, memory, target, migration, and unsafe summaries are evidence-backed. |
| Sandboxed builds and dependencies | Partial; authority-free linked build has exact-tag nonignored coverage and remains unpromoted | [Capability Manifest v1](CAPABILITY-MANIFEST-V1.md), [Offline Package Lock v1](OFFLINE-PACKAGE-LOCK-V1.md), [Offline Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md), [Multi-Package Source Capsule v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md), [Effect-Free Wasm Package Build v1](OFFLINE-PURE-WASM-PACKAGE-BUILD-V1.md), [Linked Scalar Wasm Package Build v2](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md) | Complete separately provisioned publisher evidence, then verify reproducible acquired inputs and actual least-authority OS sandbox/dependency enforcement. Empty source authority and no external tool execution are not a hermetic sandbox. |
| Debugger, profiler, diagnostics, and operations | Partial | [Architecture](ARCHITECTURE.md), [Human Diagnostic Locations v1](HUMAN-DIAGNOSTICS-V1.md) | Source-level debugging/profiling, crash and trace mapping, observability, and deployment diagnostics cover every backend. |

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
this final gate.
