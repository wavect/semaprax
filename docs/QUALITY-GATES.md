# Quality gates

Quality gates are executable evidence, not a checklist substitute for reasoning. Every pull request must pass the baseline and the gates for each changed semantic layer.

## Baseline

Run from the repository root:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
cargo build --locked --release
cargo package --locked --allow-dirty
cargo run --locked -- check examples/meaning.spx
cargo run --locked -- check examples/ownership.spx
cargo run --locked -- check examples/lifecycle.spx
cargo run --locked -- check examples/control_flow.spx
cargo run --locked -- check examples/records.spx
cargo run --locked -- fmt examples/meaning.spx --check
cargo run --locked -- fmt examples/effects.spx --check
cargo run --locked -- fmt examples/ownership.spx --check
cargo run --locked -- fmt examples/lifecycle.spx --check
cargo run --locked -- fmt examples/control_flow.spx --check
cargo run --locked -- fmt examples/records.spx --check
```

`scripts/quality.sh` runs this baseline on Unix. The integration suite also discovers every committed `.spx` example and requires it to verify and exactly equal its canonical projection. CI runs the equivalent matrix on Linux, macOS, and Windows, plus an explicit Rust 1.85 minimum-version check.

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

The gated native cleanup corpus runs at O0 and O2 everywhere Clang is available. Linux CI additionally sets `SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1`, which makes separate ASan and UBSan compile/run support mandatory rather than allowing capability-based skips.

Token-codec goldens prove deterministic mechanics only. They do not prove key entropy, runtime unforgeability, replay prevention, module lifetime, or callable resource safety; those claims require their own executable gates before the codec may leave private staging.

## Evidence strength

- A design document proves intent, not implementation.
- A compiler unit test proves only the covered semantic case.
- A generated artifact proves emission, not successful loading or execution.
- One host cannot prove cross-platform support.
- A sample UI cannot prove accessibility or native lifecycle integration.
- A completion-matrix row changes to **Implemented** only when its entire stated gate is exercised.

Never delete, loosen, skip, or platform-disable a relevant test without documenting the invalidated evidence and replacing it with an equally strong gate.
