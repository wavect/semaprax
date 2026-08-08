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
| Native loader quarantine | Separate unpublished crate; main crate still forbids unsafe; exact dependency pin and workspace supply-chain gate; separately documented descriptor-only and callable-v2 unsafe constructors; no generic lookup/raw handle/pointer/manual close; canonical-path, getter/callable-symbol, descriptor, and request/response bounds; null and exact-byte checks; `SPXNABI1` rejection before callable loading; Unix `RTLD_NOW | RTLD_LOCAL`; Windows root/default-safe dependency search without current-directory/legacy-PATH lookup; eager exact callable lookup; one-shot instance-bound prepared calls; compile-fail `!Send`, `!Sync`, non-`Clone`, and non-formatting lease/call checks; real Linux/macOS runtime-loaded positive, rejection, last-reference, and private-host retention fixtures; explicit malicious-code, code-identity, same-image-provenance, immediate-unmapping, Windows dependency-collision runtime, iOS/Android, callable safety, quiescence, fork, and public-adapter nonclaims |
| Native physical ownership host | Separate unpublished crate and unchanged compiler `unsafe_code = "forbid"`/`SPX-B104`; strict descriptor-v1 and independent descriptor-v2 decoding; every-byte/truncation/trailing mutation rejection; compiler-authenticated dictionary plus trace-path certificate; strict allocation-free postcommit response decoding; exact loader-instance callable binding; OS-seeded same-thread authority; credential-to-ledger resource/lifecycle/slot/generation checks; non-mutating fully allocated plans and atomic commit; unsafe trusted adoption; noncopying/nonformatting/`!Send`/`!Sync` owners; reusable owners after precommit rejection; scalar/owned success, normalized semantic/adapter failure, generation rotation, draining, lease retention, and cross-instance rejection; real generated O0/O2 shared libraries executing the full 14-case reference corpus through the host; green public Linux dynamically loaded generated-provider ASan+UBSan evidence. Keep `SPX-B104` closed until general physical/malformed-response fallback cleanup and quiescence, Rust-host sanitizer instrumentation, green public Windows runtime/dependency-collision evidence, Android/iOS profiles, and public compiler build/preflight emission are proven |
| Wasm owned ABI v1 | Exactly one direct trivial-resource identity and stable `SPX-W111` exclusions; replay-validated plan admission and hostile-plan `SPX-H006`; deterministic Wasm bytes, embedded exact SHA-256 artifact authentication before host construction, export metadata, `semaprax.web.v3` manifest mapping, and scalar-lane regression; private host imports; exact export/count/canonical-i64/positive-i32/result-kind validation; one-shot branded adoption with reuse rejection and nonmutation on capacity failure; instance-tagged slot/generation stale, replay, duplicate, same-module, and duplicated-generated-module same-realm cross-instance rejection plus invalid/repeated allocator fail-closed tests; complete 4/8-byte precommit out-range/alignment validation; one reserved status cell plus resolvable exhaustion sentinel; exact canonical `semaprax.status.v1` field shape; atomic owner staging and reusable owners after rejection; reverse exact-once cleanup and equal-payload exact-owner selection; owned-result rotation; requires/overflow/ensures failure with poison preservation; result/status/slot exhaustion at bounded test limits; real Node execution; compiler-generated semantic ordinals authenticated by the deterministic dictionary and materialized to exact reference traces for the shared 14-case corpus. A trusted unpoisoned same-realm host environment, cross-realm/worker identity, Components, imports/finalizers, async/shared memory, and production callable-native equivalence remain explicit boundaries/nonclaims |
| Owned resource vertical slice | The complete [v1 contract](OWNED-RESOURCE-VERTICAL-V1.md): structural narrow-slice admission; production-reachable compiler and host APIs; indivisible parameter-ordered owner commit; reusable owners after rejection; exact-once ordinary/result/failure cleanup; result-aware finalization; native unsafe admission and adoption quarantine; same-thread exact loader retention and draining; instance-bound Wasm `(slot, generation)` handles; exact native/reference/Wasm status/publication/trace equality for the full corpus; real Linux/macOS/Windows native calls and Node/Wasm execution; O0/O2 and mandatory ASan/UBSan; deterministic C/descriptor-v2/Wasm bytes and symbols; exhaustion/wraparound, hostile-boundary, compile-fail trait, MSRV, package, and dependency-policy gates; unchanged fail-closed diagnostics for every excluded shape |

The gated native cleanup corpus runs at O0 and O2 everywhere Clang is available. Linux CI additionally sets `SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1`, which makes separate ASan and UBSan compile/run support mandatory rather than allowing capability-based skips.

The dedicated Linux
[`callable-host-sanitizers` job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green: all 14 authoritative O0/O2 cases executed from dynamically loaded
generated providers with combined ASan/UBSan instrumentation through the Rust
host. Its linker flags made the sanitizer runtimes available to that executable;
they did not instrument the Rust host code. Rust-host sanitizer instrumentation
remains a separate gate. The dependency-policy job in the same run was green,
but unrelated Clippy/GCC failures stopped the platform jobs before runtime
evidence and kept the workflow run as a whole red. This is job evidence rather
than overall-CI or platform-matrix evidence.

Authority goldens prove deterministic mechanics, and OS smoke proves only that
the supported host source was available. The private physical-host fixtures now
add exact loader-backed retention, ledger replay rejection, same-thread owner
topology, and generated callable execution. The shared 14-case corpus proves
exact reference/native-host-O0/O2/Wasm semantic trace and outcome equality for
the narrow admitted shape. None of this proves mathematical uniqueness, key
secrecy after compromise, fork safety, general postcommit fallback cleanup or
quiescence, green public Windows runtime/dependency isolation, sanitizer
instrumentation of the Rust host,
Android/iOS admission, or public native backend conformance.

## Evidence strength

- A design document proves intent, not implementation.
- A compiler unit test proves only the covered semantic case.
- A generated artifact proves emission, not successful loading or execution.
- One host cannot prove cross-platform support.
- A sample UI cannot prove accessibility or native lifecycle integration.
- A completion-matrix row changes to **Implemented** only when its entire stated gate is exercised.

Never delete, loosen, skip, or platform-disable a relevant test without documenting the invalidated evidence and replacing it with an equally strong gate.
