# ADR 0008: Hold RI-14 stable Rust-source lowering at an executable evidence gate

Audience: maintainers evaluating [RI-14](https://github.com/wavect/semaprax/issues/372).

- Status: **no-go for backend promotion** after the local executable trial on
  3 October 2026. The narrow owned-Bytes island passed; the experiment did not
  establish the broader semantic parity or a measured engineering benefit.
- Scope: the feature-gated `stable_rust_lowering` seam and
  `experiments/ri-14-stable-rust-lowering`. It does not alter RI-01's stable
  interop bridge, default compiler targets, or any completion-matrix status.

## Candidate under evaluation

The candidate admits only one non-inert checked-HIR shape:

```text
@id("ri14.transfer.identity")
fn identity(value: own Bytes) -> Bytes { value }
```

After `hir::validate`, the feature-gated lowerer accepts only this whole-value
ownership transfer shape. It emits stable Rust that performs `Option::take` at
the canonical parameter-to-temporary transfer, then moves the temporary to the
provisional result. The emitted trace records those two checked-plan actions in
order. It refuses contracts, effects, yields, borrowing, finalizers, projected
values, and every other cleanup plan.

The generated Rust driver invokes the emitted `spx_entry` through a real stable
Rust `Iterator::map` generic callback. This establishes that the generic
callback reaches Rust source generated from the checked `identity` HIR. It does
not establish lowering of a Semaprax generic function or function value.

The checked source's `main` reads back its own bytes and returns `42`. The
selector runs that same source through the interpreter and the native C11
emitter. The C11 result comparison is deliberately only the shared observable
value: neither route exposes a physical cleanup trace for this island.

## Local execution record

The focused selector ran from `wavect/v080` at `d17096f73` with the RI-14
source and test changes in the worktree. The companion
[`MEASURED-REPORT.md`](../../experiments/ri-14-stable-rust-lowering/MEASURED-REPORT.md)
records the exact toolchain, command, results, and unavailable measurements.
The tested RI-14 code is committed separately from that report so its revision
can be named exactly without another expensive build.

| Field | Required value |
| --- | --- |
| Source revision | Base `d17096f73`, dirty only for ongoing multi-agent work; exact RI-14 code commit recorded in the companion report |
| Rust compiler | `/opt/homebrew/bin/rustc`, release 1.98.0, commit `88d9e12ae178fab0fb5cc050a94da85685d449ea` |
| C compiler | `/usr/bin/clang`, Apple clang 21.0.0 |
| Target | `aarch64-apple-darwin`, matching `experiments/ri-14-stable-rust-lowering/toolchain.lock` |
| Command | Feature-gated six-test selector; 6 passed, 0 failed, 0 ignored, 5736 filtered |
| Generated Rust | `sha256:035e36e71151f3fbd77227056d98399dbc4fac52db6d46eb63110fd985226447`; both optimization levels ran |
| Differential | Interpreter and C11 returned `42`; generated owned identity preserved `[0,255,7,0]` and its two-action trace |
| Negative control | Lexical Drop reverse order and forged cleanup-plan refusal both passed |
| Limitations | No common C11/interpreter cleanup trace, no Semaprax-originated generic callback, no allocation or optimized-overhead measurements |

The held local run used:

```sh
SEMAPRAX_RI14_RUSTC=/opt/homebrew/bin/rustc CLANG=/usr/bin/clang \
CARGO_TARGET_DIR="$PWD/target/ri05-owner" CARGO_BUILD_JOBS=1 \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test --offline --locked -p semaprax --lib \
  --features unstable-rust-source-lowering stable_rust_lowering::tests \
  -- --nocapture --test-threads=1
```

The selector must prove all of these facts in the one run:

1. The pinned stable `rustc` accepts and runs the generated source.
2. The generic `Iterator::map` callback invokes the included generated
   `spx_entry` exactly once.
3. Generated Rust at both optimization levels preserves `[0, 255, 7, 0]` and
   reports the canonical parameter-to-temporary then temporary-to-result trace.
4. Rust lexical `Drop` reports the reverse control trace
   `lexical.second`, then `lexical.first`, which differs from the canonical
   transfer trace.
5. Interpreter and C11 `-O0`/`-O2` execution of the same checked Semaprax
   source all produce `42`.
6. A fixture-derived cleanup-plan mutation is refused by HIR validation before
   the lowerer can emit Rust source.

## Decision

The six tests passed for the admitted identity island. **No-go:** keep the
feature experimental and unselected by default. The current C11/interpreter
lanes expose no matching physical cleanup trace; the generated callback is a
Rust driver callback; the trial has no allocator counter or optimized bridge
benchmark. A broader backend would require new lowering, diagnostics, parity
observability, and maintenance for cases this trial does not admit. The one
observable benefit is a direct generic Rust callback into generated Rust for
this tiny island; it does not justify promoting another supported backend.

Any future promotion needs a separately scoped proposal and explicit approval.
