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

The script is the executable source of truth for the precise command sequence.
Do not copy that sequence into feature documents.

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
| Standalone Wasm internal String settlement | Distinct explicit profile, structural module validation, fixed memory and selected acyclic stack/owner bounds; independent raw mint/drop accounting and every reached mint-refusal path, generated-host exact/+1 quotas and poison/reentry, exact artifact/input binding, native O0/O2 and internal-String interpreter parity, legal scalar-loop helper reuse, unchanged U105/T252/J113 rejection and legacy artifact known answers; all new evidence remains unrun, with no ordinary-Wasm, peak-heap or trap-recovery inference |
| Standalone internal String Web package | Actual explicit-source CLI selection and pre-effect usage rejection; bounded source snapshot and final drift recheck; source/descriptor/package exact/+1 bounds; exact eight-file inventory, independent manifest/digest replay and direct compiler-output equality; deterministic repeat and stable-ID rename, hostile identities, fresh-parent publication and foreign-byte preservation; real generated Node, strict provisioned TypeScript and provisioned browser consumers including streamed fetch bounds; pre-effect legacy String rejection including materialized generic bodies, unchanged raw emission and String-free legacy bytes; new gates remain unrun with no support promotion |
| Prepared Project interpreter or source trace | One cached exact closure admission and one persistent worker across repeated entry/test execution; legacy outcome/fuel parity; cancellation boundaries; exact node/byte/event limits; deterministic truncation; canonical replay; retained-HIR source-origin binding; worker panic/disconnect fail-stop; and unchanged Interpreter/Project/Transport v1-v5 bytes |
| Prepared Project revision replacement | Exact expected-content revision before candidate preparation, both closures/origins swapped together, byte-identical old execution after stale or ordinary candidate rejection, new/old trace cross-binding, same worker and permit, unchanged ceilings/cancellation/admission, concurrent-operation rejection, and terminal panic or lost acknowledgement; no epoch, incremental-compiler, or peak-heap inference |
| Ownership or cleanup | Structural inventory, canonical plan build, independent replay, hostile mutation, success/failure settlement |
| Graph schema | Exact new projection, legacy byte preservation, context projection, invalid/tampered rejection |
| Semantic patch or repair | Preview, stale/drift rejection, no-write failures, independent replay, atomic A0 application |
| Workspace transaction | Held-input rechecks, replay before candidate/staging, one publication pivot, old-or-new process termination evidence |
| Project manifest or carrier | Exact source-set authentication, Phase-A reuse, closure/admission checks, carrier replay, post-publication drift behavior |
| Project profile admission | Exhaustive v1-v10 schema/profile dispatch, descriptor derive/replay equality, ordinary v9 load and Revision Store round trip, v9/v10 execution-envelope replay, exact earlier-profile bytes and diagnostics |
| Windows Project Revision Store | Explicit Windows-entry-v1 APIs/schema, unchanged ordinary v1 bytes, protected effective-SID/LocalSystem DACL and mutex authority, fixed-local-NTFS/alias/ADS/reparse/link admission, bounded held reads/inventory, exact retained-parent publication and settlement, provisioned-host physical fixtures, all admitted Project profile round trips, and no support promotion from skipped or unrun gates |
| Project agent transport | Closed method/parameter schemas, exact revision binding, pre/post held-input authentication, response framing boundaries, zero-write inventory, hostile replay, and byte-preserved earlier protocols |
| Native backend or ABI | C11 compilation at required optimization levels, descriptor/header agreement, runtime status and cleanup conformance |
| V10 inline String settlement | Real descriptor replay and native provider generation; strict O0/O2 allocation/free accounting, failure-slot poison, late-argument/callee/local/loop failures, clone/branch/pressure and mixed Bytes ownership, same-context reuse after failure, explicitly selected sanitizers, safe locked/offline Rust consumer, and frozen earlier-provider bytes; ordinary C corrections have a separate gate and context-handle closure is not a physical-allocation proof |
| Ordinary/stdout inline String settlement and contents | Actual emitted-C O0/O2 allocation/free counts, normalized failure and poisoned out slots, parameter/temporary/provisional settlement, branches/loops/contracts/intrinsics and mixed Bytes, generic-instance runtime discovery, exact-length NUL/Unicode contents across admitted interpreter/native/Wasm value lanes, explicit sanitizers, String-free byte/budget preservation, frozen command/provider outputs, and Target Evidence/Evidence-v2 binding to current production C; no inferred ordinary Wasm settlement |
| Wasm or JavaScript boundary | Structural Wasm validation, generated binding checks, Node execution, and browser/multi-engine evidence when claimed |
| Owned-data boundary corrections | Zero payload snapshot allocation on complete-tuple rejection; intrinsic-brand/species hostility; exact UTF-8/input bounds; selected-call-path private-frame exclusion and failure-slot poison; live foreign-context/reincarnation handle rejection; 4,095/4,096/4,097 live-slot and serial-exhaustion/contention settlement; v1-v7 preservation and explicit v8-v10 artifact deltas |
| Owned npm invocation failure state | [Shared v8/v9/v10 contract](OWNED-NPM-INVOCATION-V1.md): seven real generated packages covering direct/variant/mixed/flat renderers; reusable preflight and authentic semantic failure; unexpected type/range/falsy throws, malformed statuses, forged semantic markers, caught reentry, post-consume UTF-8 failure, sticky primary under cleanup failure, no later engine/import/publication after poison, unchanged non-runtime artifacts and historical cryptographic pins |
| Doctor tool detection and subprocess lifetime | Real basename-sensitive multicall symlink, relative PATH and non-executable shadows, missing/failed tool exits, complete numeric version and suffix admission, unchanged report schemas/order, and physical exact/plus-one output, timeout, descendant, descriptor and fail-stop settlement fixtures on all supported hosts; the [lifecycle contract](DOCTOR-PROBE-V1.md) does not establish network isolation |
| Report or schema projection | Closed admission/exclusion vocabulary, deterministic envelope, independent replay, tamper and budget rejection, cross-report consistency |
| Offline package resolution | Strict SemVer/range boundaries, deterministic permutation and first-feasible backtracking, multi-root/transitive closure, conflict/duplicate/cycle rejection, exact bounds, ternary-target and capability policy, subject/report and outer-wire remint rejection, exact replay, and preserved Report/Lock/Compatibility bytes |
| Offline published semantic lock snapshot | Exact raw Subject-v2 preservation, catalog-permutation canonicalization, input/evidence/Lock cross-pair and remint rejection, checked component/cumulative bounds, two complete replays around held staging, exact fixed three-file inventory, no-replace publication, settlement/uncertainty/foreign-byte evidence, platform authority preconditions, and unchanged Resolver/Lock/build-v1/v2 bytes and diagnostics |
| Authenticated package ranges | Subject-v3 dependency order/self/grammar rejection, exact selected-version range binding, range intersection and rollback, numeric candidate ordering, catalog permutation, cycle/depth/edge/decision/work exact bounds, Lock-v3 and Resolver-v2 tamper/remint/cross-pair replay, and exact v1/v2 API/schema/byte/diagnostic preservation |
| Offline multi-package source capsule | Two-to-four-package success, exact resolver/subject/report replay, explicit-root and root-only export binding, exact typed-interface comparison ignoring display/parameter names, source-import versus dependency-graph equality, unreachable/provider/type/effect rejection, canonical-wire and every exact/+1 bound, tamper/cross-pair replay, and preserved Report/Lock/Resolver/build-v1 bytes |
| Offline linked scalar Wasm package build | Real two-package capsule-to-build replay, root-only export ownership, exact seven-import/export inventory, distinct v2 canonical manifest/evidence, mutation and cross-pair rejection, cumulative artifact/evidence and fixed-point boundaries, two compiler replays around held publication, exact three-file inventory, cleanup/uncertainty/post-publication authentication, and unchanged build-v1 bytes/order |
| Private host integration | Authority inventory, fail-stop uncertainty, process/loader settlement, platform-specific hosted jobs |
| Public API or generated SDK | External consumer with no source/workspace dependency, locked offline build, inventory and compatibility checks |

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
