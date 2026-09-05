# Quality gates

Status: living internal contributor documentation.

Audience: contributors, maintainers, and release reviewers.

This document defines repository-wide verification policy and routes changes to
their owning evidence. Exact protocol mutation matrices, known-answer digests,
platform fixtures, and focused command lists belong in the relevant versioned
specification and tests; they are not repeated here.

## The rule

A change is ready only when:

1. its baseline quality profile passes;
2. every affected versioned contract passes its focused evidence;
3. preservation tests for older schemas and unaffected behavior pass;
4. any public or hosted claim has evidence from the exact commit being claimed.

A local green test can support a local claim. It cannot be promoted to hosted,
public, cross-platform, or production evidence without the corresponding gate.

Dependency changes additionally run the complete
`tests/project.rs::package_manifest_v1` module and the Native Rust builder's
library and `project_sdk_cli` tests. Effectful Rust-crate coverage requires the
explicit tool environment documented by
[Project Dependencies v1](PROJECT-DEPENDENCIES-V1.md); a skipped tool-dependent
case is not promotion evidence. The arbitrary-crate gate must generate the
Project SDK, lock and build its consumer offline, invoke a declared external
crate from `NativeRustSdkImports`, cross the `import rust fn` boundary, and
observe the resulting value through a SEMAPRAX export.

An exact-byte native fixture that replaces the compiler's entry wrapper must
establish its own stdout transport mode. String fixtures use the shared
test-only binary-stdout setup before allocator instrumentation and check setup
success before semantic execution; retain exact transcript/status bytes rather
than normalizing away an unintended Windows CRT newline conversion. That setup
is fixture plumbing, not evidence that the generated runtime or any target gate
has executed.

## Standard entry point

Use the routed script on Unix:

```sh
scripts/quality.sh full
```

It accepts `quick`, `changed`, or `full`. The script first emits and validates a
deterministic `semaprax.quality-route.v2` plan, then dispatches only the exact
listed gates. `changed` may widen to `full` when the path classification is not
safe enough for a narrower run. Two path classes stay narrow and append their
own gate after the fixed `changed` list: CLI surface paths (`src/cli/`,
`src/cli_driver/`, `src/bin/`, `src/cli_driver.rs`, `src/main.rs`) add
`test-cli`, which runs the CLI harnesses of both the standalone package and the
full toolchain; editor
paths (`editors/`) add `test-editor`, which runs the extension's `node --test`
suite and the documentation harness. Any other unmapped path still widens the
whole run to `full`, and `full`'s gate list does not vary.

Preview the validated route without running any gates when choosing a local
feedback loop or diagnosing why `changed` widened:

```sh
scripts/quality.sh changed --plan
```

Run `scripts/quality.sh --help` for the profile and option summary. During
execution the script writes each gate name to standard error before starting
it, so long-running checks remain attributable without changing the canonical
plan on standard output.

| Profile | Intended use | Gates |
| --- | --- | --- |
| `quick` | Early local feedback | diff check, Rust formatting, workspace check, advisory documentation/examples/context tests |
| `changed` | Bounded reviewed changes | `quick` plus package Clippy, agent-context integration, and package rustdoc; plus `test-cli` for CLI surface paths and `test-editor` for editor paths |
| `full` | Semantic changes and release candidates | workspace Clippy/tests/doctests/rustdoc, release build, package check, and canonical example checks |

Capability-aware command help additionally requires the exact catalog/dispatcher
inventory, global-byte preservation, standalone/full capability separation,
scoped and malformed-position behavior, and zero-activity gates owned by
[Capability-Aware CLI Help v1](CLI-HELP-V1.md). Typo guidance additionally
requires bounded unique matching, exact diagnostics, and standalone/full
capability separation owned by [CLI Help v2](CLI-HELP-V2.md).
Known-command recovery additionally requires exact status-2 hints, capability
separation, and preservation of unknown and malformed-help diagnostics owned by
[CLI Help v3](CLI-HELP-V3.md). The guided global page additionally requires
its 2048-byte bound, fixed groups, capability filtering, and the exhaustive
`help all` catalog owned by [CLI Help v4](CLI-HELP-V4.md).

Human diagnostic rendering requires exact path/span combinations,
control-character escaping, unchanged JSON, and a physical compiler failure as
owned by [Human Diagnostic Locations v1](HUMAN-DIAGNOSTICS-V1.md).

The script is the executable source of truth for the precise command sequence.
Do not copy that sequence into feature documents.

The general Windows CI job disables dev/test debug-symbol files and incremental
artifacts to reduce cold-build I/O. It retains debug assertions, all existing
tests, physical host gates, and release-profile settings; this is a build-cost
change, not a reduction in coverage.

The current-toolchain Rust lane uses the same closed four-way Cargo target
inventory on Linux, macOS, and Windows: one lib/bin shard and three integration
target shards run in parallel, while formatting, strict Clippy, documentation,
release builds, examples, sanitizers, and physical platform gates remain in a
separate blocking job for each host. Windows retains its existing exclusion of
the separately owned native-Rust-interop package; the router validates that
exclusion against Cargo metadata instead of accepting a free-form omitted
target. Unknown target kinds or package exclusions fail closed. The release
gate requires both matrices.

The Rust 1.88 minimum-version lane partitions the complete Cargo workspace
target inventory into a lib/bin shard and three integration-target shards using
`scripts/ci-msrv.py`. Every shard retains workspace-wide feature unification,
locked dependencies, the all-targets/all-features check, and the 20-minute job
limit. Matrix fail-fast is disabled so every shard reports its result after a
peer failure. Shared integration target names stay together;
unknown target kinds fail closed instead of silently losing coverage. The
release gate requires the complete matrix. This changes scheduling only, not
the local `full` profile or any test, admission limit, or release requirement.

## Manual baseline

On a host that cannot run the POSIX script, reproduce the `full` profile:

```sh
git diff --check
cargo fmt --all --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo build --locked --workspace --release
cargo package --locked --allow-dirty -p semaprax
```

Also run the example check and canonical-format loops from
`scripts/quality.sh`; keeping the list there prevents drift.

## Documentation changes

Documentation-only changes must pass at least:

```sh
git diff --check
cargo test --locked -p semaprax --test documentation --test examples
```

`tests/documentation.rs` checks local Markdown links recursively, that every
tour code block is a verbatim example excerpt, and that every SEMAPRAX block in
the [agent quick reference](AGENT-QUICK-REFERENCE.md) either verifies cleanly in
canonical form or produces exactly the diagnostic code its marker names. The
docs workflow builds the mdBook using the pinned version in
`.github/workflows/docs.yml`.

