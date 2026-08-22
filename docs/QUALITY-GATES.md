# Quality gates

Semantic Workspace Operations v1 changes additionally require the focused
unit and public integration suites, exact derivation/Evidence/two-receipt KATs, canonical
proposal/wrapper mutation matrices, proposal/per-path input exact/+1 caps,
derivation individual and aggregate minimum-success/minus-one caps plus
greater-than-production seam rejection, one-base/one-candidate build counters,
lock-held one-read ownership,
resolver-free final-drift rejection with immediate exclusive reacquisition,
API/CLI byte parity and arity, no-write inventory proof, and preservation of
Change-v1 and Structural Change bytes. Operations Evidence additionally requires
embedded unchanged Change-v1 Evidence binding, canonical/ref/limit/budget/nonclaim
mutation matrices, replay-before-write, one exclusive lock, a sealed invocation-local
commit proof, dual final checks, the sole `ACTIVE` pivot, I211/I212 boundaries,
state-relative residue, process-termination, and OS no-clobber evidence. Hosted claims require the exact-head
Ubuntu, macOS, Windows, MSRV, Component, and dependency-policy matrix.
The decisive Operations process-termination gate is
`cargo test --locked -p semaprax --lib semantic_workspace_operations::tests::operations_apply_killed_process_boundaries_preserve_exact_old_or_new -- --exact --nocapture`
on Ubuntu, macOS, and Windows. It proves the exact old-or-new authenticated
workspace state after real child-process termination while the exclusive lock
is held; it does not establish power-loss durability.
The exact `dfc04278c6ba9a7dd247d4cc4add3af91f55b936` matrix is hosted green in
[run 31570834457](https://github.com/wavect/semaprax/actions/runs/31570834457);
all 12 jobs passed, including the Operations process-termination gate on
Ubuntu, macOS, and Windows. Current totals remain 39 Partial/17 Missing.

Bounded Native Agent Runtime v1 additionally requires
`cargo test --locked -p semaprax --lib agent_runtime::tests` with deterministic
fake hosts on Ubuntu, macOS, and Windows. Its gate covers canonical Profile,
Task, Action, Trace, and Evidence bytes; exact routing and retry rules;
incremental provider/tool sinks; per-boundary cancellation, policy, and budget
checks; deep replay; secret-sentinel absence; no-write inventory; long-ID and
escaping boundaries; and cumulative builder accounting. It makes no live
provider, transport, quality, public API/CLI, wallet, or economic-authority
claim. C1 additionally requires an external-crate host, exact public surface and
opacity locks, cancellation/retry/cap/secret/no-write checks, package/rustdoc,
and an explicit Ubuntu/macOS/Windows public integration gate. See [Bounded Native
Agent Runtime v1](AGENT-RUNTIME-V1.md). Public Agent Runtime v1 is hosted GREEN at 8cf29aff8d1be3ccf74c36bc8c837f0c666ca067 (run 31591039261, 12/12 jobs, private and public deterministic fake-host gates on Ubuntu, macOS, and Windows).
Private Economic Agent v1 additionally configures `cargo test --locked -p semaprax --lib economic_agent::tests -- --nocapture` and `cargo test --locked -p semaprax --lib economic_agent::tests::economic_process_kill_markers_never_repeat_sign_or_broadcast -- --exact --nocapture` on Ubuntu, macOS, and Windows. Private Economic Agent v1 A+B is exact-head hosted green at fe75c38d898b71e3ed5c57411fb46d0dbd4fc34b in run 31611748969, including both Economic gates on Ubuntu, macOS, and Windows. Public Economic Agent v1 C is exact-head hosted green at 03f1f2736de23d03b298f265f93409de89a6be95 in run 31616168124 (12/12 jobs), including the private, process-termination, and public Economic gates on Ubuntu, macOS, and Windows.
Public C additionally runs `cargo test --locked -p semaprax --test economic_agent_v1 -- --nocapture` on Ubuntu, macOS, and Windows.
Current totals remain 39 Partial/17 Missing.

Private Native Rust Interoperability v1 A+B additionally requires three named
gates on Ubuntu, macOS, and Windows:

```sh
cargo test --locked -p semaprax --test native_rust_interop_v1 -- --nocapture
cargo test --locked -p semaprax --test native_rust_interop_ci_contract -- --nocapture
cargo test --locked -p semaprax-native-rust-interop -- --nocapture
cargo test --locked -p semaprax-native-rust-interop-platform --all-targets -- --nocapture
```

The first gate freezes the additive syntax, distinct HIR call kind, exact
diagnostics, and explicit Graph/Wasm exclusions. The source-contract gate keeps
the private crates unpublished and quarantined and rejects a public builder
surface. The private builder suite must independently replay the canonical
Spec, Descriptor, generated sources, and Manifest; reject every-byte
substitution, deletion, insertion, and truncation; freeze byte length and raw
SHA-256 for Descriptor, Manifest, header, C, safe Rust, and private FFI plus the
protocol-domain digests for Descriptor and Manifest; prove the cumulative builder
cap with named pre-HIR/post-HIR retained-versus-scratch high waters, iterative
render/replay traversal, exact persistent transfers, no-growth final sinks, and
minimum-minus-one zero-entry rejection; prove one fixed-capacity aggregate
`rustc -vV` parse and every prepared invocation without geometric growth; prove
the create-new inventory; compile and statically link generated C and Rust
at both `-O0` and `-O2`; and execute
Rust-to-SEMAPRAX, SEMAPRAX-to-Rust, and round-trip success/failure cases. Its
hostile corpus covers ABI/version/size/alignment/bool/status/capability/result
publication, panic containment, same-thread/non-reentrant use, depth and call
budgets, tool and artifact substitution, race hooks, and safe-facade opacity.
The source-contract gate must read the exact `implementation/artifacts.rs` and
`implementation/exact_replay.rs` paths, reject filesystem/process/platform and
publication authority in both, retain separate generator and replay frame
families, and prove the independent C replay never calls generator traversal.
The platform gate must prove held directory/file/executable identity,
no-clobber publication, reparse/symlink rejection, empty child environment,
ambient FD/handle closure, bounded output kill-and-reap, and the corresponding
Windows handle/job/process behavior rather than treating an unsupported stub as
Windows evidence. Windows must also cover zero/small stdout EOF, silent timeout,
descendant-held stdout without overflow, exact reserved/case-folded name
handling, and injected image/assign/resume/terminate/wait/query/peek/read
failures. Ordinary errors publish their sticky code only after proven leader and
Job quiescence; settlement-proof failure fail-stops before later build or
publication actions. Every created build stage remains bound to its create-returned
directory authority through settlement. Success and every failure path attempt
one exact-inventory cleanup; identity, reparse/symlink, or inventory mismatch
must stop cleanup, preserve the foreign sentinel, and leave only inert residue.
Neither the facade nor its system quarantine may expose generic or recursive
path deletion. Linux additionally requires the generated boundary under Clang
ASan+UBSan. Package/source locks must keep the three unpublished private crates
out of the public `semaprax` package, reject dynamic loading/link lookup, and
preserve existing Graph, Wasm, callable-v2/v3, Agent, Economic, Workspace,
Patch, CLI, API, and KAT bytes. Exact-head hosted promotion requires the whole
matrix; one host, a compile-only lane, or a platform-disabled test is not
evidence for the other hosts. Qualifying local runs set explicit absolute
`RUSTC` and `CLANG`; Windows additionally freezes an absolute Visual C++ tools
root plus its exact `SEMAPRAX_LINKER`, prepares
`-Xmicrosoft-visualc-tools-root <root> -fuse-ld=link`, and must not export
ambient `PATH` into the process arena. An ambient launcher or proxy is
deliberately rejected and is not a regression in the direct-image policy.
The private A+B gate is exact-head hosted green at
`50b96dccabe3b3dcbcdf38bab380f3eb8699184c` in [run
32402944574](https://github.com/wavect/semaprax/actions/runs/32402944574),
including Ubuntu, macOS, Windows, Rust 1.85, Linux sanitizer, and the hosted
Windows process/capacity cases. That private gate alone did not promote Public
C; the additive Phase C builder API and generated local package require the
separate gates below.

Public Native Rust SDK v1 Phase C additionally requires these named gates on
Ubuntu, macOS, and Windows:

```sh
cargo check --locked --offline --manifest-path examples/calculator-rust/Cargo.toml
cargo test --locked -p semaprax --test public_native_rust_sdk_ci_contract -- --nocapture
SEMAPRAX_REQUIRE_PUBLIC_NATIVE_RUST_SDK=1 cargo test --locked -p semaprax --test public_native_rust_sdk_v1 -- --test-threads=1 --nocapture
```

The effectful gate must use explicit held `RUSTC`, `CLANG`, and
`SEMAPRAX_ARCHIVER` images. Windows additionally binds the same exact
`SEMAPRAX_VCTOOLS` root and `SEMAPRAX_LINKER` used by private B. Phase C must
preserve every private A+B byte while publishing exactly one fresh local
dependency-free generated Cargo package with the nine-file inventory frozen in
[Native Rust Interoperability v1](NATIVE-RUST-INTEROP-V1.md). The gate requires
independent outer-manifest replay, byte-identical double builds, exact archive
member replay, a compiler-free calculator consumer, the Rust-import callback
round trip, stable-ID facade spelling and display-rename preservation, and one
same-source Rust/native-C/Core-Wasm result. It also retains the private hostile,
sanitizer, platform settlement, and Windows process gates above. The package is
current-host scalar evidence only: registry publication, cross-target reuse,
aggregate/resource/string/pointer ABI, async, cross-thread use, dynamic loading,
and a Phase-C pre-reserved cumulative-memory or allocation-failure-recovery
proof remain held. Hosted promotion requires the exact-head three-OS SDK job;
local success alone is not promotion evidence.

Public Wasm Scalar Exports v1 additionally requires focused admission,
emission, package, CLI, and consumer gates. The profile must admit only 1–32
distinct explicit stable IDs under the exact lowercase portable 128-byte
grammar; at most eight by-value `i64`/`bool` parameters and an `i64`/`bool`
result; and a completely monomorphic, effect-free program without permits,
imports/interfaces, authored aggregates/resources/variants, generic templates
or instances, callbacks, or async. Every excluded shape must reject with exact
`SPX-W115`/`SPX-W116` evidence before output creation; there is no aggregate or
legacy ABI fallback.

The emitted module must contain only the selected stable-ID-derived scalar
adapters, with no `semaprax_main`, memory, unselected function, or owned
adapter export. Require canonical boolean input/output rejection; signed-range
BigInt and Boolean conversion; exact eight-case arithmetic and two-case
contract statuses; unknown traps remaining out of band; frozen null-prototype
bindings; exact generated declarations; deterministic bytewise stable-ID
ordering; stable symbols when another export is inserted or reordered; and an
actual stable-ID semantic rename proving unchanged external API and behavior.

The fresh destination must reject existing files, directories, and symlinks,
and successful output must have the exact seven-file inventory in [the profile
contract](WASM-SCALAR-EXPORTS-V1.md). Require canonical `semaprax.web.v4`
manifest replay, exact artifact SHA-256 bindings, mutation rejection before
Wasm instantiation, byte-identical double builds, legacy web-v3 byte
preservation, strict repeated-`--export` CLI parsing, Node execution,
native/Wasm scalar status equivalence, formatting, strict Clippy, Rust 1.85,
package and source locks, and independent security review. A pinned
`tsc --strict --noEmit` external consumer and a hosted real-browser calculator
interaction remain mandatory before claiming independently validated
TypeScript or general browser-SDK compatibility. The full exact-head hosted
matrix remains mandatory for promotion.

Project Manifest v1 additionally requires exact six-assignment canonical TOML
and bounds, explicit-source/root/ancestor held-identity authentication and
final drift recheck, no created managed workspace/control directory/cache,
complete-set scalar admission (including disconnected modules), exact
entry/test provider closures with real bodies and no stubs/synthetic mains,
stable-ID duplicate-display-name linkage, cleanup-plan rebuild plus final HIR
validation, internal native O0/O2 linked-equivalence evidence, deterministic exact
seven-file Web package and digest replay, Node execution, stable-ID display
rename preservation, CLI manifest/default/flag rejection, and preservation of
single-file and managed-Workspace behavior. Interface declarations plus
interface/native imports and `use type` edges are excluded; explicit stable-ID
`use function` provider edges are the sole cross-file composition mechanism.
Public Project Native Publication v1 additionally requires explicit
create-new `--target native` admission, compilation of exactly the linked entry
HIR through the shared Clang C11 boundary, linked-entry execution and replay,
pre-publication drift rejection before any output exists, post-publication
`SPX-J103` uncertainty that preserves the executable, existing-destination
`SPX-I307` rejection without clobber, deterministic entry C projections,
stable-ID display rename preservation of published native behavior, unchanged
Web-package bytes, and unchanged single-file/Workspace/Patch evidence. Focused
local gates are:

```sh
cargo test --locked -p semaprax --all-features --lib project::tests::
cargo test --locked -p semaprax --all-features --test project_cli_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test project_native_publication_v1 -- --test-threads=1
cargo test --locked -p semaprax --test project_manifest_v1
cargo test --locked -p semaprax --test project_backend_equivalence_v1 -- --test-threads=1
```

Project CLI publication is Web (default) or explicit native; public project
`run` and a public project test command remain held. The dedicated hosted
`project-v1` job remains manifest/CLI-protocol-level; native publication
evidence runs in the workspace verify matrix that resolves Clang on every host.
Exact-head hosted promotion is pending. A post-publication final-input drift is
`SPX-J103`: the complete retained output remains for caller reconciliation and
is never deleted automatically. Project v1 grants no general packages,
dependencies, registries, capability grants, interface/native imports, `use
type` edges, generics, resources, test discovery, hostile-window no-clobber
native publication, or cross-build executable byte-determinism claim.

Quality gates are executable evidence, not a checklist substitute for reasoning. Every pull request must pass the baseline and the gates for each changed semantic layer.

## Baseline

Run from the repository root:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo build --locked --workspace --release
cargo package --locked --allow-dirty -p semaprax
cargo run --locked -p semaprax -- check examples/meaning.spx
cargo run --locked -p semaprax -- check examples/ownership.spx
cargo run --locked -p semaprax -- check examples/lifecycle.spx
cargo run --locked -p semaprax -- check examples/control_flow.spx
cargo run --locked -p semaprax -- check examples/records.spx
cargo run --locked -p semaprax -- check examples/native_callable.spx
cargo run --locked -p semaprax -- fmt examples/meaning.spx --check
cargo run --locked -p semaprax -- fmt examples/effects.spx --check
cargo run --locked -p semaprax -- fmt examples/ownership.spx --check
cargo run --locked -p semaprax -- fmt examples/lifecycle.spx --check
cargo run --locked -p semaprax -- fmt examples/control_flow.spx --check
cargo run --locked -p semaprax -- fmt examples/records.spx --check
cargo run --locked -p semaprax -- fmt examples/native_callable.spx --check
```

`scripts/quality.sh` runs this baseline on Unix. Tests and documentation always
enable every workspace feature so an internal or staged production surface
cannot escape execution merely because it is not a default feature. The
integration suite also discovers every committed `.spx` example and requires it
to verify and exactly equal its canonical projection. CI runs the equivalent
matrix on Linux, macOS, and Windows, plus an explicit Rust 1.85 minimum-version
check with the same all-feature policy.

`scripts/quality.sh quick` is advisory and runs diff, format, workspace check,
documentation/example, economics, and routing feedback. `changed` reconciles
an exact ancestor-base-to-HEAD committed diff plus the complete
staged/unstaged/untracked/deleted/rename Git state. The base is explicit or the
unique `--all` merge base with an explicitly configured/default remote target,
never the current branch upstream; missing/non-UTF-8/ambiguous bases and
incomplete optional explicit path sets reject. It uses the closed, versioned gate plan
from `scripts/quality-route.sh`; its
only narrow mappings are documentation and agent-context/economics changes.
Missing or unmapped paths, router changes, and every broader semantic/platform
path run `full`. No argument remains the complete local `full` baseline. None
of these local profiles substitutes for required hosted platform or sanitizer
evidence. See [agent context economics v1](AGENT-ECONOMICS-V1.md).

CI also runs an immutable-SHA-pinned `cargo-deny` policy over the complete locked
target graph. Advisories, unapproved licenses, duplicate/wildcard versions, Git
dependencies, and registries other than crates.io are blocking failures;
`deny.toml` contains no advisory exceptions.

On PowerShell, run the documentation gate as:

```powershell
$env:RUSTDOCFLAGS = '-D warnings'
cargo doc --locked --workspace --all-features --no-deps
Remove-Item Env:RUSTDOCFLAGS
```

The remaining Cargo and `semaprax` commands are shell-neutral.

## Change-specific gates

Callable-v3 wire changes require all seven independent compiler/host codecs,
exact 20-byte envelopes, frozen capacities (`104 + arguments`,
`156 + 4*events`, `388 + 12*resources`, `172`, `196`,
`372 + 12*resources`, and host-only `524`), closed tags, request/response/
decision/action/frame/candidate digest known answers, the distinct receipt-key
HMAC KAT, every-prefix/trailing/every-byte mutations, cross-binding and replay
rejection, and unchanged v1/v2/proof confusion gates. Codec evidence alone
grants none of the adjacent runtime claims. The current private physical tranche
additionally requires all-14-scenario provider-only `-O0`/`-O2` execution,
root-image loader provenance, exact one-fill receipt-key/instance-binding
derivation, atomic ledger/replay tests, and the all-14-scenario joint provider ->
loader -> host test at `-O0`/`-O2`. The joint test must count zero Rust global-
allocator allocation/reallocation calls from immediately before `CallCommit`
through authenticated `ReceiptCommit`; an injected reusable-decode reserve
failure must quarantine exact evidence and the leaf pin. Provider failure gates
must cover returned physical failure, malformed response/frame/candidate,
durable finalizer interruption, exact replay, and conflicting-decision
nonmutation at `-O0`/`-O2` and under ASan+UBSan. Pre-execute unwind additionally
requires tag-3/sentinel/zero-storage KATs, proof that execute is not entered,
certified abort settlement, authenticated host receipt, and unchanged legacy
v1/v2/proof evidence. Documentation must still deny exhaustive fatal-allocator/
process-crash evidence, quiescence, malicious-code containment, public
admission, and any `SPX-B104` change. Bounded Android runtime evidence is
accepted only from the dedicated hosted Emulator job. That job must compile the loader,
host, and target-bound provider for x86_64 and arm64 with pinned NDK r27.2;
prove the exact Bionic/ELF guards and both ELF architectures; retain exact
`libloading 0.8.9`; execute the x86_64 O0/O2 providers from canonical paths in
an API-35 emulator; and require exact finalizers, receipt/ledger transition,
healthy host state, and zero measured Rust allocation across the irreversible
interval. JNI/Kotlin, APK/AAR, lifecycle/UI, device, broader corpus, and public
admission remain separate gates.

The Windows mixed-inventory directory-publication gate is an explicitly
non-blocking diagnostic while one defect stays open: on current GitHub
Windows runners, renaming a staged directory returns `STATUS_ACCESS_DENIED`
through `NtSetInformationFile(FileRenameInformationEx)` with POSIX semantics,
through the legacy fallback, and through plain `MoveFileExW` whenever any
descendant was opened through the held-handle authority — even after every
handle closes and even when the descendant is reopened through the CRT —
while empty siblings and never-opened trees rename normally. The brepro
archive admission gate remains mandatory and green. The mixed-inventory test
stays in-tree behind `#[ignore]` carrying its probe evidence; hosted
promotion of native-publication lanes remains held until this is
root-caused and the gate returns to mandatory blocking status.

[Run 31320436726, job
93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
is the first green Android execution of that exact contract.

The private [Android JNI ownership adapter
v1](ANDROID-JNI-OWNERSHIP-V1.md) has a distinct application gate. It must use
the pinned NDK 27.2.12479018, API/target 35, minSdk 28, build-tools 35.0.0,
runner Kotlin 2, and Gradle 9; Gradle runs `--offline`, applies no plugin, and
declares no repository. Before installation, the gate compiles x86_64 and arm64
JNI/provider sources with strict C warnings, checks both ELF architectures,
requires `JNI_OnLoad` as the shim's sole defined global export, enforces the
Android system-library dependency allowlist and no RPATH/RUNPATH/workspace path,
and verifies that the APK contains exactly the x86_64 JNI plus O0/O2 provider
libraries. It uninstalls any prior fixture package, installs the no-UI APK,
runs same-package framework Instrumentation on API-35 x86_64, and exact-matches
`files/semaprax-android-jni-v1.txt` through `run-as`.

Runtime assertions must cover independent Kotlin/native `SPXAJH01` and
`SPXAJS01` known answers, explicit `consume()`, O2 Cleaner dispatch, a
consume-versus-Cleaner one-winner race, stale/forged/cross-runtime/wrong-thread/
reentrant rejection with poisoned outputs untouched, declared and unexpected
JVM exception normalization with no pending exception, exact finalizers
`1:13,0:11`, no-owned publication, zero measured Rust postcommit allocations,
healthy host state, and an empty outer handle table after the drain barrier.
Cleaner evidence must call the identical registered action through
`cleanForTest()`/`cleanable.clean()`; nondeterministic GC/enqueue observation,
wrapper collection, and process-exit cleanup do not count. Local Rust/C tests,
packaging checks, and CI source locks prove implementation/configuration only.
APK runtime evidence counts only from the dedicated hosted job; the exact gate
is green in [run 31338834586, job
93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).

Current admitted hosted evidence is exact and bounded: Portable Result
Component v3 is green in [run 31338834586, job
93309086213](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086213),
the private macOS engine plus AppKit package/runtime in [job
93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230),
the Swift/iOS app plus XCFramework in [job
93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228),
and the Android JNI/Kotlin app in [job
93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
The Windows engine and Win32 UI package/runtime gates are green in [run
31343897595, job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480),
including the strict PE inspection path. That full hosted matrix also executes
the bounded Copy Variants + Match v1 tests on Linux, macOS, and Windows.
The generic/prelude matrix and isolated prelude-bound Wasmtime runner are
hosted green in [run 31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201),
with the runner's exact job at [93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).

| Change | Required evidence |
| --- | --- |
| Syntax or AST | Canonical parse-format-parse round trip and malformed-input diagnostic; record update must preserve base-first and authored replacement order; variant declarations, qualified construction, unit `{}` syntax, and match patterns/arms must preserve declaration and authored order |
| Types or ownership | Positive test, compile-fail diagnostic code, prefix/sibling behavior, divergent-branch join, borrow/shared transfer boundary, and hostile-HIR replay |
| Resolved HIR | Stable-ID rename/whitespace invariance; unique lexical/result/place/member/case/pattern-binding IDs; recursive type-fact/layout assertions; record constructor/projection/update and variant constructor/exhaustive-match integrity and ordering; invalid-AST rejection; move/effect/contract revalidation; and malformed-HIR native/Wasm rejection parity |
| Aggregate layout | Exact Native64/Wasm32 size/alignment/offset KATs for nested admitted fields; frozen one-byte/alignment-one empty records with explicit C padding and validated Wasm bytes; checked arithmetic; stable declaration-order field identity; reorder/overlap/undersize/misalignment/overflow mutation rejection; and no inference of a public ABI from an internal profile |
| Copy-variant layout | Exact Native64/Wasm32 digest, size/alignment/tag/payload/field-offset KATs; explicit declaration-order `u32` tags; one-byte empty payload; target-specific bool representation; independently reconstructed checked arithmetic; and hostile reordered/duplicate/noncanonical-tag/overlap/undersize/misalignment/target-confusion rejection. Internal layouts are not a stable public ABI |
| Cleanup inventory | Exact recomputation from validated HIR, own-versus-borrow/shared entry evidence, nested field/lifecycle/flag assertions, deterministic storage discovery, and hostile inventory rejection before native/Wasm feature gates |
| Cleanup plan | Independent canonical CleanupPlan v2 rebuild plus attached-plan structural and all-path state replay after core-HIR/inventory validation; scalar and owned result commits; every checked/contract failure source; branch/lazy/region exits; copy-variant branches bound to exact scrutinee and stable case ID; typed-`Result` body/residual staging bound to exact Try/operand IDs, source/target instances, compiler-owned members, and one shared postcondition/publication join; caller-owned argument epochs and atomic commit; partial/nested record order; immutable-update base-first/authored-replacement order, untouched-field transfer, displaced-field reverse exact-once cleanup; deterministic Graph snapshot; scenario-driven exact semantic trace; and hostile plan rejection as `SPX-H006` before native/Wasm feature gates |
| Status, conformance trace, or trace-path certificate | Reserved compiler-status forgery rejection, nonzero imported codes, exact 1–255 UTF-8-byte/no-NUL domain boundaries across source/HIR/targets, context/arena token isolation, immutable one-based records, deterministic exact JSON, nested write-once failure selection, success-only finalizer completion, no physical target data, missing/extra scenario rejection, canonical certificate/fingerprint known answers, exact accepted outcome paths, allocation-free host DFA walking, omitted/duplicate/reordered event rejection, and an explicit migration note |
| Effects or contracts | Capability/effect rejection and runtime/backend behavior |
| Graph schema or agent context | Exact schema assertion/snapshot, canonical SHA-256 known answer, resolved-reference integrity, stable-ID collision behavior, owner/index generic-parameter identity, exact concrete argument trees, compiler-prelude schema/digest authentication, canonical-source-plus-prelude revision binding, exact byte/node accounting, ordered truncation reasons and omitted counts, re-expandable frontier replay, exact supported/unavailable filters, deterministic replay, and unknown/duplicate/malformed CLI rejection. Agent Context v2 additionally requires exact forward/reverse/both closure, an independently constructed caller index, global stable-ID order at each breadth-first depth, minimum-depth cycle/direction handling, exact `calls`/`called_by` relations, disjoint traversal and reference frontiers with honest independent counts, direction-bound replay, byte/node/depth truncation, permanent-unavailability closure, generic-template callers, all Graph v10-v14 source-schema selections, exact forward/reverse/both KATs, and byte/API/CLI golden preservation when direction is absent. Local v2 and legacy-v1 gates are 8/8 and 8/8, and the full hosted matrix is green in [run 31397881268, Ubuntu job 93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327). Graph v14 additionally requires program-wide precedence over v13/v12/v11/v10 for any authenticated generic function declaration, exact template/instance/call-instance identity and ordered arguments, no fabricated unused instance, mixed legacy-root context binding, domain-confusion rejection, independently parsed valid JSON, frozen module/Agent Context/bounded-context KATs, and byte-identical v10-v13 output when the feature is absent. The corrected array-delimited v14 projections are hosted green in [run 31390043736, Ubuntu job 93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296) |
| Agent economics or quality routing | Strict canonical, exact-case, separator-normal, Windows-forbidden/reserved-name-safe, non-symlink offline manifest/source containment; unsupported-facet rejection; exact manifest/context SHA-256 plus relevant/evidence ID bindings and goldens; byte/node/non-model lexical-unit accounting; reviewed relevance/evidence scoring; source mutation and deterministic replay; independent JSON parse; quick advisory boundary; explicit or unique target-remote `merge-base --all` ancestor-to-HEAD plus dirty Git-state reconciliation with non-UTF-8/zero/multiple-base rejection and optional exact path-set proof; hostile alias rejection; broad graph/CLI and unknown/wide/router paths escalating to full; exact profile-specific ordered gate-plan validation; and executed missing/duplicate/reordered/wrong-profile dispatcher rejection |
| Public protocol or schema | Compatibility fixture, or explicit version bump with migration and changelog note |
| Semantic patch | Successful atomic edit and returned-revision equality; syntax-position collision, stale SHA, legacy-token, and failed-edit no-change proof; canonical regular non-symlink source authentication; parent-alias lock convergence; create-new lock contention without foreign deletion; bounded create-new stage collision/exhaustion; exact source identity/bytes/revision recheck and exact stage path/handle identity/bytes recheck at both final commit boundaries; permission preservation and stage sync; concurrent edit, same-byte source replacement, staged-byte mutation, foreign stage-path replacement, injected rename failure, planted source/stage/lock symlink, nonregular source, owned-artifact cleanup, serial and parallel integration evidence. Unix device/inode identity is exact. Windows must hold same-file handles and compare volume plus the available 64-bit file index, while explicitly denying uniqueness on ReFS 128-bit or hostile non-unique-index environments. Keep predictable-name collision/stale-lock DoS, crash-left locks, non-cooperating mutation in the trusted final portable path window, parent-directory sync/power-loss durability, multi-file commits, and general typed repair/impact as nonclaims. Focused commands: `cargo test --locked -p semaprax --all-features --lib patch::tests::`, `cargo test --locked -p semaprax --all-features --test patch -- --test-threads=1`, and `cargo test --locked -p semaprax --all-features --test patch` |
| Semantic workspace transaction v1 | Exact canonical path-set/root/manifest/workspace-patch/snapshot/preview schemas, complete top-level and nested key order, API-no-LF/CLI-one-LF equality, exact digest domains/framing and five frozen KATs; 2–16 sorted portable lowercase paths; existing per-file Patch v1/v2/sole-v3 preservation; aggregate source/operation/parsed-AST/render/inventory bounds before multiplied HIR work; authored module/declaration identity uniqueness excluding compiler-owned/prelude IDs; canonical original sources; authenticated root/control/LOCK/ACTIVE/generation/staging/manifest/files/nested directories; real regular/non-symlink/non-reparse/link-count-one objects; held path/handle equality, distinct identities, one volume, exact directory trie/inventory; real shared/shared and shared/exclusive/exclusive lock behavior plus process-death release; initialization generation-zero/ACTIVE ordering; immutable candidate publication no-replace; two final apply rechecks; permission preservation; old-or-new cooperating-reader snapshots across only the ACTIVE pivot; exact stale second apply; every pre-pivot hook/race/limit preserves old ACTIVE, owned residue remains authenticated/bounded, and foreign replacements are preserved and fail closed rather than deleted; post-pivot `SPX-I212` ambiguity; Unix device/inode and bounded Windows volume/64-bit-index nonclaim; no raw-source write/atomicity, cross-file semantics, repository analysis, create/delete/move/materialization, evidence/provenance/approval, automatic recovery/GC, network/NFS/overlay, power-loss, ACL/xattr/ADS, external compatibility, or new Patch/Graph/Cleanup/backend/runtime meaning. Focused commands: `cargo test --locked -p semaprax --all-features --test semantic_workspace_transaction_v1 -- --test-threads=1`, `cargo test --locked -p semaprax --all-features --test semantic_workspace_transaction_v1_hostile -- --test-threads=1`, and `cargo test --locked -p semaprax --all-features --lib workspace::tests` |
| Semantic Workspace Patch Evidence v1 | Exact canonical capsule/receipt schemas and every top-level/file/nested/limits/budget/nonclaim key order; homogeneous v1/v2/v3 plus mixed capsule/receipt KATs; exact Workspace Patch, no-LF preview, per-file Graph/source/Patch/Review/seven assessments/supporting evidence/child Evidence-v1 bindings; sorted path correlation; existing digest-domain framing plus exact new preview/artifact domains; exact/one-over child and aggregate caps, fixed-point LF-inclusive artifact sizes, JSON depth, Graph v10–v14, schema/kind/receipt substitutions, and `SPX-G160`–`G163`/`I213`; shared-lock generation/verification, exclusive-lock-first apply precedence, one owned Patch/evidence read, immutable untrusted submitted bytes until exact replay, shared plan/preflight reuse, exact replay before candidate/staging creation, unchanged sealed Workspace two-final-check/`ACTIVE` pivot, stale/no-write/raw-source preservation, and ordinary Workspace parity. The Workspace aggregate source/AST caps bound HIR/index work before children; remaining Impact/Review/child output budgets govern closure/serialization only. Require all 21 ordered nonclaims, unchanged child Evidence-v1 `no_multi_file_transaction`, no Target/Evidence-v2 aggregation, no authority/provenance/approval/target-test/cross-file reasoning/raw-tree/durability claim, exact 39 Partial/17 Missing preservation, and a fresh exact-head hosted matrix before promotion. Focused commands: `cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1 -- --test-threads=1`, `cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1_apply`, `cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1_hostile`, and `cargo test --locked -p semaprax --all-features --lib workspace_patch_evidence::tests` |
| Semantic Workspace + Workspace Semantic Graph v1 | Exact semantic path-set/root/manifest and Workspace Graph schemas, key order, LF framing, source/workspace/artifact digest domains, literal KATs, graph public getter parity, full managed-set one-build work accounting, entry-provider projection, six independently replayed typed edge families, compiler prelude once, exact identities/depths/limits/nonclaims, shared-lock lifetime through render/final recheck/checked unlock, and ordinary Workspace mode rejection/preservation. Lock the exact pure expected-projection module path, its narrow parent-only facade, independent call-edge replay, deterministic edge ordering, shared root builder reservation/append/call-walk helpers, non-duplication, and absence of filesystem, process, workspace, publication, or mutation authority. Require no raw-source write, stage/apply authority, target/test execution, implicit imports, create/delete/move/materialization, recovery/GC, or durability claim. Focused commands: `cargo test --locked -p semaprax --all-features --lib semantic_workspace::tests`, `cargo test --locked -p semaprax --all-features --lib workspace_graph::tests`, and `cargo test --locked -p semaprax --all-features --test workspace_semantic_graph_v1` |
| Workspace Analysis v1 | Exact Context/Impact/Review schemas, digest domains, key order, KATs, API-no-LF/CLI-one-LF parity, selector/option grammar, typed module/declaration/capability namespace separation, independently replayed adjacency, minimum-depth/tied-direction traversal, exact Workspace-edge order, maximal bounded prefixes and first-depth frontier, unique omitted/deferred counts, reserve-first output bounds, one cumulative analysis-builder budget, complete nontruncated Review children, seven exact findings/evidence references/nonclaims, and the same held semantic authority through render/final recheck/unlock. Keep patch delta, repair, target/test, approval, persistence, and write authority closed. Focused commands: `cargo test --locked -p semaprax --all-features --lib workspace_analysis::tests` and `cargo test --locked -p semaprax --all-features --test workspace_semantic_graph_v1` |
| Semantic Workspace Change v1 C1/C2/C3 | Exact proposal, Preview/Context/Impact/Review/Evidence, verification receipt, and application receipt schemas; every nested key/order/domain/LF/limit/budget/nonclaim; seven whole-document KATs; 2–16 replacements-only unchanged-path-set admission; one base plus one candidate unified build; full managed-graph delta including disconnected changed modules; shared generate/verify and exclusive apply input precedence; proposal and Evidence each owned once; strict Evidence parse before proposal parse; exact typed and byte replay before writes; C2 receipt confusion rejection and no token authority; no-clobber immutable candidate create/reuse; both complete final-check matrices; sole `ACTIVE` pivot; `I211` before and `I212` only after successful rename; terminal structural held-object/permission/inventory authentication; state-relative residue/G187 and regenerated exact reuse without strategy claims; no raw-source changes; and public 10/10 plus private 11/11 local evidence. Hosted promotion additionally requires exact-head Ubuntu/macOS/Windows process-termination boundaries, Windows junction/reparse/hardlink/readonly/open-handle behavior, Unix no-clobber/symlink/hardlink behavior, cooperative readers, MSRV, Component, and dependency-policy jobs. Process termination must remain explicitly distinct from power-loss durability. Focused commands: `cargo test --locked -p semaprax --all-features --test semantic_workspace_change_v1` and `cargo test --locked -p semaprax --all-features --lib semantic_workspace_change::tests` |
| Semantic impact preview | Exact canonical `semaprax.semantic-impact.v1` top-level, nested-object, operation-variant, and change-variant key order plus SHA-256 KAT; Patch v1/v2 grammar and apply-candidate parity; domain-separated exact processed-patch-byte digest; operation/change provenance and stable ordering; closed source-consumer kind-then-ID and declaration-before-reference role order; grouped call changes; explicit persistent call owner; requires/body/ensures indexing; complete finite breadth-first reverse closure before output limits, with stable-ID order and minimum-depth provenance; mandatory nontruncating operations/changes/source-consumer facts; honest whole-JSON byte/node/depth accounting, affected-function-only prefix selection, exact `used_nodes`/`max_depth_used`, first-omitted-depth frontier, omitted/deferred counts, deterministic replay, and mandatory-envelope `SPX-G109`; source no-write inventory and final byte/identity/revision drift rejection with exact Unix device/inode and bounded Windows held-volume/64-bit-file-index identity nonclaims; patch single-read mutation behavior; automatic behavioral-owner/caller `SPX-G110`; generic-call-in-template seed rejection under existing `SPX-T226` while persistent template reverse callers remain indexed; every Graph v10-v14 source schema; high-cardinality patch/closure evidence; confused CLI rejection; and explicit single-file, trusted-patch-provenance, non-call/repository/incremental/repair/review/commit nonclaims. Focused commands: `cargo test --locked -p semaprax --all-features --test semantic_impact_v1`, `cargo test --locked -p semaprax --all-features --lib impact::tests::`, `cargo test --locked -p semaprax --all-features --lib call_index::tests::`, plus the Patch v2 and Agent Context v2 preservation suites |
| Diagnostic repair / Semantic Patch v3 | Exact canonical `semaprax.diagnostic-repair.v1` and `semaprax.diagnostic-repair-preview.v1` top-level and nested key order; exact embedded three-line LF-terminated `semaprax.semantic-patch.v3` grammar; repair/source/patch/derived-rebase digest domains and frozen report, preview, and independently authored Graph-v10 SHA-256 KATs; exact `SPX-S103` name-span target; closed automatic non-entrypoint function domain; acyclic effect-free/contract-free monomorphic scalar Graph-v10 program closure; persistent-ID syntax, reserved-domain, collision, and name/enum-confusion evidence; hard source/function/call-site/output limits with fixed-point exact whole-JSON byte accounting and no truncation; stale repair and source identity/byte/revision final-check failures; exact one-annotation candidate reconstruction; structural HIR bijection and normalized-Graph excessive-delta rejection as `SPX-G112`; stable direct-caller ordering/counts; read-only discovery/instantiation inventory; v3 exact grammar/schema/selector confusion, failure no-write, stale apply, candidate-revision parity, unchanged A0 race/stage/cleanup evidence, and exact second-apply rejection; exact Graph-v10 revision/identity/callee/derived-ID content rebase plus any identity-bearing CleanupPlan rebase, with no Graph/CleanupPlan schema/version/semantic-shape widening, Graph v11-v14 repair admission, or backend/runtime semantic change; strict Native O0/O2 plus Node/Wasm equality with an independently authored explicit-ID source; Impact-v3 split proving that every syntactically valid canonical v3 reaches `SPX-G110` before semantic selector interpretation while malformed/noncanonical v3 remains `SPX-G101`, with unchanged Impact v1/Patch v1/v2 bytes; confused CLI rejection; and explicit breaking-identity, trusted-patch-provenance, no-general-repair/no-typed-holes/no-other-v3-operation/no-multi-file nonclaims. Focused commands: `cargo test --locked -p semaprax --all-features --test diagnostic_repair_v1`, `cargo test --locked -p semaprax --all-features --test semantic_patch_v3`, the A0/Patch v1/v2 and Impact preservation suites, and `cargo test --locked -p semaprax --all-features --lib` |
| Semantic human review | Exact canonical `semaprax.semantic-review.v1` top-level and nested key order; exact fixed `sections` order `behavior`, `api_identity`, `security_authority`, `memory_ownership`, `target_artifact`, `migration`, `unsafe`; one closed code/disposition/statement finding per authored operation per section with exact operation/evidence indices; closed assessment reduction; fixed-arity flag-free CLI and API-byte-plus-LF equality; source/patch/Impact/identity-rebase digest domains; frozen Patch v1/v2/v3 whole-report SHA-256 KATs; Patch v1/v2 complete nontruncated embedded Impact-v1 equality under fixed depth/byte/node options; v3 exact shared repair identity-rebase equality with no Impact report and zero Impact budget; name-normalized Graph/Cleanup projection proof for all rename domains; exact security-fact equality; breaking v3 identity/derived/callee/Cleanup/symbol/migration classifications; parsed-AST declaration/callable/call-site work bounds before HIR; bounded source/patch reads, fixed-point output accounting, exact budget fields, final source byte/identity/revision/growth rejection, deterministic no-write inventory, stale/malformed/schema/operation/flag confusion, and `SPX-G120`/`SPX-G121` closure. Preserve Impact v1, Diagnostic Repair, Patch v1/v2/v3, A0, Context v1/v2, Graph v10-v14, CleanupPlan, native, and Wasm evidence. Keep no-proof/no-provenance/no-approval/no-verifier/no-A0/no-repository-or-multi-file/no-Context/no-test-or-target-execution/no-general-security-memory-unsafe-ABI/no-Impact-v3/no-persistence/no-external-consumer claims. Focused commands: `cargo test --locked -p semaprax --all-features --test semantic_review_v1` and `cargo test --locked -p semaprax --all-features --lib review::tests::`, plus Impact, repair, and Patch-v3 preservation suites |
| Runtime semantics | Native and Wasm result/trap equivalence, deterministic evaluation order |
| Backend | Host artifact execution, stable failure behavior, exact status mapping, poisoned out-slot preservation on failure, source-order propagation, strict generated-code warnings, cross-platform CI, and sanitizer evidence when memory/ownership lowering changes |
| Aggregate backend | Public nested `i64`/`bool` construction/projection/update executes through strict native C11 at O0/O2 and real Node/Wasm; exact base-first/authored replacement failure selection; internal aggregate pointer parameters and caller-owned results; poisoned result preservation; deterministic layout assertions; empty-record one-byte/alignment-one parity; repeated same-instance Wasm calls with shadow-stack restoration; and exact Native/Wasm result equivalence. Resource-bearing evidence remains a private test-only scenario projected from the same authenticated cleanup plan into C O0/O2 and real Wasm, with exact common finalization trace, poison, zero final liveness, hostile-action rejection, and unchanged public `SPX-B104`/`SPX-W111`, callable/component signature, and stable-ABI gates |
| Generic Copy-record backend | Explicit record templates with owner/index-stable parameters, direct scalar/own-parameter fields, and explicit direct `i64`/`bool` arguments must round-trip and substitute exactly through construction, projection, update, parameters, and results. Require stable diagnostics, hostile identity/order/substitution closure, exact instance-keyed layouts/digests/symbols including same-layout Phantom instances, Graph v12 precedence, unchanged lower snapshots, canonical CleanupPlan replay, strict Native O0/O2, deterministic Wasm, poison/failure order, and 4,096-call Node re-entry. This gate is hosted green in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995). Generic-record inference, nested/resource/non-Copy arguments or fields, public aggregate/callable/FFI ABI, and public resource gates remain closed; generic functions and record patterns have separate gates. Focused commands: `cargo test --locked -p semaprax --all-features --test generic_records` and `cargo test --locked -p semaprax --all-features --test executable_generic_record_backends` |
| Bounded generic Copy-function backend | Canonical `fn id<T>` declarations and explicit `id<i64>`/`id<bool>` calls with one or two owner/index-stable parameters; direct scalar/own-parameter by-value signature slots; stable `SPX-T224`/`T225`/`T226`; comparison-token lookahead; all-`2^N` validation of unused templates without materialization; exact explicitly referenced instance order; domain-separated template/instance/value/expression identities; same-signature template and concrete-instance confusion rejection; generic-to-generic/transitive-cycle/recursion/effect/entrypoint/aggregate/resource closure; hostile HIR template/body/argument/order/call-instance/cleanup rejection; program-wide Graph v14 precedence, exact function-template/function-instance/call-instance facts, mixed-root context, frozen module/Agent Context/bounded-context KATs, and unchanged v10-v13 output without a generic function. CleanupPlan v2 stays byte/schema/meaning unchanged and template-ID-only, with exact instance admission owned by HIR/Graph. Require deterministic strict Native C11 O0/O2 exact symbols, contracts, argument/body/postcondition failure order and poison; deterministic Wasm exact indices and 4,096-entry Node re-entry. Local evidence, independent security review, and the hosted matrix are green, with hosted evidence in [run 31385406865, Ubuntu job 93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). Callable/trace/settlement/resource/owned boundaries remain closed; Component authority is limited to the separate exact private v9 profile below and grants no general/public mapping. Focused commands: `cargo test --locked -p semaprax --all-features --test generic_functions` and `cargo test --locked -p semaprax --all-features --test executable_generic_function_backends` |
| Irrefutable Copy-record patterns | Require one exact record scrutinee evaluation, one scalar arm, exact recursive fields/bindings, hostile field/type/identity closure, wildcard schema neutrality, straight-line CleanupPlan v2/v3, and Native C11 O0/O2 plus 4,096-entry Node/Wasm evidence. An explicit authenticated record pattern selects Graph v13 above v12/v11/v10 unless a generic function declaration selects v14; mixed-root context and lower-schema byte preservation remain required. This gate is hosted green in [run 31373317800, Ubuntu job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130). Refutable/literal/guard/or/rest/nested-variant patterns, non-Copy/resource matching, aggregate arms, and public aggregate/callable/FFI admission remain closed. Focused commands: `cargo test --locked -p semaprax --all-features --test record_patterns` and `cargo test --locked -p semaprax --all-features --test executable_record_pattern_backends` |
| Copy/generic-variant backend | Monomorphic unit/direct-scalar variants plus explicitly instantiated direct-scalar templates and ordinary `Option`/`Result` execute exhaustive Copy matches with scalar arms through strict native C11 O0/O2 and Node/Wasm. Require exact substitution/identity/layout/symbols, hostile arity/scope/nested/resource closure, authored constructor order, one scrutinee, selected-arm-only execution, full poison, invalid-tag closure, shadow-stack restoration, and native/Wasm agreement. Generic-function use of variants, nested/resource arguments, resource- or record-bearing payloads, non-copy match modes, public aggregate ABI, callable/component signatures, and public resource gates remain closed. Focused commands: `cargo test --locked -p semaprax --all-features --test generic_variants`, `cargo test --locked -p semaprax --all-features --test graph_generics`, and `cargo test --locked -p semaprax --all-features --test executable_variant_backends` |
| Typed ordinary-`Result`/`Option` propagation | Canonical postfix parse/format/parse and precedence; stable `SPX-T218`/`T219`; exact compiler-owned carrier/member/source/outer identities; hostile identity/type/ownership rejection; one evaluation, later-work skip, shared ensures/commit, status separation, poison, invalid-tag closure, and native C11 O0/O2 plus Node/Wasm equality. Result-only programs retain CleanupPlan v2/Graph v10. Option uses per-function CleanupPlan v3 and program-bound Graph v11, raised to v12 by a generic record, v13 by an explicit record pattern, or v14 by any generic function declaration. The separate exact private Component v10 gate below maps only `Option<i64>` through postfix `?` to `Option<bool>`; keep residual conversion, nested/resource/non-copy arguments, generic-function `?`, contract `?`, public aggregate ABI, general/public callable or Component signatures, and conformance-trace aggregate values closed. Focused commands: `cargo test --locked -p semaprax --all-features --test option_try_semantics`, `cargo test --locked -p semaprax --all-features --test result_try_semantics`, `cargo test --locked -p semaprax --all-features --test graph_result_try`, and `cargo test --locked -p semaprax --all-features --test executable_try_backends` |
| Interop or package | Bidirectional conformance fixture, ownership/error mapping, reproducible artifact |
| Private native desktop UI | Existing private desktop engine remains feature-gated/unpublished and `SPX-B104` stays closed; exact AppKit and Win32 source; one visible titled native window and labeled native button; platform accessibility-name query; delayed OS control event through the real event loop; canonical SHA-256 manifest and exact engine-byte verification before launch; executable-preserving mismatch rejection before result publication; exact engine subprocess output; bounded AppKit deadline/terminate/kill with a digest-valid hanging-engine regression; ordered close/terminate evidence; success-file publication only after termination; pinned compiler/linker/SDK/import-library roots; byte-identical double UI build; foreground macOS `APPL` plus exact framework/load/export/build-version/inventory checks; x64 PE32+ GUI subsystem plus exact seven-DLL import set, absent export directory including ordinal-only functions, path/manifest/inventory checks; hostile source-lock removal; mandatory hosted macOS and Windows launch; explicit no-signed-provenance/co-replacement defense, language-UI/state/layout, SwiftUI/WinUI, general accessibility/lifecycle, signing/installer/distribution, or public-admission claim |
| UI/platform | Accessibility checks, lifecycle/capability tests, representative simulator/device or host evidence |
| Private native desktop app | Default-off feature and unpublished host; exact generated callable-v3 provider/descriptor; pinned and asserted Rust/LLVM/Clang plus Apple ld/macOS SDK build or MSVC linker/Windows SDK import-library identity; Cargo offline; two independent byte-identical builds within that exact toolchain (not a cross-toolchain/SDK claim); stable package-relative macOS install identity; canonical content-derived `LC_UUID`, two independently assembled byte-identical signed app bundles, timestamp-free fixed-identifier ad-hoc signatures, strict bundle verification, and no distribution credential; exact Mach-O or PE/COFF architecture/file-kind/load/import/export/path checks; canonical macOS `APPL` or effective Windows `asInvoker` manifest and exact package inventory; hostile inspection/source-lock regressions; two authenticated owned publications with refreshed-generation reuse; exact cached replay; no network; mandatory hosted macOS and Windows launch; explicit no-window/UI/accessibility/lifecycle/installer/public-admission and unchanged `SPX-B104` claims |
| Private Android JNI/Kotlin APK | Feature remains private and `SPX-B104` stays closed; exact four-output target-matched generator; strict NDK x86_64/arm64 provider/JNI compilation; exact shim export/dependency/path inspection; independent handle/status KATs; HandlerThread-only host access; explicit `consume()` restore-only-on-defined-precommit semantics; non-throwing `AutoCloseable.close()`/`PhantomReference` fallback; identical deterministic Cleaner action; exception clear/normalization; poisoned-output preservation; exact O0/O2 finalizers/receipt/publication/allocation/handle-table result; plugin-free repository-free Gradle 9 `--offline` packaging from pinned runner tools; exact APK inventory/signature/alignment; clean install; API-35 x86_64 framework-Instrumentation execution; and exact app-private result. The bounded gate is green in [run 31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206); arm64 remains compile/inspect only |
| Private Apple Swift ownership app | Feature and C/Rust modules remain private and `SPX-B104` stays closed; exact device-arm64, Simulator-arm64, and Simulator-x86_64 descriptor/provider pairing; fixed hidden evidence hooks with no legacy caller-configurable raw open; stable Swift `Thread` FIFO under complete Swift 6 concurrency checking; handle/status KATs; poison-preserving outputs; explicit consume and identical deterministic ARC-deinit cleanup; stale/forged/wrong-thread, live-close, race, repeated-call/reset, and no-retry checks; exact O0/O2 finalizers/receipt/publication/ledger/allocation/empty-table result; strict C warnings; device and universal-Simulator XCFramework inspection; ad-hoc signing; installed arm64-Simulator app execution; and exact app-container result. The bounded gate is green in [run 31338834586, job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228); physical-device, public-framework, and general lifecycle claims remain closed |
| Private WIT/component boundary v1/v2/v3/v4/v5/v6/v7/v8/v9/v10 | Preserve every v1-v9 byte/KAT contract and all existing hosted evidence. Private Record-Pattern Projection Component v8 and Private Generic-Function Instance Component v9 retain their exact separately hosted contracts. Private Source-Option Propagation Component v10 is a separate default-off, capability-free exact profile for package `semaprax:private@0.8.0`, interface `option-propagation`, world `semaprax-private-v10`, and one `evaluate(input: option<s64>, divisor: s64) -> result<option<bool>, status>` export selected from the exact compiler-owned `Option<i64>` through postfix-`?` to `Option<bool>` source function plus `app.main`. Require no authored types/resources/templates/instances/imports/capabilities; exact source/Graph-v11/prelude/two-layout/CleanupPlan-v3/core/profile/raw-component/artifact-DAG KATs; exact selected closure and WIT order; independent/upstream validation; every-byte/truncation/trailing/noncanonical/cross-version hostility; typed and raw Some/None/contracts/arithmetic/sticky-failure/repeated/fresh-instance behavior; status-before-output, tag-last publication, full 20-byte poison, input/output tag and bool plus unknown-status closure; out-of-band fuel exhaustion; default-consumer hiding; source/runner/CI locks; strict gates; and independent security review. Local v10 core 5/5, component 4/4, CI-lock 4/4, full gates, and security review are green. The isolated pinned-Rust-1.97.1/Wasmtime-47 zero-import/empty-linker/no-WASI typed v3-v10 runner is hosted green in [run 31396483313, job 93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502). General source selection/export, general `Result`/`Option`/`?` or algebraic Component mapping, nested/resource/non-Copy carriers, general generic-function Component mapping, inference/constraints, imports/capabilities, callbacks/reentrancy/async, callable/FFI aggregate signatures, browser/multi-engine conformance, package negotiation, public API/ABI, and `SPX-B104`/`SPX-W111` remain closed. Focused v10 commands: `cargo test --locked -p semaprax --all-features --lib wasm::option_propagation_component_v10::tests::`, `cargo test --locked -p semaprax --all-features --lib wit_component::option_propagation_v10::tests::`, and `cargo test --locked -p semaprax --test component_runtime_ci_contract` |
| Security boundary | Threat-model delta, hostile input test, no newly ambient capability |
| Native capability token | Published HMAC KAT, independently reproduced owner and result complete-token goldens, exact length/layout/endian assertions, every-bit, arbitrary-byte, and length-boundary mutation, closed kind/reserved/zero-field parsing, cross-secret/module/adapter/epoch/template/type/lifecycle/thread-policy/thread-binding rejection, owner-versus-result scope, stale/max-generation evidence, pinned audited crypto dependency, and proof that compiler preflight creates no authority |
| Native capability authority | Exactly pinned OS-random dependency and locked checksum, one exact fill with no fallback/retry, partial-error and every structural-zero rejection, invalid-binding-or-draining-lease before entropy proof, test-only deterministic injection, independently reproduced complete owner/result goldens, lease-derived physical fingerprint, every sealed context delta, wrong-thread-first owner/result mint/authentication, same-thread recovery, exact-instance wrapper authentication, owner/result lease retention after authority drop, explicit `Send + Sync` policy, non-formatting credential wrapper, stable error redaction, native OS smoke on every desktop CI host, Rust 1.85, and unchanged private/export/`SPX-B104` boundaries |
| Native module lease topology | Fake-backed unit evidence still proves equal-fingerprint instance nonconflation, exact retention, drain ordering, cross-instance rejection despite equal bytes, exactly-once fake release, and no cycles; the real physical-host gate below must independently prove the corresponding loader-backed lifetime rather than treating the fake as physical evidence |
| Native settlement model | Fixed resource/checkpoint/progress/work ceilings; nonzero recovery-contract and nonempty/no-NUL function identity; one all-live post-`CallCommit` start; dense checkpoints; canonical typed progress; exact cleanup permutations; trace-bound terminal outcomes; exhaustive abort and accepted-outcome owner-state enumeration through six resources; executable post-`CallCommit` `Executing`, exact `DecisionLocked`, `ActionInProgress`, `ProviderSettled`, model-`ReceiptCommitted`, and absorbing `Quarantined` phases; pre-decision unwind selecting `Abort(HostUnwind)` and post-decision unwind resuming the exact locked decision; unknown/conflicting phase quarantine; `Finalizing` recorded before effect, `Dead` only after normal return, and interruption quarantine with no retry; provider `Published` distinguished from public ledger publication; unique receipt-commit eligibility only after candidate validation; same-decision idempotence with no repeated actions; stale/cross-bound/skipped/reordered/duplicate candidate rejection; exact independently reconstructed receipts; zero active finalizers and terminal dispositions; deterministic domain-separated certificate/receipt fingerprints and migration note; hostile structural/progress/receipt mutations; quarantine evidence retention; start-only nonmutating progress walks; non-`Clone`, non-formatting frame compile-fail tests; and an executable nonclaim that model preparation is not invocation/module/frame-generation reservation, allocation-free physical execution, physical-finalizer authority, host authentication, or ledger mutation authority |
| Native callable settlement proof v1 | Private `SPXNPRF1` only; exact unchanged callable-v2 bytes plus canonical pointer-free binary graph in one immutable envelope; 64 KiB global ceiling enforced during compiler serialization and before host allocation; independent encoder/parser and canonical re-encoding; acyclic domain-separated schema/v2/graph/envelope fingerprints; exact source-v2 call-contract and trace-certificate cross-binding; dense checkpoints, one all-live start, reachability, forward DAG, typed transition and cleanup-order continuity validation; all 14 corpus cases; fixed known answer; every prefix, trailing byte, and single-byte mutation; rehashed hostile graph, unknown tag, invalid text, hostile count, cross-module, and changed-trace rejection; v2 loader pre-open rejection; default-consumer compile-fail; unchanged v2 bundle/provider/CLI and `SPX-B104`; and explicit no-provenance, no-authority, no-descriptor-v3, no-provider, no-runtime-settlement nonclaims |
| Native callable ABI v3 descriptor/wires | Private `SPXNABI3` only; exact descriptor/signature/graph plus independently implemented compiler/host codecs for `SPXNRQ03`, `SPXNEX03`, `SPXNFR03`, `SPXNDC03`, `SPXNAC03`, `SPXNCR03`, and host-only `SPXHRP03`; frozen 20-byte envelopes, six-argument execute ABI, payload-bearing frame cells, closed tags/phases/dispositions, checked capacities, request/response/semantic/decision/action/frame/candidate digest DAG, separate-key receipt HMAC, canonical re-encoding and changed private known answers; every-prefix/trailing/every-byte mutation, hostile tag/count/cap/cross-binding/replay, v1/v2/proof/v3 confusion, unchanged v2/proof KATs, default-consumer hiding, and both legacy loaders rejecting v3 before path/image access; distinct dynamic/iOS-static metadata and iOS device/simulator/macabi target identities. Graph-derived providers execute all 14 normal scenarios through the loader/host at O0/O2 with zero measured Rust heap growth across the irreversible interval; decode-reserve failure quarantines exact evidence, and seven joint failure fixtures cover returned/malformed/interruption/replay/conflict paths. Canonical pre-execute unwind uses tag 3 plus sentinel `0xFFFF_FFFE`, hashes exact zero storage, skips execute, and reaches authenticated abort receipt. A bounded process-lifetime exact-address static-registration model feeds the same private host ledger; non-Apple fake functions prove same-thread idempotent retention after every explicit lease drops, cross-thread/target/address conflict rejection, draining, and quarantine without any unload claim. Dedicated macOS CI must compile the static-only loader/host composition for arm64 device, arm64/x86_64 simulator, and arm64/x86_64 Catalyst targets and prove every iOS dependency graph excludes `libloading`. It must also generate one exact arm64-Simulator `token.discard-two` provider, link a standalone ad-hoc-signed Mach-O with no workspace dylib, run it at O0/O2 through `simctl`, and require exact finalizers, authenticated no-owned receipt/ledger transition, and zero measured Rust allocation across the irreversible interval. A separate pinned Android job must compile exact Bionic/ELF providers and the dynamic loader/host for x86_64 and arm64, inspect both ELF architectures, and run the x86_64 `token.discard-two` provider through canonical-path admission and receipt commit at O0/O2 in an API-35 emulator. Runtime evidence counts only when the corresponding hosted job is green. The separate private JNI/Kotlin APK gate is green in [run 31324497016, job 93272580149](https://github.com/wavect/semaprax/actions/runs/31324497016/job/93272580149), but grants no AAR, device/app lifecycle, remaining-mobile-corpus, fatal allocator/process-crash, public-execution, or `SPX-B104` claim |
| Native callable bundle | Public preflight requires an explicit persistent function ID and at least one direct `own` trivial-resource parameter; deterministic descriptor/provider/dictionary/certificate derivation and preflight SHA-256; strict host Clang shared-library build; exact regular-file-only staged/final inventory with Windows import-library suppression and checked removal of the linker's `.exp` side artifact; canonical sorted per-payload SHA-256 manifest plus manifest checksum; byte-identical double build on each host; build-only API/CLI with no load/invoke/adopt/authority surface; default-feature external-consumer compile-fail proof for host internals; ordinary native `SPX-B104`; missing/automatic/excluded-function diagnostics; observed file/directory/dangling-symlink no-overwrite; failed-Clang staging cleanup; canonical example and green [Ubuntu](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277094), [macOS](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277081), and [Windows](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277085) hosted-CI builds. The canonical output parent is trusted against concurrent adversarial mutation because portable `std` lacks atomic directory rename-no-replace |
| Rust native-host ASan | Dedicated Ubuntu 24.04 job; every Rust command routed through the exact pinned nightly and compiler commit; audited Clang 18 major for the executable and generated providers; `cfg(sanitize = "address")` compile-time probe; intentional Rust heap-use-after-free diagnostic; rebuilt target standard library; verbose host-crate `rustc` flag proof; ASan symbols in the host binary and callbacks in the generated provider; and real callable-host plus authoritative-corpus execution. The [fail-closed contract and exact green public job](RUST-HOST-SANITIZERS.md) must remain intact; configuration or static tests alone never count as runtime evidence |
| Native loader quarantine | Separate unpublished crate; main crate still forbids unsafe; exact dependency pin and workspace supply-chain gate; separately documented descriptor-only, callable-v2, dynamic settlement-v3, and iOS-static registration boundaries; no generic lookup/raw handle/raw pointer/manual close; canonical-path, symbol, descriptor, and wire bounds; null and exact-byte checks; legacy constructors reject `SPXNABI1`, `SPXNPRF1`, and full-magic `SPXNABI3` as applicable before loading; Unix `RTLD_NOW | RTLD_LOCAL`; Windows root/default-safe dependency search without current-directory/legacy-PATH lookup. The private dynamic v3 constructor retains an immutable exact descriptor copy and proves root-image provenance. A bounded static-table model requires exact descriptor-storage/getter/execute/settle addresses, idempotently retains one same-thread instance, rejects cross-thread/partial address reuse and target relabeling, and has no path/close/unload surface. Both lease forms preallocate the same five descriptor-projected buffers and expose instance-bound one-shot execute/settle preparations; compile-fail gates enforce `!Send`, `!Sync`, non-`Clone`, and non-formatting wrappers. Real Linux/macOS/Windows dynamic fixtures cover positive, rejection, dependency provenance, same-capacity descriptor substitution, cross-instance, last-reference, and unload behavior; non-Apple static fake functions cover retention/quarantine. The static-only loader and private host composition must type-check for five iOS-family Rust targets with zero resolved `libloading` dependency, and one exact arm64-Simulator provider must link and execute through the static lease/ledger in the hosted O0/O2 gate. Android must compile the dynamic loader/host and exact providers for x86_64 and arm64 with `libloading 0.8.9`, inspect both NDK ELFs, and execute canonical-path x86_64 admission in the hosted emulator gate. The private JNI/APK adapter is separately green on hosted API-35 x86_64 in [run 31324497016, job 93272580149](https://github.com/wavect/semaprax/actions/runs/31324497016/job/93272580149); malicious-code containment, file/code identity, immediate unmapping, device/general-corpus execution, lifecycle breadth, quiescence, fork safety, and public admission remain explicit nonclaims |
| Native physical ownership host | Separate unpublished crate and unchanged compiler `unsafe_code = "forbid"`/`SPX-B104`; strict descriptor-v1 and independent descriptor-v2 decoding; every-byte/truncation/trailing mutation rejection; compiler-authenticated dictionary plus trace-path certificate; strict allocation-free postcommit response decoding in callable v2; exact loader-instance callable binding; OS-seeded same-thread authority; credential-to-ledger resource/lifecycle/slot/generation checks; non-mutating fully allocated plans and atomic commit; unsafe trusted adoption; noncopying/nonformatting/`!Send`/`!Sync` owners; reusable owners after precommit rejection; scalar/owned success, normalized semantic/adapter failure, generation rotation, draining, lease retention, and cross-instance rejection; real generated O0/O2 shared libraries executing the full callable-v2 14-case reference corpus through the host; green public Linux generated-provider ASan+UBSan, Rust-host ASan, and Windows callable/dependency-isolation evidence; public build-only bundle emission. Private callable-v3 evidence includes independent pre-settle/candidate replay, exact descriptor/instance/frame binding, separate one-fill receipt authority and HMAC KATs, authoritative duplicate/stale/cross-frame rejection, zero measured Rust heap growth across all 14 O0/O2 joint normal paths, atomic publication with refreshed generation, exact cached replay, panic-safe RAII evidence/pin quarantine, and injected decode-reserve failure retaining exact evidence. Seven joint failure fixtures cover returned/malformed/interruption/replay/conflict paths; canonical pre-execute unwind reaches abort receipt without execute. Hosted run 31313341303 proves Linux/macOS/Windows, MSRV, dependency policy, generated-provider ASan+UBSan, and Rust-host ASan; [run 31324497016, job 93272580149](https://github.com/wavect/semaprax/actions/runs/31324497016/job/93272580149) adds the bounded Android JNI/APK path. Keep `SPX-B104` closed until exhaustive fatal-allocation/physical-failure/crash evidence, quiescence, representative general Android/iOS execution, and public execution/admission are proven |
| Wasm owned ABI v1 | Exactly one direct trivial-resource identity and stable `SPX-W111` exclusions; replay-validated plan admission and hostile-plan `SPX-H006`; deterministic Wasm bytes, embedded exact SHA-256 artifact authentication before host construction, export metadata, `semaprax.web.v3` manifest mapping, and scalar-lane regression; private host imports; exact export/count/canonical-i64/positive-i32/result-kind validation; one-shot branded adoption with reuse rejection and nonmutation on capacity failure; instance-tagged slot/generation stale, replay, duplicate, same-module, and duplicated-generated-module same-realm cross-instance rejection plus invalid/repeated allocator fail-closed tests; complete 4/8-byte precommit out-range/alignment validation; one reserved status cell plus resolvable exhaustion sentinel; exact canonical `semaprax.status.v1` field shape; atomic owner staging and reusable owners after rejection; reverse exact-once cleanup and equal-payload exact-owner selection; owned-result rotation; requires/overflow/ensures failure with poison preservation; result/status/slot exhaustion at bounded test limits; real Node execution; compiler-generated semantic ordinals authenticated by the deterministic dictionary and materialized to exact reference traces for the shared 14-case corpus. A trusted unpoisoned same-realm host environment, cross-realm/worker identity, Components, imports/finalizers, async/shared memory, and production callable-native equivalence remain explicit boundaries/nonclaims |
| Owned resource vertical slice | The complete [v1 contract](OWNED-RESOURCE-VERTICAL-V1.md): structural narrow-slice admission; production-reachable compiler and host APIs; indivisible parameter-ordered owner commit; reusable owners after rejection; exact-once ordinary/result/failure cleanup; result-aware finalization; native unsafe admission and adoption quarantine; same-thread exact loader retention and draining; instance-bound Wasm `(slot, generation)` handles; exact native/reference/Wasm status/publication/trace equality for the full corpus; real Linux/macOS/Windows native calls and Node/Wasm execution; O0/O2 and mandatory ASan/UBSan; deterministic C/descriptor-v2/Wasm bytes and symbols; exhaustion/wraparound, hostile-boundary, compile-fail trait, MSRV, package, and dependency-policy gates; unchanged fail-closed diagnostics for every excluded shape |

The gated native cleanup corpus runs at O0 and O2 everywhere Clang is available. Linux CI additionally sets `SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1`, which makes separate ASan and UBSan compile/run support mandatory rather than allowing capability-based skips.

The dedicated Linux generated-provider sanitizer lane first passed in
[`callable-host-sanitizers` job 93099637801](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801):
all 14 authoritative O0/O2 cases executed from dynamically loaded providers
with combined ASan/UBSan instrumentation through the Rust host. Its linker flags
made the sanitizer runtimes available to that executable; they did not
instrument the Rust host code. The separate fail-closed [Rust-host ASan
job](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065)
then passed with the Rust host itself instrumented, inside a [fully green public
current hosted-CI run](https://github.com/wavect/semaprax/actions/runs/31259216533)
that also covered Linux, macOS, Windows, Rust 1.85, dependency policy, and the
stable provider sanitizer lane. This is not app-platform or mobile evidence.
The two jobs remain distinct evidence boundaries; Rust-host UBSan is not
inferred.

Authority goldens prove deterministic mechanics, and OS smoke proves only that
the supported host source was available. The private physical-host fixtures now
add exact loader-backed retention, ledger replay rejection, same-thread owner
topology, and generated callable execution. The shared 14-case corpus proves
exact reference/native-host-O0/O2/Wasm semantic trace and outcome equality for
the narrow admitted shape. None of this proves mathematical uniqueness, key
secrecy after compromise, fork safety, general postcommit fallback cleanup or
quiescence, Android/iOS admission, or public native backend conformance. The
bounded Linux Rust-host ASan evidence is separately recorded above.

The established Windows gate runs
`runtime_loader_windows::windows_callable_dependency_search_excludes_cwd_and_legacy_path`
and explicitly reruns the complete callable-v2 generated O0/O2 corpus. Both passed
in [run 31257545008, job
93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756);
this does not prove broader Windows application-platform completion.
The workflow now additionally names the all-14 private v3 provider test and the
all-14 joint provider/loader/host test, while the Linux lane requires the
v3 provider corpus under ASan+UBSan and the joint path in the dynamically
loaded sanitizer job. [Run 31315343417](https://github.com/wavect/semaprax/actions/runs/31315343417)
proved the current pre-iOS-cross-check baseline on Linux, macOS, Windows, MSRV,
dependency policy, generated-provider ASan+UBSan, Rust-host ASan, release, and
package gates. [Run
31316677457](https://github.com/wavect/semaprax/actions/runs/31316677457)
then proved the five-target iOS type-check and no-`libloading` dependency
contract. [Run 31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
proved the separate arm64-Simulator provider/loader/host O0/O2 runtime contract.

Semantic Target Evidence v1 requires exact report/nested order, Graph-v10-v14
and Patch-v1-v3 admission, typed capability-fact order and zero delta, all
digest domains, fixed-point accounting, parsed-AST bounds, source final checks,
production C11/Wasm byte equality, pinned wasmparser structural validation,
the three report KATs, and ordered nonclaims. Hosted integration must compile/
run the exact candidate C at O0/O2 and run exact candidate Wasm through Node,
while proving execution is absent from the report and grants no authority.

Evidence v2 requires strict additive capsule/receipt parsing, exact Target
Evidence binding, unchanged Review/Evidence-v1 bytes and KATs, v1/v2/v3 replay
parity, v3 `target_artifact = change_proven`, hostile/confusion cases, lock-
before-read, replay-before-stage, and unchanged A0/ordinary patch behavior.

```sh
cargo test --locked -p semaprax --all-features --test semantic_target_evidence_v1
cargo test --locked -p semaprax --all-features --lib target_evidence::tests
cargo test --locked -p semaprax --all-features --test semantic_patch_evidence_v2
```

Focused results are Target 9/9, Target units 4/4, and Evidence v2 8/8. Root
library 439/439, full workspace/all-target/all-feature, release, host 11/11 and
loader 26/26 doctests, rustdoc `-D warnings`, strict Clippy, formatting, diff,
preservation, and security are locally green. The exact
`fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in [run
31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793), with
[dependency job
93624123614](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123614),
[Ubuntu job
93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631),
[macOS job
93624123633](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123633),
[Windows job
93624123715](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123715),
[component job
93624123698](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123698),
and [MSRV job
93624123711](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123711);
all 12 jobs passed. The current dashboard remains exactly 39 Partial/17 Missing.

Semantic Workspace Transaction v1 additionally requires the exact managed
generation and `ACTIVE` publication gate above. Its frozen two-file initial
workspace revision is
`sha256:9a7368825342cee138d02a8037248e9a41ed0479d4f7c32a21c7ee7141cf280c`;
snapshot and preview JSON SHA-256 KATs are
`3646097c9fb8c47bced51cf2c404b886755f657c73c57afb18d25282574f0b80`
and `a4f1a9467d535aada97e7f253cf51c0d2168b5557a5a400d11692ac6966776b4`.
Mixed Patch-v1/v2/v3 snapshot and preview KATs are
`dfd35db518d0a8d94b83702dd1d2760ce9340b5875e0960ac573f84474c223b5`
and `3cbd8d22bc26069387ac8ebce72ca590f095cbaa193b04bdef041e4c06beced1`.

```sh
cargo test --locked -p semaprax --all-features --test semantic_workspace_transaction_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test semantic_workspace_transaction_v1_hostile -- --test-threads=1
cargo test --locked -p semaprax --all-features --lib workspace::tests
```

Focused local results are integration 12/12, hostile wire/CLI 5/5, and
workspace units 37/37. Root library 482/482, full workspace/all-target/all-
feature, release, host 11/11 and loader 26/26 doctests, rustdoc `-D warnings`,
strict Clippy, formatting, diff, examples, preservation, and security are
green. The exact `afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted
green in [run
31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068), with
[dependency job
93719800523](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800523),
[Ubuntu job
93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613),
[macOS job
93719800554](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800554),
[Windows job
93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611),
[MSRV job
93719800689](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800689),
and [component job
93719800635](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800635);
all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
strict Clippy and is not green evidence; no earlier Phase A/B workflow is Phase
C publication proof. Current status remains exactly 39 Partial/17 Missing.

Semantic Workspace Patch Evidence v1 additionally requires all eight literal
whole-artifact KATs, exact capsule/receipt and nested order, every canonical
wire substitution, exact/one-over limit, lock/read/diagnostic precedence, and
replay-before-candidate/staging evidence frozen in [its normative
contract](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md). The shared Workspace source
and AST limits bound HIR/index work before child construction; remaining
Impact, Review, and child-artifact budgets cap closure/serialization rather
than HIR. Submitted evidence remains immutable owned untrusted input until
exact typed and byte replay succeeds.

```sh
cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1_apply
cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1_hostile
cargo test --locked -p semaprax --all-features --lib workspace_patch_evidence::tests
```

Focused local results are public generation/verification 6/6, apply 5/5,
hostile 2/2, and module units 8/8. Shared Workspace core 39/39, Workspace
integration 12/12, root library 496/496, preservation 107/107, full workspace/
all-target/all-feature, release, examples, host 11/11 and loader 26/26
doctests, rustdoc `-D warnings`, strict Clippy, formatting, diff, and security
are locally green. The exact `cda4892ee74100fd11c5161ad857d469ec5e5421`
matrix is hosted green in [run
31491573287](https://github.com/wavect/semaprax/actions/runs/31491573287), with
all 12 jobs passing: [Dependency
policy](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116816),
[Ubuntu](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117078),
[macOS](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116941),
[Windows](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117130),
[MSRV](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116811),
and [Component](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116886).
The intermediate exact `658b2f4dc6d69974cef553dbd4e6eaecafacdd63`
documentation/count head [run
31490049153](https://github.com/wavect/semaprax/actions/runs/31490049153)
was nonqualifying and cancelled: its macOS early-error precedence test observed
`SPX-I210` instead of the expected `SPX-G150`; Windows was
concurrency-cancelled after that failure and reported no product failure.
The exact `3e41b3a0318730fec41e7d75438414e93dafa313` predecessor [run
31486578192](https://github.com/wavect/semaprax/actions/runs/31486578192)
was nonqualifying at 10/12: macOS exposed snapshot-lock handoff as
`SPX-I210` instead of the expected stale `SPX-G152`, while the Windows
lock-precedence fixture hit OS error 33 reopening the locked `LOCK` file. The
corrective head makes the owned-snapshot lock release explicit and avoids the
fixture-only reopen without changing the wire contract. No earlier Workspace
or single-file Evidence run qualifies. Current status remains exactly 39
Partial/17 Missing.

## Evidence strength

Semantic Patch v2 changes additionally require the exact grammar/schema
confusion matrix, persistent owner/member/case identity and wrong-domain tests,
pre-state batch resolution, pattern-shorthand binding/Place preservation,
complete generic call tuple mismatch coverage, selective post-HIR Graph delta,
unchanged layout/CleanupPlan facts, stale/failure no-write evidence, the legacy
v1 and A0 suites, a canonical post-edit revision KAT, and the existing strict
Native O0/O2 plus Node/Wasm backend equivalence gates. The focused transaction
suite is `cargo test --locked -p semaprax --all-features --test semantic_patch_v2`.
The focused suite is 9/9, and the complete exact-`f95d243` matrix is hosted
green in [run 31401200449 attempt
2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2),
including [Ubuntu job
93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044).
The isolated component runtime graph is separately green in [Wasmtime job
93505622110](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622110).

Semantic Impact v1 changes additionally require independent JSON parsing,
exact whole-document byte accounting, the frozen canonical report KAT,
source-inventory and final snapshot no-write evidence, exact processed patch
digest replay, preview/apply candidate-revision parity, all Graph v10-v14
schema selections, high-cardinality deterministic closure, and preservation of
Agent Context v1/v2 and Patch v1/v2 behavior. The local focused suites are
12/12 integration and 4/4 internal across `tests/semantic_impact_v1.rs`,
`impact::tests`, and `call_index::tests`. The exact `1b3731a` full hosted matrix
is green in [run 31408654657 attempt
2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
including [Ubuntu job
93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).

Diagnostic Repair v1 and Semantic Patch v3 changes additionally require the
exact report/preview/v3 schemas and key/line order, the three frozen SHA-256
KATs, exact diagnostic/location key order, the closed caller-origin and
derived-entry-kind enums, algorithm-tagged protocol digest fields, raw whole-
artifact KAT distinction, lexicographic `(kind,before,after)` derived-entry
order including the zero-entry domain-only digest, every closed
target/input/program domain, hard work/output bounds, exact
parsed-AST pre-HIR function/call-site rejection, parsed-v3-only bounded initial
and two-final source reads, greater-than-16-MiB final-growth preservation,
source final-check behavior, independent one-edit candidate reconstruction,
the structural-HIR and normalized-Graph breaking-rebase gate, and unchanged A0
commit behavior. V3 must authenticate every selector, reject grammar/schema and
v1/v2 confusion, leave source unchanged on every failure, and prove the Impact
split: syntactically valid canonical v3 is unsupported as `SPX-G110` before
semantic selector interpretation, while malformed/noncanonical v3 remains
`SPX-G101`. Local Phase A integration is 13/13; the Phase B semantic
integration corpus is 7/7; v3 A0 hook units are 4/4; aggregate v3
integration-plus-hook evidence is 9/9; and the library suite is 404/404. Full
preservation is green and independent security review is clean. The 9/9
aggregate means seven semantic integration cases plus two bounded-work
integration hooks; the separate internal v3 A0 hook-unit result remains 4/4.
Its focused hook command is
`cargo test --locked -p semaprax --all-features --lib patch::commit_tests::v3`.
The exact `dae957a` full matrix is hosted green in [run 31418476217 attempt
1](https://github.com/wavect/semaprax/actions/runs/31418476217/attempts/1),
including [Ubuntu job
93553147265](https://github.com/wavect/semaprax/actions/runs/31418476217/job/93553147265);
all 12 jobs passed.

Semantic Review v1 changes additionally require exact schema and seven-section
wire order; one finding per operation in every section; closed code,
disposition, assessment, operation-index, and evidence-ID behavior; independent
JSON parsing; exact source, patch, Impact, and identity-rebase digest domains;
and frozen whole-report SHA-256 KATs
`054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946`,
`37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421`, and
`081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544`
for Patch v1/v2/v3. V1/v2 must embed the exact complete nontruncated Impact v1
object under fixed depth 1,024, 16-MiB, and 1,024-node limits. V3 must embed the
exact shared repair identity-rebase object, omit the Impact report, and record
zero Impact budget. Require parsed-AST pre-HIR declaration/callable/call-site
bounds, bounded source/patch reads, fixed-point output accounting, final source
drift/identity/over-bound-growth rejection, fixed-arity CLI confusion, no-write
inventory, every Patch v1/v2 operation family, the sole v3 operation, and
unchanged Impact/repair/Patch/A0/Context/Graph/Cleanup/backend evidence.

Local Review integration is 10/10 and hook/limit units are 4/4. Library
408/408, full workspace, release, doctest, rustdoc, strict Clippy, format, diff,
preservation, and independent security gates are green. The exact
`2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in [run
31423743369 attempt
1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
including [Ubuntu job
93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170),
[Windows job
93570423172](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423172),
[macOS job
93570423226](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423226),
[MSRV job
93570423203](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423203),
and [dependency-policy job
93570423175](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423175);
all 12 jobs passed. Review's target/artifact section and hosted
backend preservation do not constitute target or test execution by Review.
The no-Context, no-public-verifier/proof-artifact, no-provenance, no-approval,
no-A0-authority, no-general-analysis, and single-file nonclaims remain exact.

Semantic Patch Evidence v1 changes additionally require exact closed capsule
and receipt schemas, all nested key orders, duplicate-key and depth rejection,
fixed-point byte accounting, the frozen source/Patch/Impact/identity/Review/
artifact digest domains, and the six whole-artifact KATs in
[`SEMANTIC-PATCH-EVIDENCE-V1.md`](SEMANTIC-PATCH-EVIDENCE-V1.md). Generation
and verification must independently rebuild unchanged Review v1, preserve its
bytes/KATs and lack of `review::verify`, reject receipt/capsule confusion, own
single bounded patch/evidence reads, and recheck exact bounded source
identity/bytes/revision. Parsed declarations, callables, and call sites must
reject before HIR at their exact boundaries.

The `patch-with-evidence` gate must prove lock acquisition occurs before patch
or evidence input reads/replay work, exact replay occurs before stage preparation,
Patch v1/v2/v3 candidates match ordinary A0, replay-after-commit rejects,
mismatch and receipt substitution create no stage, source drift and
same-bytes identity replacement reject at every A0 boundary, both final source
reads remain bounded, permissions are preserved, stage mutation/rename failure
cleans only owned staging, and a foreign stage replacement is never deleted.
Ordinary `patch` bytes and behavior must remain unchanged.

Run the focused gates:

```sh
cargo test --locked -p semaprax --all-features --test semantic_patch_evidence_v1
cargo test --locked -p semaprax --all-features --lib patch_evidence::tests
```

A+B generation/verification is 11/11 integration plus 5/5 internal units;
Phase C apply is 16/16 integration plus 11/11 hook/limit units. Library
420/420, doctest 37/37, full workspace/release/rustdoc/strict-Clippy/format/
diff/preservation, and independent security are locally green. The exact
`34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted green
in [run
31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
including [Ubuntu job
93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949),
[macOS job
93596706897](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706897),
[Windows job
93596706899](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706899),
[MSRV job
93596707079](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596707079),
[dependency-policy job
93596706994](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706994),
and [component job
93596706902](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706902);
all 12 jobs passed. The earlier `e04c2c9` run failed only the Rust 1.97 lint
and is not green evidence. Capsule nonclaims for provenance, approval, target/
test execution, reusable authorization, general proof, Context/repository/
multi-file scope, persistence, consumer compatibility, and semantic widening
remain blocking boundaries.

- A design document proves intent, not implementation.
- A compiler unit test proves only the covered semantic case.
- A generated artifact proves emission, not successful loading or execution.
- One host cannot prove cross-platform support.
- A sample UI cannot prove accessibility or native lifecycle integration.
- A completion-matrix row changes to **Implemented** only when its entire stated gate is exercised.

Never delete, loosen, skip, or platform-disable a relevant test without documenting the invalidated evidence and replacing it with an equally strong gate.
