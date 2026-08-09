# Quality gates

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
The Windows desktop package/runtime remains pending in [run
31339938860](https://github.com/wavect/semaprax/actions/runs/31339938860).

| Change | Required evidence |
| --- | --- |
| Syntax or AST | Canonical parse-format-parse round trip and malformed-input diagnostic; record update must preserve base-first and authored replacement order |
| Types or ownership | Positive test, compile-fail diagnostic code, prefix/sibling behavior, divergent-branch join, borrow/shared transfer boundary, and hostile-HIR replay |
| Resolved HIR | Stable-ID rename/whitespace invariance, unique lexical/result/place/field IDs, recursive type-fact/layout assertions, record constructor/projection/update integrity and ordering, invalid-AST rejection, move/effect/contract revalidation, and malformed-HIR native/Wasm rejection parity |
| Aggregate layout | Exact Native64/Wasm32 size/alignment/offset KATs for nested admitted fields; frozen one-byte/alignment-one empty records with explicit C padding and validated Wasm bytes; checked arithmetic; stable declaration-order field identity; reorder/overlap/undersize/misalignment/overflow mutation rejection; and no inference of a public ABI from an internal profile |
| Cleanup inventory | Exact recomputation from validated HIR, own-versus-borrow/shared entry evidence, nested field/lifecycle/flag assertions, deterministic storage discovery, and hostile inventory rejection before native/Wasm feature gates |
| Cleanup plan | Independent canonical rebuild plus attached-plan structural and all-path state replay after core-HIR/inventory validation; scalar and owned result commits; every checked/contract failure source; branch/lazy/region exits; caller-owned argument epochs and atomic commit; partial/nested record order; immutable-update base-first/authored-replacement order, untouched-field transfer, displaced-field reverse exact-once cleanup; deterministic Graph snapshot; scenario-driven exact semantic trace; and hostile plan rejection as `SPX-H006` before native/Wasm feature gates |
| Status, conformance trace, or trace-path certificate | Reserved compiler-status forgery rejection, nonzero imported codes, exact 1–255 UTF-8-byte/no-NUL domain boundaries across source/HIR/targets, context/arena token isolation, immutable one-based records, deterministic exact JSON, nested write-once failure selection, success-only finalizer completion, no physical target data, missing/extra scenario rejection, canonical certificate/fingerprint known answers, exact accepted outcome paths, allocation-free host DFA walking, omitted/duplicate/reordered event rejection, and an explicit migration note |
| Effects or contracts | Capability/effect rejection and runtime/backend behavior |
| Graph schema or agent context | Exact schema assertion/snapshot, canonical SHA-256 known answer, resolved-reference integrity, stable-ID collision behavior, exact byte/node accounting, ordered truncation reasons and omitted counts, re-expandable frontier replay, source-revision binding, exact supported/unavailable filters, deterministic replay, and unknown/duplicate/malformed CLI rejection |
| Agent economics or quality routing | Strict canonical, exact-case, separator-normal, Windows-forbidden/reserved-name-safe, non-symlink offline manifest/source containment; unsupported-facet rejection; exact manifest/context SHA-256 plus relevant/evidence ID bindings and goldens; byte/node/non-model lexical-unit accounting; reviewed relevance/evidence scoring; source mutation and deterministic replay; independent JSON parse; quick advisory boundary; explicit or unique target-remote `merge-base --all` ancestor-to-HEAD plus dirty Git-state reconciliation with non-UTF-8/zero/multiple-base rejection and optional exact path-set proof; hostile alias rejection; broad graph/CLI and unknown/wide/router paths escalating to full; exact profile-specific ordered gate-plan validation; and executed missing/duplicate/reordered/wrong-profile dispatcher rejection |
| Public protocol or schema | Compatibility fixture, or explicit version bump with migration and changelog note |
| Semantic patch | Successful atomic edit, returned-revision equality, syntax-position collision fixture, stale SHA and legacy-token rejection, failed-edit no-change proof |
| Runtime semantics | Native and Wasm result/trap equivalence, deterministic evaluation order |
| Backend | Host artifact execution, stable failure behavior, exact status mapping, poisoned out-slot preservation on failure, source-order propagation, strict generated-code warnings, cross-platform CI, and sanitizer evidence when memory/ownership lowering changes |
| Aggregate backend | Public nested `i64`/`bool` construction/projection/update executes through strict native C11 at O0/O2 and real Node/Wasm; exact base-first/authored replacement failure selection; internal aggregate pointer parameters and caller-owned results; poisoned result preservation; deterministic layout assertions; empty-record one-byte/alignment-one parity; repeated same-instance Wasm calls with shadow-stack restoration; and exact Native/Wasm result equivalence. Resource-bearing evidence remains a private test-only scenario projected from the same authenticated cleanup plan into C O0/O2 and real Wasm, with exact common finalization trace, poison, zero final liveness, hostile-action rejection, and unchanged public `SPX-B104`/`SPX-W111`, callable/component signature, and stable-ABI gates |
| Interop or package | Bidirectional conformance fixture, ownership/error mapping, reproducible artifact |
| Private native desktop UI | Existing private desktop engine remains feature-gated/unpublished and `SPX-B104` stays closed; exact AppKit and Win32 source; one visible titled native window and labeled native button; platform accessibility-name query; delayed OS control event through the real event loop; canonical SHA-256 manifest and exact engine-byte verification before launch; executable-preserving mismatch rejection before result publication; exact engine subprocess output; bounded AppKit deadline/terminate/kill with a digest-valid hanging-engine regression; ordered close/terminate evidence; success-file publication only after termination; pinned compiler/linker/SDK/import-library roots; byte-identical double UI build; foreground macOS `APPL` plus exact framework/load/export/build-version/inventory checks; x64 PE32+ GUI subsystem plus exact seven-DLL import set, absent export directory including ordinal-only functions, path/manifest/inventory checks; hostile source-lock removal; mandatory hosted macOS and Windows launch; explicit no-signed-provenance/co-replacement defense, language-UI/state/layout, SwiftUI/WinUI, general accessibility/lifecycle, signing/installer/distribution, or public-admission claim |
| UI/platform | Accessibility checks, lifecycle/capability tests, representative simulator/device or host evidence |
| Private native desktop app | Default-off feature and unpublished host; exact generated callable-v3 provider/descriptor; pinned and asserted Rust/LLVM/Clang plus Apple ld/macOS SDK build or MSVC linker/Windows SDK import-library identity; Cargo offline; two independent byte-identical builds within that exact toolchain (not a cross-toolchain/SDK claim); stable package-relative macOS install identity; canonical content-derived `LC_UUID`, two independently assembled byte-identical signed app bundles, timestamp-free fixed-identifier ad-hoc signatures, strict bundle verification, and no distribution credential; exact Mach-O or PE/COFF architecture/file-kind/load/import/export/path checks; canonical macOS `APPL` or effective Windows `asInvoker` manifest and exact package inventory; hostile inspection/source-lock regressions; two authenticated owned publications with refreshed-generation reuse; exact cached replay; no network; mandatory hosted macOS and Windows launch; explicit no-window/UI/accessibility/lifecycle/installer/public-admission and unchanged `SPX-B104` claims |
| Private Android JNI/Kotlin APK | Feature remains private and `SPX-B104` stays closed; exact four-output target-matched generator; strict NDK x86_64/arm64 provider/JNI compilation; exact shim export/dependency/path inspection; independent handle/status KATs; HandlerThread-only host access; explicit `consume()` restore-only-on-defined-precommit semantics; non-throwing `AutoCloseable.close()`/`PhantomReference` fallback; identical deterministic Cleaner action; exception clear/normalization; poisoned-output preservation; exact O0/O2 finalizers/receipt/publication/allocation/handle-table result; plugin-free repository-free Gradle 9 `--offline` packaging from pinned runner tools; exact APK inventory/signature/alignment; clean install; API-35 x86_64 framework-Instrumentation execution; and exact app-private result. The bounded gate is green in [run 31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206); arm64 remains compile/inspect only |
| Private Apple Swift ownership app | Feature and C/Rust modules remain private and `SPX-B104` stays closed; exact device-arm64, Simulator-arm64, and Simulator-x86_64 descriptor/provider pairing; fixed hidden evidence hooks with no legacy caller-configurable raw open; stable Swift `Thread` FIFO under complete Swift 6 concurrency checking; handle/status KATs; poison-preserving outputs; explicit consume and identical deterministic ARC-deinit cleanup; stale/forged/wrong-thread, live-close, race, repeated-call/reset, and no-retry checks; exact O0/O2 finalizers/receipt/publication/ledger/allocation/empty-table result; strict C warnings; device and universal-Simulator XCFramework inspection; ad-hoc signing; installed arm64-Simulator app execution; and exact app-container result. The bounded gate is green in [run 31338834586, job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228); physical-device, public-framework, and general lifecycle claims remain closed |
| Private WIT/component boundary v1/v2/v3 | Default-off feature plus external default-consumer compile-fail; exact `SPXWIT01` framing and SHA-256 KAT; deterministic WIT, canonical mapping JSON, and JavaScript adapter; every-byte/truncation/trailing/magic rejection; own-data snapshot normalization with accessor and changing-proxy regressions; lossless UTF-8 including lone-surrogate rejection, valid-pair acceptance, exact 255-byte ceiling, no NUL, and exact `semaprax.status.v1` u32 bounds; standalone v1 scalar component; checked v2 composition of the unmodified generated scalar core and frozen runtime; private read-only digests; JavaScript digest derivation from exact bytes and forged-metadata rejection; independent exact-profile parsers; exact `wasmparser = 0.255.0` validation; rehashed invalid signature/body/cardinality/canonical-lift cross-type rejection; canonical-LEB/every-byte/truncation/trailing closure; immutable input snapshots; and Node v1/v2 execution. Portable Result Component v3 must bind the exact generated status/out core/profile/source/component KATs to `result<s64, status>`, pass the independent and upstream validators, preserve poison/sticky status at the generated-core boundary, execute the add/subtract/multiply/divide/remainder/negate status matrix, and locally execute typed Wasmtime success/add-overflow/division-by-zero/precondition/postcondition plus competing add-overflow/division-by-zero twice on one and fresh instances. Its standalone exact Rust 1.97.1/Wasmtime 47.0.3 graph must remain outside the root workspace with zero imports, an empty linker, no WASI/host callbacks, locked offline format/lint/test/run after explicit root and isolated dependency-acquisition phases, fail-closed dependency policy, and engine failures out of band. Hosted evidence counts only after the configured Ubuntu job is green. Explicitly no source-language `Result`/`Option`, records/resources/imports, async, capabilities, multi-engine/browser conformance, public API, or `SPX-B104`. Focused commands: `cargo test --locked -p semaprax --all-features --lib wit_component::tests::` and `cargo test --locked -p semaprax --test component_runtime_ci_contract` |
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

## Evidence strength

- A design document proves intent, not implementation.
- A compiler unit test proves only the covered semantic case.
- A generated artifact proves emission, not successful loading or execution.
- One host cannot prove cross-platform support.
- A sample UI cannot prove accessibility or native lifecycle integration.
- A completion-matrix row changes to **Implemented** only when its entire stated gate is exercised.

Never delete, loosen, skip, or platform-disable a relevant test without documenting the invalidated evidence and replacing it with an equally strong gate.
