# Quality gates

Quality gates are executable evidence, not a checklist substitute for reasoning. Every pull request must pass the baseline and the gates for each changed semantic layer.

## Baseline

Run from the repository root:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
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

`scripts/quality.sh` runs this baseline on Unix. The integration suite also discovers every committed `.spx` example and requires it to verify and exactly equal its canonical projection. CI runs the equivalent matrix on Linux, macOS, and Windows, plus an explicit Rust 1.85 minimum-version check.

CI also runs an immutable-SHA-pinned `cargo-deny` policy over the complete locked
target graph. Advisories, unapproved licenses, duplicate/wildcard versions, Git
dependencies, and registries other than crates.io are blocking failures;
`deny.toml` contains no advisory exceptions.

On PowerShell, run the documentation gate as:

```powershell
$env:RUSTDOCFLAGS = '-D warnings'
cargo doc --locked --no-deps
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
| Status or conformance trace | Reserved compiler-status forgery rejection, nonzero imported codes, exact 1–255 UTF-8-byte/no-NUL domain boundaries across source/HIR/targets, context/arena token isolation, immutable one-based records, deterministic exact JSON, nested write-once failure selection, success-only finalizer completion, no physical target data, missing/extra scenario rejection, and an explicit migration note |
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
| Native module lease topology | Test-only construction; zero fingerprint/process/incarnation rejection; equal-fingerprint instance nonconflation; exact-instance explicit retention; process/incarnation rejection without state change; retain-versus-drain gate; existing-pin survival after drain; authority plus owner/result wrapper retention across drop orders; cross-instance rejection even when token bytes match; exactly-once fake release including concurrent final drops; no retention backedge; deliberate `Send + Sync` and non-`Clone`/non-formatting traits; no production constructor, platform loader, physical-lifetime claim, export, or `SPX-B104` change |
| Native loader quarantine | Separate unpublished crate; main crate still forbids unsafe; exact dependency pin and workspace supply-chain gate; one documented unsafe constructor; no generic lookup/raw handle/pointer/symbol/manual close; canonical-path, symbol, and descriptor bounds; null and exact-byte checks; logical-admission identity; compile-fail `!Send`, `!Sync`, non-`Clone`, and non-formatting lease checks; real Linux/macOS runtime-loaded positive, rejection, and last-reference fixtures; explicit malicious-code, code-identity, same-image-provenance, immediate-unmapping, hardened-Windows/iOS/Android, quiescence, fork, and public-adapter nonclaims |

The gated native cleanup corpus runs at O0 and O2 everywhere Clang is available. Linux CI additionally sets `SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1`, which makes separate ASan and UBSan compile/run support mandatory rather than allowing capability-based skips.

Authority goldens prove deterministic mechanics, and OS smoke proves only that the supported host source was available. Neither proves mathematical uniqueness, key secrecy after memory compromise, replay prevention, fork safety, module lifetime, or callable resource safety; those claims require their own executable gates before private staging can become a public adapter.

## Evidence strength

- A design document proves intent, not implementation.
- A compiler unit test proves only the covered semantic case.
- A generated artifact proves emission, not successful loading or execution.
- One host cannot prove cross-platform support.
- A sample UI cannot prove accessibility or native lifecycle integration.
- A completion-matrix row changes to **Implemented** only when its entire stated gate is exercised.

Never delete, loosen, skip, or platform-disable a relevant test without documenting the invalidated evidence and replacing it with an equally strong gate.
