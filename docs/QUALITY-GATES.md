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
cargo run --locked -p semaprax -- fmt examples/meaning.spx --check
cargo run --locked -p semaprax -- fmt examples/effects.spx --check
cargo run --locked -p semaprax -- fmt examples/ownership.spx --check
cargo run --locked -p semaprax -- fmt examples/lifecycle.spx --check
cargo run --locked -p semaprax -- fmt examples/control_flow.spx --check
cargo run --locked -p semaprax -- fmt examples/records.spx --check
```

`scripts/quality.sh` runs this baseline on Unix. Tests and documentation always
enable every workspace feature so an internal or staged production surface
cannot escape execution merely because it is not a default feature. The
integration suite also discovers every committed `.spx` example and requires it
to verify and exactly equal its canonical projection. CI runs the equivalent
matrix on Linux, macOS, and Windows, plus an explicit Rust 1.85 minimum-version
check with the same all-feature policy.

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
derivation, atomic ledger/replay tests, and the bounded scalar-discard/owned-
identity joint provider -> loader -> host test. Documentation must still deny
all-14 coverage through loader plus host, canonical pre-execute unwind recovery,
post-`CallCommit` allocation-failure safety, exhaustive physical failure
injection, hosted v3 sanitizer and Windows evidence until their configured jobs
pass publicly, Android/iOS, quiescence, malicious-code containment, public
admission, and any `SPX-B104` change.

| Change | Required evidence |
| --- | --- |
| Syntax or AST | Canonical parse-format-parse round trip and malformed-input diagnostic |
| Types or ownership | Positive test, compile-fail diagnostic code, prefix/sibling behavior, divergent-branch join, borrow/shared transfer boundary, and hostile-HIR replay |
| Resolved HIR | Stable-ID rename/whitespace invariance, unique lexical/result/place/field IDs, recursive type-fact/layout assertions, record constructor/projection integrity, invalid-AST rejection, move/effect/contract revalidation, and malformed-HIR native/Wasm rejection parity |
| Cleanup inventory | Exact recomputation from validated HIR, own-versus-borrow/shared entry evidence, nested field/lifecycle/flag assertions, deterministic storage discovery, and hostile inventory rejection before native/Wasm feature gates |
| Cleanup plan | Independent canonical rebuild plus attached-plan structural and all-path state replay after core-HIR/inventory validation; scalar and owned result commits; every checked/contract failure source; branch/lazy/region exits; caller-owned argument epochs and atomic commit; partial/nested record order; deterministic Graph snapshot; scenario-driven exact semantic trace; and hostile plan rejection as `SPX-H006` before native/Wasm feature gates |
| Status, conformance trace, or trace-path certificate | Reserved compiler-status forgery rejection, nonzero imported codes, exact 1–255 UTF-8-byte/no-NUL domain boundaries across source/HIR/targets, context/arena token isolation, immutable one-based records, deterministic exact JSON, nested write-once failure selection, success-only finalizer completion, no physical target data, missing/extra scenario rejection, canonical certificate/fingerprint known answers, exact accepted outcome paths, allocation-free host DFA walking, omitted/duplicate/reordered event rejection, and an explicit migration note |
| Effects or contracts | Capability/effect rejection and runtime/backend behavior |
| Graph schema | Exact schema assertion/snapshot, canonical SHA-256 known answer, resolved-reference integrity, stable-ID collision behavior, and bounded context frontier behavior |
| Public protocol or schema | Compatibility fixture, or explicit version bump with migration and changelog note |
| Semantic patch | Successful atomic edit, returned-revision equality, syntax-position collision fixture, stale SHA and legacy-token rejection, failed-edit no-change proof |
| Runtime semantics | Native and Wasm result/trap equivalence, deterministic evaluation order |
| Backend | Host artifact execution, stable failure behavior, exact status mapping, poisoned out-slot preservation on failure, source-order propagation, strict generated-code warnings, cross-platform CI, and sanitizer evidence when memory/ownership lowering changes |
| Interop or package | Bidirectional conformance fixture, ownership/error mapping, reproducible artifact |
| UI/platform | Accessibility checks, lifecycle/capability tests, representative simulator/device or host evidence |
| Security boundary | Threat-model delta, hostile input test, no newly ambient capability |
| Native capability token | Published HMAC KAT, independently reproduced owner and result complete-token goldens, exact length/layout/endian assertions, every-bit, arbitrary-byte, and length-boundary mutation, closed kind/reserved/zero-field parsing, cross-secret/module/adapter/epoch/template/type/lifecycle/thread-policy/thread-binding rejection, owner-versus-result scope, stale/max-generation evidence, pinned audited crypto dependency, and proof that compiler preflight creates no authority |
| Native capability authority | Exactly pinned OS-random dependency and locked checksum, one exact fill with no fallback/retry, partial-error and every structural-zero rejection, invalid-binding-or-draining-lease before entropy proof, test-only deterministic injection, independently reproduced complete owner/result goldens, lease-derived physical fingerprint, every sealed context delta, wrong-thread-first owner/result mint/authentication, same-thread recovery, exact-instance wrapper authentication, owner/result lease retention after authority drop, explicit `Send + Sync` policy, non-formatting credential wrapper, stable error redaction, native OS smoke on every desktop CI host, Rust 1.85, and unchanged private/export/`SPX-B104` boundaries |
| Native module lease topology | Fake-backed unit evidence still proves equal-fingerprint instance nonconflation, exact retention, drain ordering, cross-instance rejection despite equal bytes, exactly-once fake release, and no cycles; the real physical-host gate below must independently prove the corresponding loader-backed lifetime rather than treating the fake as physical evidence |
| Native settlement model | Fixed resource/checkpoint/progress/work ceilings; nonzero recovery-contract and nonempty/no-NUL function identity; one all-live post-`CallCommit` start; dense checkpoints; canonical typed progress; exact cleanup permutations; trace-bound terminal outcomes; exhaustive abort and accepted-outcome owner-state enumeration through six resources; executable post-`CallCommit` `Executing`, exact `DecisionLocked`, `ActionInProgress`, `ProviderSettled`, model-`ReceiptCommitted`, and absorbing `Quarantined` phases; pre-decision unwind selecting `Abort(HostUnwind)` and post-decision unwind resuming the exact locked decision; unknown/conflicting phase quarantine; `Finalizing` recorded before effect, `Dead` only after normal return, and interruption quarantine with no retry; provider `Published` distinguished from public ledger publication; unique receipt-commit eligibility only after candidate validation; same-decision idempotence with no repeated actions; stale/cross-bound/skipped/reordered/duplicate candidate rejection; exact independently reconstructed receipts; zero active finalizers and terminal dispositions; deterministic domain-separated certificate/receipt fingerprints and migration note; hostile structural/progress/receipt mutations; quarantine evidence retention; start-only nonmutating progress walks; non-`Clone`, non-formatting frame compile-fail tests; and an executable nonclaim that model preparation is not invocation/module/frame-generation reservation, allocation-free physical execution, physical-finalizer authority, host authentication, or ledger mutation authority |
| Native callable settlement proof v1 | Private `SPXNPRF1` only; exact unchanged callable-v2 bytes plus canonical pointer-free binary graph in one immutable envelope; 64 KiB global ceiling enforced during compiler serialization and before host allocation; independent encoder/parser and canonical re-encoding; acyclic domain-separated schema/v2/graph/envelope fingerprints; exact source-v2 call-contract and trace-certificate cross-binding; dense checkpoints, one all-live start, reachability, forward DAG, typed transition and cleanup-order continuity validation; all 14 corpus cases; fixed known answer; every prefix, trailing byte, and single-byte mutation; rehashed hostile graph, unknown tag, invalid text, hostile count, cross-module, and changed-trace rejection; v2 loader pre-open rejection; default-consumer compile-fail; unchanged v2 bundle/provider/CLI and `SPX-B104`; and explicit no-provenance, no-authority, no-descriptor-v3, no-provider, no-runtime-settlement nonclaims |
| Native callable ABI v3 descriptor/wires | Private `SPXNABI3` only; exact descriptor/signature/graph plus independently implemented compiler/host codecs for `SPXNRQ03`, `SPXNEX03`, `SPXNFR03`, `SPXNDC03`, `SPXNAC03`, `SPXNCR03`, and host-only `SPXHRP03`; frozen 20-byte envelopes, six-argument execute ABI, payload-bearing frame cells, closed tags/phases/dispositions, checked capacities, request/response/semantic/decision/action/frame/candidate digest DAG, separate-key receipt HMAC, canonical re-encoding and changed private known answers; every-prefix/trailing/every-byte mutation, hostile tag/count/cap/cross-binding/replay, v1/v2/proof/v3 confusion, unchanged v2/proof KATs, default-consumer hiding, and both legacy loaders rejecting v3 before path/image access; distinct dynamic/iOS-static metadata and iOS device/simulator/macabi target identities. Graph-derived providers execute all 14 normal scenarios at O0/O2, while one narrower generated-provider/loader/host path covers scalar discard-two and owned identity. Pending/pre-execute unwind fails closed and postcommit Rust decoding/replay remains allocating; static/mobile admission, the remaining joint corpus, exhaustive failure evidence, public execution, and any `SPX-B104` change remain closed |
| Native callable bundle | Public preflight requires an explicit persistent function ID and at least one direct `own` trivial-resource parameter; deterministic descriptor/provider/dictionary/certificate derivation and preflight SHA-256; strict host Clang shared-library build; exact regular-file-only staged/final inventory with Windows import-library suppression and checked removal of the linker's `.exp` side artifact; canonical sorted per-payload SHA-256 manifest plus manifest checksum; byte-identical double build on each host; build-only API/CLI with no load/invoke/adopt/authority surface; default-feature external-consumer compile-fail proof for host internals; ordinary native `SPX-B104`; missing/automatic/excluded-function diagnostics; observed file/directory/dangling-symlink no-overwrite; failed-Clang staging cleanup; canonical example and green [Ubuntu](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277094), [macOS](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277081), and [Windows](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277085) hosted-CI builds. The canonical output parent is trusted against concurrent adversarial mutation because portable `std` lacks atomic directory rename-no-replace |
| Rust native-host ASan | Dedicated Ubuntu 24.04 job; every Rust command routed through the exact pinned nightly and compiler commit; audited Clang 18 major for the executable and generated providers; `cfg(sanitize = "address")` compile-time probe; intentional Rust heap-use-after-free diagnostic; rebuilt target standard library; verbose host-crate `rustc` flag proof; ASan symbols in the host binary and callbacks in the generated provider; and real callable-host plus authoritative-corpus execution. The [fail-closed contract and exact green public job](RUST-HOST-SANITIZERS.md) must remain intact; configuration or static tests alone never count as runtime evidence |
| Native loader quarantine | Separate unpublished crate; main crate still forbids unsafe; exact dependency pin and workspace supply-chain gate; separately documented descriptor-only, callable-v2, and settlement-v3 unsafe constructors; no generic lookup/raw handle/pointer/manual close; canonical-path, symbol, descriptor, and wire bounds; null and exact-byte checks; legacy constructors reject `SPXNABI1`, `SPXNPRF1`, and full-magic `SPXNABI3` as applicable before loading; Unix `RTLD_NOW | RTLD_LOCAL`; Windows root/default-safe dependency search without current-directory/legacy-PATH lookup. The private v3 constructor retains an immutable exact descriptor copy, proves getter/execute/settle/returned-descriptor-address root-image provenance, preallocates five exact descriptor-projected buffers, and exposes instance-bound one-shot execute/settle preparations; compile-fail gates enforce `!Send`, `!Sync`, non-`Clone`, and non-formatting wrappers. Real Linux/macOS fixtures cover positive, rejection, dependency provenance, same-capacity descriptor substitution, cross-instance, last-reference, and unload behavior; Windows equivalents compile for CI. Malicious-code containment, file/code identity, immediate unmapping, observed Windows v3 runtime, iOS static registration, Android, quiescence, fork safety, and public admission remain explicit nonclaims |
| Native physical ownership host | Separate unpublished crate and unchanged compiler `unsafe_code = "forbid"`/`SPX-B104`; strict descriptor-v1 and independent descriptor-v2 decoding; every-byte/truncation/trailing mutation rejection; compiler-authenticated dictionary plus trace-path certificate; strict allocation-free postcommit response decoding in callable v2; exact loader-instance callable binding; OS-seeded same-thread authority; credential-to-ledger resource/lifecycle/slot/generation checks; non-mutating fully allocated plans and atomic commit; unsafe trusted adoption; noncopying/nonformatting/`!Send`/`!Sync` owners; reusable owners after precommit rejection; scalar/owned success, normalized semantic/adapter failure, generation rotation, draining, lease retention, and cross-instance rejection; real generated O0/O2 shared libraries executing the full callable-v2 14-case reference corpus through the host; green public Linux generated-provider ASan+UBSan, Rust-host ASan, and Windows callable/dependency-isolation evidence; public build-only bundle emission. Private callable-v3 evidence includes independent candidate replay, exact descriptor/instance/frame binding, separate one-fill receipt authority and HMAC KATs, authoritative duplicate/stale/cross-frame rejection, allocation-free `CallCommit`, atomic publication with refreshed generation, exact cached replay, panic-safe RAII evidence/pin quarantine, and one joint scalar-discard/owned-identity provider-loader-host path. Dedicated all-14 provider sanitizer and explicit Windows gates are configured but require a green hosted run. Keep `SPX-B104` closed until all 14 scenarios cross the joint path, canonical pre-execute unwind, allocation-failpoint and exhaustive physical failure/crash evidence, observed v3 sanitizer/Windows runtime, quiescence, Android/iOS, and public execution/admission are proven |
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
two-fixture joint provider/loader/host test, while the Linux lane requires the
v3 provider corpus under ASan+UBSan and the joint path in the dynamically
loaded sanitizer job. These new gates are configured evidence, not observed
hosted evidence until the corresponding public run is green.

## Evidence strength

- A design document proves intent, not implementation.
- A compiler unit test proves only the covered semantic case.
- A generated artifact proves emission, not successful loading or execution.
- One host cannot prove cross-platform support.
- A sample UI cannot prove accessibility or native lifecycle integration.
- A completion-matrix row changes to **Implemented** only when its entire stated gate is exercised.

Never delete, loosen, skip, or platform-disable a relevant test without documenting the invalidated evidence and replacing it with an equally strong gate.