If documentation changes a technical claim, run the evidence that owns that
claim. Editing prose does not substitute for implementation evidence.

## Change-specific evidence

Select every row touched by the change; these categories are cumulative.

| Change | Minimum additional evidence |
| --- | --- |
| Lexer, parser, or formatter | Success and diagnostic cases, canonical round-trip, unchanged legacy formatting |
| Verifier or HIR | Focused verifier tests, hostile-HIR rejection where applicable, deterministic identity checks |
| Runtime semantics | Interpreter/native O0/native O2/Wasm agreement for success, failure, evaluation order, and re-entry |
| Agent/payment harness | Exact AgentDefinition, AgentGraph, Economic Policy and payment-graph replay; model-output rejection; disjoint-host authority inventory; completed-run handoff; Economic Agent all-rail, x402, restart, cancellation and hostile-document evidence |
| Opt-in internal String interpreter | Distinct schema/domain and cross-profile rejection, frozen ordinary/Project/prepared/effectful admission, unchanged external String rejection, source and envelope bounds, canonical/duplicate/re-signed hostile-wire rejection, exact output capacity, String call/contract/failure value parity, fuel/depth boundaries, CLI behavior, and unchanged legacy golden/fuel facts; no heap-memory or Wasm settlement inference |
| Standalone Wasm internal String settlement | Distinct explicit profile, structural module validation, fixed memory and selected acyclic stack/owner bounds; independent raw mint/drop accounting and every reached mint-refusal path, generated-host exact/+1 quotas and poison/reentry, exact artifact/input binding, native O0/O2 and internal-String interpreter parity, legal scalar-loop helper reuse, unchanged U105/T252/J113 rejection and legacy artifact known answers; [local validation record](WASM-INTERNAL-STRINGS-V1.md#local-validation-record) owns bounded partial evidence, with cross-platform, full-profile, and hosted gates remaining; no support promotion or ordinary-Wasm, peak-heap or trap-recovery inference |
| Standalone internal String Web package | Actual explicit-source CLI selection and pre-effect usage rejection; bounded source snapshot and final drift recheck; source/descriptor/package exact/+1 bounds; exact eight-file inventory, independent manifest/digest replay and direct compiler-output equality; deterministic repeat and stable-ID rename, hostile identities, fresh-parent publication and foreign-byte preservation; real generated Node, strict provisioned TypeScript and provisioned browser consumers including streamed fetch rejection before EOF; pre-effect legacy String rejection including materialized generic bodies, unchanged raw emission and String-free legacy bytes; the [package validation record](WASM-INTERNAL-STRINGS-WEB-V1.md#local-validation-record) separates selected local consumers and real source/descriptor boundaries from private renderer accounting and unrun required-host/release gates; no support promotion |
| Prepared Project interpreter or source trace | One cached exact closure admission and one persistent worker across repeated entry/test execution; legacy outcome/fuel parity; cancellation boundaries; exact node/byte/event limits; deterministic truncation; canonical replay; retained-HIR source-origin binding; worker panic/disconnect fail-stop; and unchanged Interpreter/Project/Transport v1-v5 bytes |
| Prepared Project revision replacement | Exact expected-content revision before candidate preparation, both closures/origins swapped together, byte-identical old execution after stale or ordinary candidate rejection, new/old trace cross-binding, same worker and permit, unchanged ceilings/cancellation/admission, concurrent-operation rejection, and terminal panic or lost acknowledgement; no epoch, incremental-compiler, or peak-heap inference |
| Ownership or cleanup | Structural inventory, canonical plan build, independent replay, hostile mutation, success/failure settlement |
| Nested owned records and loans | Exact/+1 depth, leaf and work bounds; stable full field-ID paths; partial-construction and trailing-Copy failure; atomic whole moves/call commits; path-overlap loans; interpreter/native O0/O2/Wasm trace parity; v2-v6/v1-v25 byte preservation; [Nested Records v1](NESTED-OWNED-BYTE-RECORDS-V1.md) owns the complete gate |
| Nested owned-record destructuring | Exact recursive pattern identities/inventory; every owned descendant bound once or rejected; Copy-only wildcards/results; atomic whole-source commit; borrowed-arm alias lifetime and sibling independence; hostile HIR/plan mutations; interpreter/native O0/O2/Wasm trace parity; v2-v7/v1-v27 byte preservation; [Nested Destructuring v1](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md) owns the complete gate |
| Nested owned-record immutable update | Exact base/record/field identities; top-level replacement inventory and left-to-right completion; unchanged transfer, replaced-old settlement and reverse completed-prefix cleanup; atomic result commit; active-loan rejection; hostile HIR/plan mutations; interpreter/native O0/O2/Wasm trace parity; v2-v8/v1-v29 and flat-update preservation; [Nested Immutable Update v1](NESTED-OWNED-RECORD-UPDATE-V1.md) owns the complete gate |
| Project Agent Transport v6 | Exact startup admission for only Project v8-v11 and rejection of earlier/future profiles; byte-frozen v2-v5 protocol behavior; direct retained descriptor/carrier equality for all four profiles; full carrier replay plus profile-specific typed descriptor and subject binding; exact closed result keys and complete response-wrapper boundary; zero/oversized/one-byte-short rejection with recovery; stale/surplus/notification rejection and zero Project-tree writes; generated Python, in-harness Rust, and provisioned Node/TypeScript codecs against retained v8-v11 sessions; complete cross-profile/schema/digest/build hostile pairs; closed direct-child environments and bounded settlement; [Project Agent Transport v6](PROJECT-AGENT-TRANSPORT-V6.md) and its [SDK](PROJECT-AGENT-TRANSPORT-V6-SDK-V1.md) own the local gate. Hosted, packaged-client, registry and cross-platform evidence remain required for promotion. |
| C++ scalar package v1 | Exact caller-authorized source and stable-ID replay through parser, verifier, admission and native generation; byte-identical reconstructed Shim/header/provider; valid-subject substitution, digest-reminted source/revision/selection/artifact and appended-code rejection; final-envelope exact/+1 bounds plus independent hard source/intermediate bounds; separate C11 provider and C++17 consumer compilation/link/execution; success plus precondition/postcondition/arithmetic/invalid-char/null-output failure-slot preservation; frozen C++ Shim v1 bytes. [C++ scalar package v1](CXX-PACKAGE-V1.md) owns the local gate; cross-platform ABI, owned values, exceptions, packaging and support promotion remain separate. |
| Project v8 C/C++ owned-data package v1 | Exact held-Project v8 descriptor/provider regeneration and package replay; bounded canonical C/C++ artifacts; separate provider plus C11 and C++17 consumer compilation, linking and execution at O0/O2; the pure C lane covers invalid-bool poison, owned-byte length/copy/drop, stale handles and context closure; exact/+1 cumulative input and output limits; poison preservation and recovery; wrong-context/stale/duplicate handle rejection; copy/drop/context-close settlement and fail-stop uncertainty. [Project v8 C/C++ owned-data package](PUBLIC-CXX-OWNED-DATA-PACKAGE-V1.md) owns the local gate; target activation, MSVC/cross-platform ABI, maintained distribution, compatibility and support promotion remain separate. |
| Project v9 C11 flat owned-record integration | Header bytes derive only from the replayed descriptor; closed status/type-kind vocabulary; exact export, field-count and descriptor-order ordinal mapping; `uint64_t[static N]` carrier rather than native record layout; invalid-input poison; copied scalar validation; owned-handle length/copy/single-drop and stale-drop rejection; context closure; separate actual-provider/consumer C11 compilation, link and execution at O0/O2. [Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md) owns the local gate; safe application wrapping, packaging, cross-platform support, compatibility and v9 promotion remain separate. |
| Project v10 C11 owned-UTF8 integration | Shared header bytes derive only from the replayed descriptor; fixed-width status/tag/handle boundary; no native String layout or sentinel scan; exact length, embedded NUL and multibyte UTF-8 preservation; copy, single drop and context closure; separate actual-provider/consumer C11 compilation, link and execution at O0/O2. [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) owns the local gate; malformed-provider fault injection, safe C wrapping, packaging, cross-platform support, compatibility and v10 promotion remain separate. |
| Project v11 C11 nested owned-record integration | Header bytes derive only from the replayed descriptor; closed status/leaf-kind vocabulary; exact export, leaf-count, path-order ordinal and kind mapping; `uint64_t[static N]` carrier rather than native record layout; two distinct owned handles copied and settled exactly once; duplicate-drop rejection and post-settlement context closure; separate actual-provider/consumer C11 compilation, link and execution at O0/O2. [Public Nested Owned-Record API v1](PUBLIC-NESTED-OWNED-RECORD-API-V1.md) owns the local gate; fault injection, safe application wrapping, packaging, cross-platform support, compatibility and v11 promotion remain separate. |
| Graph schema | Exact new projection, legacy byte preservation, context projection, invalid/tampered rejection |
| Semantic patch or repair | Preview, stale/drift rejection, no-write failures, independent replay, atomic A0 application |
| Project candidates and typed intentions | Exact canonical change admission, complete caller migration/evaluation order, AST/source round-trip and complete Project replay, unchanged explicit identities/effects/contracts, core target admission preservation, stale/tampered evidence rejection and no writes; [Candidate v1](PROJECT-CANDIDATES-V1.md) owns the focused unrun evidence |
| Candidate holes and ordered signature mapping | Unresolved drafts expose no source/evidence, failed fills preserve every sibling, exact stale/duplicate/capacity rejection, actual scope/contracts/effect budget, original argument evaluation order including removed arguments, hygienic bindings, and ownership-mode rejection; [Holes v1](PROJECT-CANDIDATE-HOLES-V1.md) and [Signature Evolution v1](PROJECT-SIGNATURE-EVOLUTION-V1.md) own authored/unrun cases |
| Unified workspace protocol v5 | Exact startup capability matrix and rejection of RPC elevation; frozen v1–v4 bytes; expected-old/new cold refresh, failure immutability, preserved candidate handles and cleared drafts/attempts; bounded output before state mutation; [Workspace Protocol v5](IMAGE-WORKSPACE-PROTOCOL-V5.md) and [Session CLI](WORKSPACE-SESSION-CLI-V1.md) own authored/unrun cases |
| Source-backed candidate archives and startup recovery | Exact canonical original-source rebuilding plus unchanged capsule replay; stale/rehash/compatibility/authority-claim rejection; recovery after raw source removal; private normalized root and held ancestor identities; single-link files, bounded inventory/read, no-replace pivot and retained failed stages; post-pivot uncertainty; startup-only same-manifest candidate handoff with live pre/post authentication and no approval restoration; v1/v2 policy and full historical CLI help preservation. [Archive](PROJECT-CANDIDATE-ARCHIVE-V1.md), [Store](CANDIDATE-ARCHIVE-STORE-V1.md), [Recovery](IMAGE-WORKSPACE-ARCHIVE-RECOVERY-V1.md), and [CLI](CANDIDATE-ARCHIVE-CLI-V1.md) own authored/unrun evidence |
| Automatic candidate/draft retention lifecycle | Publish the immutable typed archive before registry mutation; retain distinct held archive/registry roots; exact checkpoint/plan and expected-current pivot; preserve archive success on stale or uncertain registry outcomes; replay an existing archive into a typed receipt without adoption; resume once or recognize an exact already-retained subject; reject cross-kind/reminted selectors; restart without the original checkout; canonical authority-free replay/resume reports. [Automatic lifecycle v1](AUTOMATIC-CANDIDATE-DRAFT-LIFECYCLE-V1.md) owns the focused local gate. |
| Live frontend reuse and parallel image reads | Exact-source parser/formatter reuse without a preliminary cold build; unchanged cold/image bytes; source path/identity authentication despite cache hits; staged preview/failed-refresh rollback; strict host-policy v1/v2 separation; bounded worker overlap, ordered sequential-byte equivalence, no mutable/execution/publication dispatch, all-worker join on failure and whole-batch stale rejection. [Live Frontend Cache](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md) and [Parallel Reads](IMAGE-PARALLEL-READS-V1.md) own authored/unrun cases |
| V5 source-commit extension | Independent startup-only approval, exact restored candidate/base selection, no request-selected Git policy, approval consumption, one-shot success and G267 uncertainty, bounded retained receipt/chunks and historical status after drift; generic transport checks must not mask a pivot outcome; [Source Commit v5](IMAGE-SOURCE-COMMIT-PROTOCOL-V5.md) owns authored/unrun cases |
| Integrated v5 signature-to-Git workflow | Execute both real SHA1/SHA256 provider scenarios, exact migrated-call/source/semantic-delta checks, preserved contract/effect/export/parameter facts, explicit-policy tests, all four target projection rows, separate review/restore/approval sessions, committed source/parent/unrelated-mode preservation, wrong approval and actual stale-ref preflight. [Git Workflow v1](PROJECT-GRAPH-OPERATIONAL-GIT-WORKFLOW-V1.md) owns authored/unrun cases; general ownership, native/Wasm execution and physical CAS-race evidence remain separate |
| Supported v5 product workflow | One clean exact local subject; exact Cargo inputs and selected tool identities; frozen review/publish capability profiles and host test policy; independently executed generated Python, Rust, and provisioned-TypeScript codecs; three isolated local Unix bare SHA-256 publications with handoff, approval, receipt, committed-object, and raw-source bindings; stale/drift/test/recovery/approval/pre-pivot/real-post-CAS-loss/malformed-response hostility; canonical archived replay. [Product Workflow v1](IMAGE-SUPPORTED-PRODUCT-WORKFLOW-V1.md) and [Phase 1 Product Workflow Evidence v1](GRAPH-OPERATIONAL-PHASE1-PRODUCT-WORKFLOW-EXECUTION-EVIDENCE-V1.md) own the gate. Local passage does not establish MCP/editor transport, hosted/cross-platform, target runtime, network isolation, full quality, or programme completion. |
| V5 workflow response accountability | Preserve the existing JSON-RPC code/message and generic grammar failures; emit complete closed application diagnostic data or the existing overflow response; reject malformed/foreign diagnostic data in generated TypeScript, Python, and Rust clients; bind every selected workflow step to its exact method, payload, grant, effect, authority flags, blind-spot ledger and only permitted runtime update; bind the exact selected-profile digest; reject phase/index/method mismatches. [Application Error Data v1](IMAGE-AGENT-APPLICATION-ERROR-DATA-V1.md) and [Response Accountability v1](IMAGE-SUPPORTED-WORKFLOW-RESPONSE-ACCOUNTABILITY-V1.md) own the gate. This does not qualify a packaged SDK, automatic orchestration/repair, cancellation, or later-head workflow execution. |
| Candidate test task control | Preserve byte-identical synchronous/cancellable success reports; prove immediate pre-step cancellation emits no report; admit one exact image/project/candidate-bound task per test-enabled v5 session; pin queued/running/sticky terminal states, duplicate/stale handle rejection, bounded result paging, all-false authority and blind spots; cancel and join on drift, refresh, finish and drop; execute direct v5 and ordinary MCP tool paths; validate editor cancellation and late-result invalidation. [Candidate Test Tasks v1](IMAGE-CANDIDATE-TEST-TASKS-V1.md) owns the gate. Broad quality, MCP Tasks conformance, real Extension Host task execution, hosted/cross-platform and timing evidence remain separate. |
| Packaged TypeScript workflow SDK | Build the production `@semaprax/agent-workflow` artifact; strict-compile generated review/publish codecs and its consumer; offline pack and local-tarball install with disabled scripts and closed inventory, integrity and lockfile; resolve and execute the installed package by name without a compiler; drive the exact thirteen-step review and separately approved nine-step publication through distinct real v5 stdio sessions; authenticate handoff, unchanged source, receipt and local SHA-256 Git objects; reject malformed/structured failure and duplicate publication. [Packaged TypeScript Workflow SDK v1](IMAGE-PACKAGED-TYPESCRIPT-WORKFLOW-SDK-V1.md) owns the explicitly provisioned ignored gate. Offline npm is not OS network isolation, and a local Unix package run is not registry, hosted, cross-platform, release or programme promotion. |
| V5 targets, artifacts and typed discovery | Actual closure target-emission facts without false per-symbol blame; independent Web/npm carrier replay and source/export/file bindings; candidate replay before pathless builds; zero artifact filesystem writes; runtime-granted catalog/schema/client alignment and explicit opaque-response gaps; executed cross-language validation remains required; [Target/Artifact Projections](IMAGE-TARGET-ARTIFACTS-V1.md) and [Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md) own authored/unrun cases |
| Candidate-only image protocol | Explicit host selection, preserved read-only v1, schema/catalog alignment, exact handle selection, bounded registries and response-before-mutation, source drift invalidation, independent candidate replay, no source/test/build authority, and hole lifecycle; [Candidate Protocol v2](IMAGE-CANDIDATE-PROTOCOL-V2.md) owns authored/unrun cases |
| Candidate expression/contract changes and rebase | Actual HIR/source expression identity and lexical scope, exact expected type/ownership after source replay, additive contract inventory and predicate preservation, independent body/display-name changes, contract revalidation, competing signatures and deleted dependencies, exact shared history handling, stale selectors and no writes; [Expression Change](PROJECT-EXPRESSION-CHANGE-V1.md), [Contract Change](PROJECT-CONTRACT-CHANGE-V1.md), and [Candidate Rebase](PROJECT-CANDIDATE-REBASE-V1.md) own authored/unrun cases |
| Workspace transaction | Held-input rechecks, replay before candidate/staging, one publication pivot, old-or-new process termination evidence |
| Canonical Semantic Workspace Revision v1 | Deterministic authority-free derivation from one admitted immutable Project; exact schemas and all nine typed nodes; separate semantic/source-projection/manifest/dependency-lock component digests and ordered composite revision; exact fresh replay; stale, malformed, reminted and one-byte-over-limit rejection; no writes or authority; byte-identical legacy Project, managed Workspace and Semantic Workspace Image v1 artifacts; [Canonical Revision v1](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md) owns the locally passed focused evidence |
| Universal Semantic Transaction v1 | Exact canonical one-operation envelope; composite base and old-name preconditions; explicit monomorphic non-main function restriction; comment-free canonical source guard; deterministic intent/impact/review/result/evidence; fresh exact replay; ProjectCandidate parity; stale, malformed and reminted rejection; no writes or authority; unchanged legacy bytes; [Universal Semantic Transaction v1](UNIVERSAL-SEMANTIC-TRANSACTION-V1.md) owns the locally passed focused gate |
| Universal Semantic Transaction Composition v1 | Deterministic canonical structural diff with exact four-component/nine-node bindings, semantic-delta catalogue, source review and exact replay; unrelated-drift rebase with reminted transaction/direct Candidate parity and replay; both explicit distinct-target merge orders with direct Candidate parity and replay; same-target, stale, tampered, noncanonical and cross-base rejection; exact CLI/core parity with closed grammar and held-root preservation; no filesystem writes; unchanged Semantic Transaction v1, Candidate and canonical-workspace bytes. [Universal Semantic Transaction Composition v1](UNIVERSAL-SEMANTIC-TRANSACTION-COMPOSITION-V1.md) owns the locally passed five-case Project-Candidate and four-case Workspace CLI gates. |
| Universal Semantic Query v1 | All five typed constructors and exact canonical parsing; deterministic revision-bound request/result with domain-separated digests; bounded declaration paging with stable direct-query parity; exact symbol/context/impact delegation; truthful shared transaction eligibility paired with a valid rename and unavailable-target cases; fresh exact replay; stale, malformed, noncanonical, reminted and oversized rejection; immutable old snapshots after refresh; no writes, mutation, transport or authority. Frozen Project Agent Transport v5 is outside this additive core and must remain unchanged. [Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md) owns the locally passed focused integration gate. |
| Persistent Incremental Semantic Workspace Service v1 | Deterministic cold and caller-supplied semantic-cache opens; immutable revision-bound snapshots and bounded symbol/context/impact delegation; canonical bounded work and refresh receipts; source-exact incremental reuse with cold-equivalent Project, graph and canonical-revision results; one expected-current atomic in-memory generation/cache CAS; stale and failed refresh preserve the complete installed generation; transaction validation returns exact artifacts without mutation; no filesystem, transport, execution or publication authority. [Persistent Incremental Semantic Workspace Service v1](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md) owns the locally passed focused gate covering cold open, query delegation, unchanged and one-source refresh accounting, cold equivalence, rollback, transaction parity/staleness and no writes. |
| Universal Semantic Workflow CLI v1 | One authenticated Project lifetime per invocation; exact core JSON for declarations, symbol, context, impact and available operations; exact transaction result/evidence for display-rename preview; explicit/current revision binding and stale rejection; closed subcommand/options grammar; unchanged legacy query behavior; no source, cache, generation, transport or publication writes. Project Agent Transport v5 remains outside the adapter and unchanged, but its separate conformance corpus is not part of this focused gate. [Universal Semantic Workflow CLI v1](UNIVERSAL-SEMANTIC-WORKFLOW-CLI-V1.md) owns the locally passed five-case Workspace-harness gate. |
| Installed Agent Guidance v1 | Exact core/CLI parity for all six closed skill selectors; recursively canonical LF-terminated envelopes, domain-separated payload and embedded-source digests, installed package-version binding, optional commit grammar and the one-MiB cap; inert malformed/unknown grammar and empty working directory; exact five-operation query capability inventory with no host grants or authority; unchanged legacy source-query bytes. [Installed Agent Guidance v1](INSTALLED-AGENT-GUIDANCE-V1.md) owns the locally passed five-case focused projections-harness gate. |
| Installed Diagnostics v1 | Independent exact rescan of every static diagnostic code occurrence below `src/` and `crates/`, with the complete unresolved dynamic-constructor-site inventory retained; deterministic canonical LF envelopes, domain-separated payload digests, installed version binding and 8-MiB/1-MiB bounds; exact core/default-text/JSON CLI parity from an empty working directory; malformed, unknown, noncanonical, tampered and oversized rejection; no writes; unchanged legacy diagnostic text and JSON. [Installed Diagnostics v1](INSTALLED-DIAGNOSTICS-V1.md) owns the locally passed five-case focused projections-harness gate. |
| Installed Fix Plan v1 | Exact deterministic one-operation installed catalog; canonical LF bytes, domain-separated payload and embedded-report digests, compiler version binding and 1-MiB/64-MiB bounds; exact current-source Diagnostic Repair report/source binding and fresh replay; exact core/CLI parity for both closed `fix --plan` forms; malformed, unavailable, noncanonical, tampered, oversized and stale rejection; no writes or authority; unchanged legacy `repairs` and `repair` bytes. [Installed Fix Plan v1](INSTALLED-FIX-PLAN-V1.md) owns the locally passed five-case semantic-harness gate. |
| Candidate tests and v3 execution | Exact candidate replay, real transitive HIR relevance, conservative non-call fallback, fixed host policy, nonzero/fuel failure, source/test/options/diff binding, old-profile rejection and no request elevation; [Candidate Tests](PROJECT-CANDIDATE-TESTS-V1.md) and [Test Protocol](IMAGE-CANDIDATE-TEST-PROTOCOL-V3.md) own authored/unrun cases |
| Rejected candidate attempts | Exact predecessor/intent/diagnostic provenance, no invalid source/image exposure, stale selectors and compiler-admitted same-value repair only; [Candidate Diagnostics](PROJECT-CANDIDATE-DIAGNOSTICS-V1.md) owns authored/unrun cases |
| Candidate managed publication | Lock before replay, exact host-approved candidate and ACTIVE base, independent Project/source/evidence reconstruction before staging, existing single pivot, unchanged raw files and explicit postpublication uncertainty; [Candidate Publication](PROJECT-CANDIDATE-PUBLICATION-V1.md) owns authored/unrun cases |
| Semantic image store and refresh | Secure source-backed store reuse, exact receipt/image replay, stale/corrupt/deleted inputs, same-revision reuse and conservative reverse-module invalidation; [Image Store](SEMANTIC-IMAGE-STORE-V1.md) owns authored/unrun cases |
| Semantic deltas and diagnostic protocol v4 | Exact source-bound fact replay, no invented runtime/equivalence claims, bounded UTF8 chunks and attempt accounting, legacy method preservation and no request capability escalation; [Delta](PROJECT-CANDIDATE-SEMANTIC-DELTA-V1.md) and [Diagnostic Protocol](IMAGE-CANDIDATE-DIAGNOSTIC-PROTOCOL-V4.md) own authored/unrun cases |
| Integrated graph workflow | Cross-file signature migration, unrelated merge, competing-signature rejection, reports/test policy, separate managed publication and stale rejection with unchanged raw source; [Workflow](PROJECT-GRAPH-OPERATIONAL-WORKFLOW-V1.md) remains authored/unrun |
| Incremental frontend and expression holes | Source-exact cache keys, actual parse/canonicalization reuse, invalidation and cold semantic-output equivalence; disjoint selections, lexical scope, overlap rejection, full fill replay, surviving-selector remapping and no unresolved materialization; [Frontend Cache](PROJECT-FRONTEND-CACHE-V1.md) and [Expression Holes](PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md) own authored/unrun cases |
| Persistent semantic-cache lifecycle | Existing empty private root; exact cold source admission, authenticated stored restore, unchanged explicit refresh, digest-selected eviction and byte-identical cold reconstruction; unchanged canonical source bytes; exact Project/image identity; required nonzero cold resolutions, zero cold hits and complete warm/refresh hits; bounded deterministic work/retained-byte receipt and honest exclusion of timing, RSS, cross-process and crash claims. [Semantic Cache Store v1](SEMANTIC-CACHE-STORE-V1.md) owns the authored/unrun gate. |
| Candidate Git publication | Independent source and Git-object authentication, exact approved candidate, preserved unrelated tree entries, host-selected bare repository/ref, held executable/cwd use under same-byte and ancestor substitution, fixed non-inherited environment and exact descriptor inventory, bounded I/O/deadline, leader reap plus process-group quiescence, old-OID compare-and-swap, disabled ambient hooks/network and explicit post-pivot uncertainty; [Git Publication](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md) owns the gate |
| Root unsafe quarantine | `root_unsafe_quarantine` plus strict Clippy prove the root manifest denies unsafe code, reject allow/expect/warn overrides outside the held-Git platform module, source-lock that sole local exception, and confirm the private module exposes no unrestricted public process API |
| Source-backed static protocol conformance | Canonical protocol/impl preservation, original-source locality before synthetic imports, global identity uniqueness, exact required-member signatures and effect/precondition rejection; [Static Conformance](STATIC-PROTOCOL-CONFORMANCE-V1.md) owns authored/unrun cases |
| Typed interface candidates and image conformance | Complete member discovery, preserved source binding identities, exact replay/recovery, explicit rebase rejection, source-bound read-only image reports and v4 chunks with legacy method exclusion; [Interface Changes](PROJECT-INTERFACE-CHANGE-V1.md) and [Image Conformance](IMAGE-PROTOCOL-CONFORMANCE-V1.md) own authored/unrun cases |
| Candidate moves and record fields | Exact stable-ID relocation and import/call rebinding, no effect/export widening, pure appended defaults after existing constructor evaluation, recursive exact-pattern migration, preserved old field identities, complete source replay and conflict handling; [Declaration Move](PROJECT-DECLARATION-MOVE-V1.md) and [Record Field Change](PROJECT-RECORD-FIELD-CHANGE-V1.md) own authored/unrun cases |
| Image HIR relationships | Exact ValueId/field/expression/source facts, declared consumption contexts, bounded deterministic traversal and paging, unchanged prior facet handles/payloads, and fail-closed unsafe Project admission; [HIR Relationships](SEMANTIC-IMAGE-HIR-RELATIONSHIPS-V1.md) owns authored/unrun cases |
| Candidate declaration, extraction and recovery | Exact one-function identity extension, namespace/effect/ownership admission, actual ValueId capture order, Copy-only captures, exact whole provisional publication for a resource-free owned result, no crossing loans, and unsafe-boundary rejection; later edits and merge of introduced identities, complete canonical history replay, tampered/stale/capacity failures and unchanged source/read-only authority; [Declaration Change](PROJECT-DECLARATION-CHANGE-V1.md), [Extraction](PROJECT-EXTRACTION-V1.md), and [Recovery](PROJECT-CANDIDATE-RECOVERY-V1.md) own authored/unrun cases |
| Project manifest or carrier | Exact source-set authentication, Phase-A reuse, closure/admission checks, carrier replay, post-publication drift behavior |
| Windows owned npm publication | Opaque compiler-prepared six-file handoff; exact v8/v9/v10 inline/published equality; standalone CLI/library pre-effect rejection and full-host aliases; source drift and primary failures; held-parent/stage/inventory/byte authentication, no-clobber and post-settlement no-rollback; actual Windows Node consumers and unchanged Unix/older-profile routes. [Windows publication v1](WINDOWS-OWNED-NPM-PUBLICATION-V1.md) owns the authored, unrun gate. |
| Project profile admission | Exhaustive v1-v10 schema/profile dispatch, descriptor derive/replay equality, ordinary v9 load and Revision Store round trip, v9/v10 execution-envelope replay, exact earlier-profile bytes and diagnostics |
| Windows Project Revision Store | Explicit Windows-entry-v1 APIs/schema, unchanged ordinary v1 bytes, protected effective-SID/LocalSystem DACL and mutex authority, fixed-local-NTFS/alias/ADS/reparse/link admission, bounded held reads/inventory, exact retained-parent publication and settlement, provisioned-host physical fixtures, all admitted Project profile round trips, and no support promotion from skipped or unrun gates |
| Project agent transport | Closed method/parameter schemas, exact revision binding, pre/post held-input authentication, response framing boundaries, zero-write inventory, hostile replay, and byte-preserved earlier protocols |
| Native backend or ABI | C11 compilation at required optimization levels, descriptor/header agreement, runtime status and cleanup conformance; Project-v8's public C boundary additionally executes the named Option/Result tags at O0/O2, rejects inactive-case handle authority, and settles every active handle exactly once |
| V10 inline String settlement | Real descriptor replay and native provider generation; strict O0/O2 allocation/free accounting, failure-slot poison, late-argument/callee/local/loop failures, clone/branch/pressure and mixed Bytes ownership, same-context reuse after failure, explicitly selected sanitizers, safe locked/offline Rust consumer, and earlier-provider preservation except the explicit internal-String correction below; ordinary C corrections have a separate gate and context-handle closure is not a physical-allocation proof |
| Ordinary/stdout inline String settlement and contents | Actual emitted-C O0/O2 allocation/free counts, normalized failure and poisoned out slots, parameter/temporary/provisional settlement, branches/loops/contracts/intrinsics and mixed Bytes, generic-instance runtime discovery, exact-length NUL/Unicode contents across admitted interpreter/native/Wasm value lanes, explicit sanitizers, String-free byte/budget preservation, unchanged v10/command/callable selectors, and Target Evidence/Evidence-v2 binding to current production C; no inferred ordinary Wasm settlement |
| Owned-data provider internal Strings | [Explicit v8/v9 correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md): actual descriptor replay/provider generation, physical allocation/free accounting beyond handle closure, strict O0/O2 and provisioned sanitizers, pre/post-call-commit and mixed Bytes failure settlement, exact NUL/Unicode values, poisoned output slots, same-context reuse, real generated locked/offline safe Rust consumer; String-free output/budget and existing KAT preservation, no activated Project admission widening; all new execution remains unrun |
| Wasm or JavaScript boundary | Structural Wasm validation, generated binding checks, Node execution, and browser/multi-engine evidence when claimed |
| Direct-Bytes browser boundary | Execute the provisioned [Owned Data Browser v1](../platform-tests/owned-data-browser-v1/README.md) on each selected engine: real package imports, exact fixture signatures/carrier, capacity and hostile-input rejection, calibrated pre-instantiation Wasm authentication and genuine failure recovery; missing prerequisites fail and authored cases alone grant no browser promotion |
| Owned-data boundary corrections | Zero payload snapshot allocation on complete-tuple rejection; intrinsic-brand/species hostility; exact UTF-8/input bounds; selected-call-path private-frame exclusion and failure-slot poison; live foreign-context/reincarnation handle rejection; 4,095/4,096/4,097 live-slot and serial-exhaustion/contention settlement; v1-v7 preservation and explicit v8-v10 artifact deltas, subject to the separately documented shared arithmetic correction below |
| Checked `usize` multiplication | [Shared Wasm correction](PORTABLE-INDEXED-BYTE-DATA-V1.md#checked-multiplication-correction): ordinary and aggregate routes, zero on both sides, maximum/exact/overflow boundaries, evaluated-left failure precedence, nested status-branch depth, actual staged-owner success and failure settlement, preserved failed output, and same-instance recovery across interpreter/native O0/O2/generated npm; affected Wasm/integrity bytes change intentionally, not schemas or native behavior |
| Owned npm invocation failure state | [Shared v8/v9/v10 contract](OWNED-NPM-INVOCATION-V1.md): seven real generated packages covering direct/variant/mixed/flat renderers; reusable preflight and authentic semantic failure; unexpected type/range/falsy throws, malformed statuses, forged semantic markers, caught reentry, post-consume UTF-8 failure, sticky primary under cleanup failure, no later engine/import/publication after poison, unchanged non-runtime artifacts and historical cryptographic pins. Actual generated-result decoding must reject corrupted tags/carriers/bools and modified failure slots with calibrated payload-read/consume observations, same-arena stale tokens, inactive-storage non-access and live-owner settlement disagreement; a test-local decoder or arena is not a substitute. |
| Doctor tool detection and subprocess lifetime | Real basename-sensitive multicall symlink, relative PATH and non-executable shadows, missing/failed tool exits, complete numeric version and suffix admission, unchanged report schemas/order, and physical exact/plus-one output, timeout, descendant, descriptor and fail-stop settlement fixtures on all supported hosts. Linux additionally requires actual BPF policy interpretation, direct/exec-descendant syscall denial, before/after unfiltered-host controls, actual filter-install rejection before executable entry, unsupported-ABI rejection and unchanged Command-descendant compatibility gates. The [lifecycle contract](DOCTOR-PROBE-V1.md) leaves complete discovery/filesystem/broker and cross-platform no-network isolation open. |
| Linux production doctor provisioner | Exact Ed25519 capsule/request/bundle/static-image binding, fixed descriptor inventory, procfs/cgroup2 authentication, pre-effect bounds, role-specific default-deny syscall tables, private namespace maps, atomic cgroup placement, held-image execution, sticky capture failure, exact reap and `populated 0`; the [Provisioner v1](DOCTOR-PRODUCTION-PROVISIONER-V1.md) additionally requires an unpacked signed release with real Clang/Node/Rust plus hostile authority and settlement cases before promotion. Missing kernel/cgroup/sealing prerequisites fail rather than skip. |
| Signed doctor generation install | Hold the exact private Unix store root for the API lifetime; verify signed release meaning from held exact member bytes; reject unsafe roots, modes, links, surplus, adoption and substitution; fsync members/stage/root around no-replace generation publication; reverify before cooperative expected-current activation/rollback; classify post-pivot failures as sticky uncertainty; authenticate every held stage member before recovery's first effect and preserve foreign bytes. [Signed Install v1](DOCTOR-SIGNED-INSTALL-V1.md) owns the local gate and explicitly excludes Windows, CLI execution and kernel-CAS claims. |
| Project v8 promotion receipt replay | Closed canonical one-line schema; exact 40-lowerhex commit; baseline/display-rename subject and stable-ID bindings; fixed eight-artifact inventory; closed ordered fifteen-gate platform/tool inventory; pass-only observations; domain-separated receipt and artifact digests; unknown/duplicate/reordered/deep/oversized/reminted/cross-profile rejection. [Promotion Receipt v1](PROJECT-V8-PROMOTION-RECEIPT-V1.md) is authority-free local evidence and does not itself satisfy WP-15 or hosted promotion. |
| Report or schema projection | Closed admission/exclusion vocabulary, deterministic envelope, independent replay, tamper and budget rejection, cross-report consistency |
| Offline package resolution | Strict SemVer/range boundaries, deterministic permutation and first-feasible backtracking, multi-root/transitive closure, conflict/duplicate/cycle rejection, exact bounds, ternary-target and capability policy, subject/report and outer-wire remint rejection, exact replay, and preserved Report/Lock/Compatibility bytes |
| Offline published semantic lock snapshot | Exact raw Subject-v2 preservation, catalog-permutation canonicalization, input/evidence/Lock cross-pair and remint rejection, checked component/cumulative bounds, two complete replays around held staging, exact fixed three-file inventory, no-replace publication, settlement/uncertainty/foreign-byte evidence, platform authority preconditions, and unchanged Resolver/Lock/build-v1/v2 bytes and diagnostics |
| Authenticated package ranges | Subject-v3 dependency order/self/grammar rejection, exact selected-version range binding, range intersection and rollback, numeric candidate ordering, catalog permutation, cycle/depth/edge/decision/work exact bounds, Lock-v3 and Resolver-v2 tamper/remint/cross-pair replay, and exact v1/v2 API/schema/byte/diagnostic preservation |
| Offline multi-package source capsule | Two-to-four-package success, exact resolver/subject/report replay, explicit-root and root-only export binding, exact typed-interface comparison ignoring display/parameter names, source-import versus dependency-graph equality, unreachable/provider/type/effect rejection, canonical-wire and every exact/+1 bound, tamper/cross-pair replay, and preserved Report/Lock/Resolver/build-v1 bytes |
| Offline linked scalar Wasm package build | Real two-package capsule-to-build replay, root-only export ownership, exact seven-import/export inventory, distinct v2 canonical manifest/evidence, mutation and cross-pair rejection, cumulative artifact/evidence and fixed-point boundaries, two compiler replays around held publication, exact three-file inventory, cleanup/uncertainty/post-publication authentication, and unchanged build-v1 bytes/order |
| Private host integration | Authority inventory, fail-stop uncertainty, process/loader settlement, platform-specific hosted jobs |
| Calculator project publication | [Owning contract](NEW-PROJECT-PUBLICATION-V1.md): unchanged template bytes and ordinary Project validation, relative/parent-relative success, post-rename held-parent/output and original-alias displacement, preserved original/foreign inventories after failure and drop, partial-stage residue rejection; forced Windows extended-to-legacy call with actual zero replacement field plus native success and collision preservation; all new physical gates remain unrun |
| Public Project scaffold capsule | [Owning contract](PROJECT-SCAFFOLD-V1.md): exact ordered four-file bytes, ordinary and top-level digests, subject-bound canonical replay, every semantic/canonical mutation, exact/+1 descriptor capacity, private-`new` byte convergence, stdout-only public CLI, empty working directory, and unchanged Project-v1/private-publication known answers; authored evidence is unrun and grants no filesystem or publication authority |
| Standard library | Exact package-directory/catalog agreement; canonical sources; stable `std.*` identities; examples and conformance on interpreter, native C11 O0/O2, and Core Wasm; bundled dependency range and multi-package linking; cross-file `borrow str`; exact calculator/library staged-publication inventories |
| Semantic Workspace Image | Execute the authored, unrun [Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md) exact replay, typed-index, stale/drift, capacity, deterministic cross-root, CLI and zero-write evidence before promotion |
| Public scalar WIT interface | [Owning contract](PUBLIC-SCALAR-WIT-INTERFACE-V1.md): exact retained Project-v1 selection, injective stable-ID names, ordinal parameters, `result<T, status>`, deterministic bounded WIT and descriptor bytes, independent subject/HIR replay, mutation/truncation/trailing/remint/cross-subject rejection, wrong-profile pre-effect rejection, external parser and consumer evidence, and proof that access and replay perform no new target or I/O activity after Project admission. Interface evidence is not Component execution. |
| Public API or generated SDK | External consumer with no source/workspace dependency, locked offline build, inventory and compatibility checks; retained-HIR descriptor binding through authentic self-replay and correctly digested cross-replay rejection, as specified by [owned data](PUBLIC-OWNED-DATA-API-V1.md) and [flat records](PUBLIC-FLAT-OWNED-RECORD-API-V1.md) |
| Unix npm publication | Real-carrier parent/ancestor substitution, exact retained artifact and foreign-byte preservation, healthy alias binding, unchanged no-clobber behavior and thread-local fixture isolation; [Project Manifest v2](PROJECT-MANIFEST-V2.md) owns the shared boundary and unrun regression modules |
| Shared full-toolchain test launcher | Exact Cargo artifact selection, stale guessed-path rejection, unique manifest-bound binary and successful build completion; [development](DEVELOPMENT.md#verification) owns the helper boundary and authored/unrun regression entry point |
| Unpacked release product | Explicit native archive admission, exact inventory and manifest/version agreement, outside-checkout calculator and read-only daemon execution, stable source/package bytes, and real generated Node/Rust consumers; [release process](RELEASE-PROCESS.md) separates artifact labels, local execution and release provenance. No implicit archive build, extraction, installation or hosted promotion. |

The owning specification lists exact focused tests. If it does not, add the
missing evidence section there instead of growing this document into a second
copy of the spec.

## Required semantic cases

When runtime meaning changes, cover all applicable cases:

- minimum and maximum admitted values and capacities;
- exact-capacity success and capacity-plus-one rejection;
- left-to-right evaluation and lazy boolean behavior;
- first-failure stickiness;
- success, contract failure, runtime failure, and cleanup failure;
- repeated entry and deterministic output;
- stale source, source drift, tampered evidence, and forged re-digested input;
- unsupported profile rejection before target or filesystem effects;
- unchanged bytes for older schema versions and unaffected examples.

Never weaken a diagnostic or golden merely to make the gate green. A deliberate
wire change needs a migration, an updated versioned contract, and explicit
compatibility evidence.

## Public Native Rust SDK promotion

The generated Rust SDK is a useful example of evidence layers. Local promotion
requires the focused `public_native_rust_sdk_v1` and
`public_native_rust_sdk_ci_contract` suites plus the standalone
`examples/calculator-rust` consumer. The consumer must use the generated
package with no repository source or workspace dependency and build in locked
offline mode.

Public promotion additionally requires the blocking Ubuntu, macOS, and Windows
jobs at the exact claimed commit, including deterministic inventory,
tool-authority, failure-settlement, and compiler-free consumer evidence. The
builder remains unpublished until that boundary is intentionally promoted.

## Hosted evidence

Hosted claims require the exact workflow jobs named by the owning
specification. A prior-head run is historical evidence only. A diagnostic or
allowed-failure job is not a passing promotion gate.

A cancelled run is neither. The `Release gate` job aggregates every CI blocker
and fails, rather than skipping, when any of them failed, was skipped, was
cancelled, is missing from its dependency set, or reported against another
commit; [required CI checks](CI-REQUIRED-CHECKS-V1.md) owns that gate's contract
and the unapplied repository-rule proposal that would make it a required check
for `main`. No branch rule is in force today, so a green gate is evidence about
a commit, not a precondition that any commit had to meet.

The current released baseline is annotated tag `v0.2.0` at exact commit
`5f6fb9655fdec92c57ab71615cfd7bfa8cc76051`. All 45 jobs in
[tag run 33608662244](https://github.com/wavect/semaprax/actions/runs/33608662244)
passed, including the blocking release aggregation, three host-built archive
smokes, and final prerelease publication. The exact asset inventory and
digests live in the [release evidence record](RELEASE-PROCESS.md#v020-hosted-release-evidence).
That run promotes release evidence only where an owning gate selects it; it
does not turn ignored, unprovisioned, multi-engine, physical-device, registry,
or production-support requirements into passing evidence.

For platform claims:

- compilation or object inspection is not runtime execution;
- simulator evidence is not physical-device evidence;
- Node execution is not browser or multi-engine evidence;
- one operating system is not a cross-platform matrix;
- a private fixture is not a supported public SDK or application surface.

Record exact commit and run links in the owning specification's status/evidence
section or the changelog. The completion matrix should link to the owner rather
than duplicate those run IDs.

## Evidence strength

From weakest to strongest:

1. design text;
2. compilation or structural inspection;
3. deterministic local unit/integration evidence;
4. independent replay and hostile-input evidence;
5. exact-head hosted execution on the required target matrix;
6. external consumer or representative application evidence;
7. maintained release and compatibility evidence.

Higher evidence does not erase scope limits. A perfectly replayed scalar report
is still a scalar report; it does not prove general aggregates, resources, or
production interoperability.
