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
safe enough for a narrower run.

| Profile | Intended use | Gates |
| --- | --- | --- |
| `quick` | Early local feedback | diff check, Rust formatting, workspace check, advisory documentation/examples/context tests |
| `changed` | Bounded reviewed changes | `quick` plus package Clippy, agent-context integration, and package rustdoc |
| `full` | Semantic changes and release candidates | workspace Clippy/tests/doctests/rustdoc, release build, package check, and canonical example checks |

Capability-aware command help additionally requires the exact catalog/dispatcher
inventory, global-byte preservation, standalone/full capability separation,
scoped and malformed-position behavior, and zero-activity gates owned by
[Capability-Aware CLI Help v1](CLI-HELP-V1.md).

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
locked dependencies, the all-targets/all-features check, fail-fast execution,
and the 20-minute job limit. Shared integration target names stay together;
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

`tests/documentation.rs` checks local Markdown links recursively. The docs
workflow builds the mdBook using the pinned version in
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
| Opt-in internal String interpreter | Distinct schema/domain and cross-profile rejection, frozen ordinary/Project/prepared/effectful admission, unchanged external String rejection, source and envelope bounds, canonical/duplicate/re-signed hostile-wire rejection, exact output capacity, String call/contract/failure value parity, fuel/depth boundaries, CLI behavior, and unchanged legacy golden/fuel facts; no heap-memory or Wasm settlement inference |
| Standalone Wasm internal String settlement | Distinct explicit profile, structural module validation, fixed memory and selected acyclic stack/owner bounds; independent raw mint/drop accounting and every reached mint-refusal path, generated-host exact/+1 quotas and poison/reentry, exact artifact/input binding, native O0/O2 and internal-String interpreter parity, legal scalar-loop helper reuse, unchanged U105/T252/J113 rejection and legacy artifact known answers; [local validation record](WASM-INTERNAL-STRINGS-V1.md#local-validation-record) owns bounded partial evidence, with cross-platform, full-profile, and hosted gates remaining; no support promotion or ordinary-Wasm, peak-heap or trap-recovery inference |
| Standalone internal String Web package | Actual explicit-source CLI selection and pre-effect usage rejection; bounded source snapshot and final drift recheck; source/descriptor/package exact/+1 bounds; exact eight-file inventory, independent manifest/digest replay and direct compiler-output equality; deterministic repeat and stable-ID rename, hostile identities, fresh-parent publication and foreign-byte preservation; real generated Node, strict provisioned TypeScript and provisioned browser consumers including streamed fetch rejection before EOF; pre-effect legacy String rejection including materialized generic bodies, unchanged raw emission and String-free legacy bytes; the [package validation record](WASM-INTERNAL-STRINGS-WEB-V1.md#local-validation-record) separates selected local consumers and real source/descriptor boundaries from private renderer accounting and unrun required-host/release gates; no support promotion |
| Prepared Project interpreter or source trace | One cached exact closure admission and one persistent worker across repeated entry/test execution; legacy outcome/fuel parity; cancellation boundaries; exact node/byte/event limits; deterministic truncation; canonical replay; retained-HIR source-origin binding; worker panic/disconnect fail-stop; and unchanged Interpreter/Project/Transport v1-v5 bytes |
| Prepared Project revision replacement | Exact expected-content revision before candidate preparation, both closures/origins swapped together, byte-identical old execution after stale or ordinary candidate rejection, new/old trace cross-binding, same worker and permit, unchanged ceilings/cancellation/admission, concurrent-operation rejection, and terminal panic or lost acknowledgement; no epoch, incremental-compiler, or peak-heap inference |
| Ownership or cleanup | Structural inventory, canonical plan build, independent replay, hostile mutation, success/failure settlement |
| Graph schema | Exact new projection, legacy byte preservation, context projection, invalid/tampered rejection |
| Semantic patch or repair | Preview, stale/drift rejection, no-write failures, independent replay, atomic A0 application |
| Project candidates and typed intentions | Exact canonical change admission, complete caller migration/evaluation order, AST/source round-trip and complete Project replay, unchanged explicit identities/effects/contracts, core target admission preservation, stale/tampered evidence rejection and no writes; [Candidate v1](PROJECT-CANDIDATES-V1.md) owns the focused unrun evidence |
| Candidate holes and ordered signature mapping | Unresolved drafts expose no source/evidence, failed fills preserve every sibling, exact stale/duplicate/capacity rejection, actual scope/contracts/effect budget, original argument evaluation order including removed arguments, hygienic bindings, and ownership-mode rejection; [Holes v1](PROJECT-CANDIDATE-HOLES-V1.md) and [Signature Evolution v1](PROJECT-SIGNATURE-EVOLUTION-V1.md) own authored/unrun cases |
| Unified workspace protocol v5 | Exact startup capability matrix and rejection of RPC elevation; frozen v1–v4 bytes; expected-old/new cold refresh, failure immutability, preserved candidate handles and cleared drafts/attempts; bounded output before state mutation; [Workspace Protocol v5](IMAGE-WORKSPACE-PROTOCOL-V5.md) and [Session CLI](WORKSPACE-SESSION-CLI-V1.md) own authored/unrun cases |
| Source-backed candidate archives and startup recovery | Exact canonical original-source rebuilding plus unchanged capsule replay; stale/rehash/compatibility/authority-claim rejection; recovery after raw source removal; private normalized root and held ancestor identities; single-link files, bounded inventory/read, no-replace pivot and retained failed stages; post-pivot uncertainty; startup-only same-manifest candidate handoff with live pre/post authentication and no approval restoration; v1/v2 policy and full historical CLI help preservation. [Archive](PROJECT-CANDIDATE-ARCHIVE-V1.md), [Store](CANDIDATE-ARCHIVE-STORE-V1.md), [Recovery](IMAGE-WORKSPACE-ARCHIVE-RECOVERY-V1.md), and [CLI](CANDIDATE-ARCHIVE-CLI-V1.md) own authored/unrun evidence |
| Live frontend reuse and parallel image reads | Exact-source parser/formatter reuse without a preliminary cold build; unchanged cold/image bytes; source path/identity authentication despite cache hits; staged preview/failed-refresh rollback; strict host-policy v1/v2 separation; bounded worker overlap, ordered sequential-byte equivalence, no mutable/execution/publication dispatch, all-worker join on failure and whole-batch stale rejection. [Live Frontend Cache](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md) and [Parallel Reads](IMAGE-PARALLEL-READS-V1.md) own authored/unrun cases |
| V5 source-commit extension | Independent startup-only approval, exact restored candidate/base selection, no request-selected Git policy, approval consumption, one-shot success and G267 uncertainty, bounded retained receipt/chunks and historical status after drift; generic transport checks must not mask a pivot outcome; [Source Commit v5](IMAGE-SOURCE-COMMIT-PROTOCOL-V5.md) owns authored/unrun cases |
| Integrated v5 signature-to-Git workflow | Execute both real SHA1/SHA256 provider scenarios, exact migrated-call/source/semantic-delta checks, preserved contract/effect/export/parameter facts, explicit-policy tests, all four target projection rows, separate review/restore/approval sessions, committed source/parent/unrelated-mode preservation, wrong approval and actual stale-ref preflight. [Git Workflow v1](PROJECT-GRAPH-OPERATIONAL-GIT-WORKFLOW-V1.md) owns authored/unrun cases; general ownership, native/Wasm execution and physical CAS-race evidence remain separate |
| V5 targets, artifacts and typed discovery | Actual closure target-emission facts without false per-symbol blame; independent Web/npm carrier replay and source/export/file bindings; candidate replay before pathless builds; zero artifact filesystem writes; runtime-granted catalog/schema/client alignment and explicit opaque-response gaps; executed cross-language validation remains required; [Target/Artifact Projections](IMAGE-TARGET-ARTIFACTS-V1.md) and [Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md) own authored/unrun cases |
| Candidate-only image protocol | Explicit host selection, preserved read-only v1, schema/catalog alignment, exact handle selection, bounded registries and response-before-mutation, source drift invalidation, independent candidate replay, no source/test/build authority, and hole lifecycle; [Candidate Protocol v2](IMAGE-CANDIDATE-PROTOCOL-V2.md) owns authored/unrun cases |
| Candidate expression/contract changes and rebase | Actual HIR/source expression identity and lexical scope, exact expected type/ownership after source replay, additive contract inventory and predicate preservation, independent body/display-name changes, contract revalidation, competing signatures and deleted dependencies, exact shared history handling, stale selectors and no writes; [Expression Change](PROJECT-EXPRESSION-CHANGE-V1.md), [Contract Change](PROJECT-CONTRACT-CHANGE-V1.md), and [Candidate Rebase](PROJECT-CANDIDATE-REBASE-V1.md) own authored/unrun cases |
| Workspace transaction | Held-input rechecks, replay before candidate/staging, one publication pivot, old-or-new process termination evidence |
| Candidate tests and v3 execution | Exact candidate replay, real transitive HIR relevance, conservative non-call fallback, fixed host policy, nonzero/fuel failure, source/test/options/diff binding, old-profile rejection and no request elevation; [Candidate Tests](PROJECT-CANDIDATE-TESTS-V1.md) and [Test Protocol](IMAGE-CANDIDATE-TEST-PROTOCOL-V3.md) own authored/unrun cases |
| Rejected candidate attempts | Exact predecessor/intent/diagnostic provenance, no invalid source/image exposure, stale selectors and compiler-admitted same-value repair only; [Candidate Diagnostics](PROJECT-CANDIDATE-DIAGNOSTICS-V1.md) owns authored/unrun cases |
| Candidate managed publication | Lock before replay, exact host-approved candidate and ACTIVE base, independent Project/source/evidence reconstruction before staging, existing single pivot, unchanged raw files and explicit postpublication uncertainty; [Candidate Publication](PROJECT-CANDIDATE-PUBLICATION-V1.md) owns authored/unrun cases |
| Semantic image store and refresh | Secure source-backed store reuse, exact receipt/image replay, stale/corrupt/deleted inputs, same-revision reuse and conservative reverse-module invalidation; [Image Store](SEMANTIC-IMAGE-STORE-V1.md) owns authored/unrun cases |
| Semantic deltas and diagnostic protocol v4 | Exact source-bound fact replay, no invented runtime/equivalence claims, bounded UTF8 chunks and attempt accounting, legacy method preservation and no request capability escalation; [Delta](PROJECT-CANDIDATE-SEMANTIC-DELTA-V1.md) and [Diagnostic Protocol](IMAGE-CANDIDATE-DIAGNOSTIC-PROTOCOL-V4.md) own authored/unrun cases |
| Integrated graph workflow | Cross-file signature migration, unrelated merge, competing-signature rejection, reports/test policy, separate managed publication and stale rejection with unchanged raw source; [Workflow](PROJECT-GRAPH-OPERATIONAL-WORKFLOW-V1.md) remains authored/unrun |
| Incremental frontend and expression holes | Source-exact cache keys, actual parse/canonicalization reuse, invalidation and cold semantic-output equivalence; disjoint selections, lexical scope, overlap rejection, full fill replay, surviving-selector remapping and no unresolved materialization; [Frontend Cache](PROJECT-FRONTEND-CACHE-V1.md) and [Expression Holes](PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md) own authored/unrun cases |
| Candidate Git publication | Independent source and Git-object authentication, exact approved candidate, preserved unrelated tree entries, host-selected bare repository/ref, old-OID compare-and-swap, disabled ambient hooks/network and explicit post-pivot uncertainty; [Git Publication](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md) owns authored/unrun cases |
| Source-backed static protocol conformance | Canonical protocol/impl preservation, original-source locality before synthetic imports, global identity uniqueness, exact required-member signatures and effect/precondition rejection; [Static Conformance](STATIC-PROTOCOL-CONFORMANCE-V1.md) owns authored/unrun cases |
| Typed interface candidates and image conformance | Complete member discovery, preserved source binding identities, exact replay/recovery, explicit rebase rejection, source-bound read-only image reports and v4 chunks with legacy method exclusion; [Interface Changes](PROJECT-INTERFACE-CHANGE-V1.md) and [Image Conformance](IMAGE-PROTOCOL-CONFORMANCE-V1.md) own authored/unrun cases |
| Candidate moves and record fields | Exact stable-ID relocation and import/call rebinding, no effect/export widening, pure appended defaults after existing constructor evaluation, recursive exact-pattern migration, preserved old field identities, complete source replay and conflict handling; [Declaration Move](PROJECT-DECLARATION-MOVE-V1.md) and [Record Field Change](PROJECT-RECORD-FIELD-CHANGE-V1.md) own authored/unrun cases |
| Image HIR relationships | Exact ValueId/field/expression/source facts, declared consumption contexts, bounded deterministic traversal and paging, unchanged prior facet handles/payloads, and fail-closed unsafe Project admission; [HIR Relationships](SEMANTIC-IMAGE-HIR-RELATIONSHIPS-V1.md) owns authored/unrun cases |
| Candidate declaration, extraction and recovery | Exact one-function identity extension, namespace/effect/ownership admission, actual ValueId capture order and unsafe-boundary rejection, later edits and merge of introduced identities, complete canonical history replay, tampered/stale/capacity failures and unchanged source/read-only authority; [Declaration Change](PROJECT-DECLARATION-CHANGE-V1.md), [Extraction](PROJECT-EXTRACTION-V1.md), and [Recovery](PROJECT-CANDIDATE-RECOVERY-V1.md) own authored/unrun cases |
| Project manifest or carrier | Exact source-set authentication, Phase-A reuse, closure/admission checks, carrier replay, post-publication drift behavior |
| Windows owned npm publication | Opaque compiler-prepared six-file handoff; exact v8/v9/v10 inline/published equality; standalone CLI/library pre-effect rejection and full-host aliases; source drift and primary failures; held-parent/stage/inventory/byte authentication, no-clobber and post-settlement no-rollback; actual Windows Node consumers and unchanged Unix/older-profile routes. [Windows publication v1](WINDOWS-OWNED-NPM-PUBLICATION-V1.md) owns the authored, unrun gate. |
| Project profile admission | Exhaustive v1-v10 schema/profile dispatch, descriptor derive/replay equality, ordinary v9 load and Revision Store round trip, v9/v10 execution-envelope replay, exact earlier-profile bytes and diagnostics |
| Windows Project Revision Store | Explicit Windows-entry-v1 APIs/schema, unchanged ordinary v1 bytes, protected effective-SID/LocalSystem DACL and mutex authority, fixed-local-NTFS/alias/ADS/reparse/link admission, bounded held reads/inventory, exact retained-parent publication and settlement, provisioned-host physical fixtures, all admitted Project profile round trips, and no support promotion from skipped or unrun gates |
| Project agent transport | Closed method/parameter schemas, exact revision binding, pre/post held-input authentication, response framing boundaries, zero-write inventory, hostile replay, and byte-preserved earlier protocols |
| Native backend or ABI | C11 compilation at required optimization levels, descriptor/header agreement, runtime status and cleanup conformance |
| V10 inline String settlement | Real descriptor replay and native provider generation; strict O0/O2 allocation/free accounting, failure-slot poison, late-argument/callee/local/loop failures, clone/branch/pressure and mixed Bytes ownership, same-context reuse after failure, explicitly selected sanitizers, safe locked/offline Rust consumer, and earlier-provider preservation except the explicit internal-String correction below; ordinary C corrections have a separate gate and context-handle closure is not a physical-allocation proof |
| Ordinary/stdout inline String settlement and contents | Actual emitted-C O0/O2 allocation/free counts, normalized failure and poisoned out slots, parameter/temporary/provisional settlement, branches/loops/contracts/intrinsics and mixed Bytes, generic-instance runtime discovery, exact-length NUL/Unicode contents across admitted interpreter/native/Wasm value lanes, explicit sanitizers, String-free byte/budget preservation, unchanged v10/command/callable selectors, and Target Evidence/Evidence-v2 binding to current production C; no inferred ordinary Wasm settlement |
| Owned-data provider internal Strings | [Explicit v8/v9 correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md): actual descriptor replay/provider generation, physical allocation/free accounting beyond handle closure, strict O0/O2 and provisioned sanitizers, pre/post-call-commit and mixed Bytes failure settlement, exact NUL/Unicode values, poisoned output slots, same-context reuse, real generated locked/offline safe Rust consumer; String-free output/budget and existing KAT preservation, no activated Project admission widening; all new execution remains unrun |
| Wasm or JavaScript boundary | Structural Wasm validation, generated binding checks, Node execution, and browser/multi-engine evidence when claimed |
| Direct-Bytes browser boundary | Execute the provisioned [Owned Data Browser v1](../platform-tests/owned-data-browser-v1/README.md) on each selected engine: real package imports, exact fixture signatures/carrier, capacity and hostile-input rejection, calibrated pre-instantiation Wasm authentication and genuine failure recovery; missing prerequisites fail and authored cases alone grant no browser promotion |
| Owned-data boundary corrections | Zero payload snapshot allocation on complete-tuple rejection; intrinsic-brand/species hostility; exact UTF-8/input bounds; selected-call-path private-frame exclusion and failure-slot poison; live foreign-context/reincarnation handle rejection; 4,095/4,096/4,097 live-slot and serial-exhaustion/contention settlement; v1-v7 preservation and explicit v8-v10 artifact deltas, subject to the separately documented shared arithmetic correction below |
| Checked `usize` multiplication | [Shared Wasm correction](PORTABLE-INDEXED-BYTE-DATA-V1.md#checked-multiplication-correction): ordinary and aggregate routes, zero on both sides, maximum/exact/overflow boundaries, evaluated-left failure precedence, nested status-branch depth, actual staged-owner success and failure settlement, preserved failed output, and same-instance recovery across interpreter/native O0/O2/generated npm; affected Wasm/integrity bytes change intentionally, not schemas or native behavior |
| Owned npm invocation failure state | [Shared v8/v9/v10 contract](OWNED-NPM-INVOCATION-V1.md): seven real generated packages covering direct/variant/mixed/flat renderers; reusable preflight and authentic semantic failure; unexpected type/range/falsy throws, malformed statuses, forged semantic markers, caught reentry, post-consume UTF-8 failure, sticky primary under cleanup failure, no later engine/import/publication after poison, unchanged non-runtime artifacts and historical cryptographic pins. Actual generated-result decoding must reject corrupted tags/carriers/bools and modified failure slots with calibrated payload-read/consume observations, same-arena stale tokens, inactive-storage non-access and live-owner settlement disagreement; a test-local decoder or arena is not a substitute. |
| Doctor tool detection and subprocess lifetime | Real basename-sensitive multicall symlink, relative PATH and non-executable shadows, missing/failed tool exits, complete numeric version and suffix admission, unchanged report schemas/order, and physical exact/plus-one output, timeout, descendant, descriptor and fail-stop settlement fixtures on all supported hosts. Linux additionally requires actual BPF policy interpretation, direct/exec-descendant syscall denial, before/after unfiltered-host controls, actual filter-install rejection before executable entry, unsupported-ABI rejection and unchanged Command-descendant compatibility gates. The [lifecycle contract](DOCTOR-PROBE-V1.md) leaves complete discovery/filesystem/broker and cross-platform no-network isolation open. |
| Report or schema projection | Closed admission/exclusion vocabulary, deterministic envelope, independent replay, tamper and budget rejection, cross-report consistency |
| Offline package resolution | Strict SemVer/range boundaries, deterministic permutation and first-feasible backtracking, multi-root/transitive closure, conflict/duplicate/cycle rejection, exact bounds, ternary-target and capability policy, subject/report and outer-wire remint rejection, exact replay, and preserved Report/Lock/Compatibility bytes |
| Offline published semantic lock snapshot | Exact raw Subject-v2 preservation, catalog-permutation canonicalization, input/evidence/Lock cross-pair and remint rejection, checked component/cumulative bounds, two complete replays around held staging, exact fixed three-file inventory, no-replace publication, settlement/uncertainty/foreign-byte evidence, platform authority preconditions, and unchanged Resolver/Lock/build-v1/v2 bytes and diagnostics |
| Authenticated package ranges | Subject-v3 dependency order/self/grammar rejection, exact selected-version range binding, range intersection and rollback, numeric candidate ordering, catalog permutation, cycle/depth/edge/decision/work exact bounds, Lock-v3 and Resolver-v2 tamper/remint/cross-pair replay, and exact v1/v2 API/schema/byte/diagnostic preservation |
| Offline multi-package source capsule | Two-to-four-package success, exact resolver/subject/report replay, explicit-root and root-only export binding, exact typed-interface comparison ignoring display/parameter names, source-import versus dependency-graph equality, unreachable/provider/type/effect rejection, canonical-wire and every exact/+1 bound, tamper/cross-pair replay, and preserved Report/Lock/Resolver/build-v1 bytes |
| Offline linked scalar Wasm package build | Real two-package capsule-to-build replay, root-only export ownership, exact seven-import/export inventory, distinct v2 canonical manifest/evidence, mutation and cross-pair rejection, cumulative artifact/evidence and fixed-point boundaries, two compiler replays around held publication, exact three-file inventory, cleanup/uncertainty/post-publication authentication, and unchanged build-v1 bytes/order |
| Private host integration | Authority inventory, fail-stop uncertainty, process/loader settlement, platform-specific hosted jobs |
| Calculator project publication | [Owning contract](NEW-PROJECT-PUBLICATION-V1.md): unchanged template bytes and ordinary Project validation, relative/parent-relative success, post-rename held-parent/output and original-alias displacement, preserved original/foreign inventories after failure and drop, partial-stage residue rejection; forced Windows extended-to-legacy call with actual zero replacement field plus native success and collision preservation; all new physical gates remain unrun |
| Semantic Workspace Image | Execute the authored, unrun [Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md) exact replay, typed-index, stale/drift, capacity, deterministic cross-root, CLI and zero-write evidence before promotion |
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
