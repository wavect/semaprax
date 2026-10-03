# RI-14 measured go/no-go report

Status: **executed local no-go**, 3 October 2026. One focused run was sufficient
to reject backend promotion: the missing common cleanup trace and benefit
measurement capabilities are structural gaps. Additional identical timing runs
would not change that decision.

This report evaluates the narrow checked-HIR `own Bytes` identity island. It
does not claim generic Semaprax callback lowering, a full Rust-source backend,
or a physical cleanup trace from C11 or the interpreter.

## Run identity

| Field | Recorded value |
| --- | --- |
| Git base at execution | `d17096f73` on `wavect/v080`; the RI-14 changes were uncommitted at execution and are included in the code commit cited in the issue closing record |
| Worktree dirty state | Dirty with RI-14 changes and disjoint multi-agent work; only RI-14 paths are used by this report |
| Host OS and CPU | macOS Darwin, Apple arm64 |
| Target | `aarch64-apple-darwin` |
| `SEMAPRAX_RI14_RUSTC` absolute path | `/opt/homebrew/bin/rustc` |
| `rustc --version --verbose` | 1.98.0, commit `88d9e12ae178fab0fb5cc050a94da85685d449ea`, host `aarch64-apple-darwin`, LLVM 22.1.8 |
| `CLANG` absolute path | `/usr/bin/clang` |
| `clang --version` | Apple clang 21.0.0, arm64-apple-darwin25.5.0 |
| Artifact function / cleanup schema / SHA-256 | `ri14.transfer.identity`, `semaprax.cleanup-plan.v2`, `sha256:035e36e71151f3fbd77227056d98399dbc4fac52db6d46eb63110fd985226447` |

## Reproduction

The one necessary focused run used the warm private target in this checkout.
The code selector includes generated Rust and C11 optimization variants,
interpreter execution, and the two negative controls.

```sh
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="$PWD/target/ri05-owner"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SEMAPRAX_RI14_RUSTC=/opt/homebrew/bin/rustc
export CLANG=/usr/bin/clang
cargo test --offline --locked -p semaprax --lib \
  --features unstable-rust-source-lowering stable_rust_lowering::tests \
  -- --nocapture --test-threads=1
```

The selector compiles and executes generated Rust at `-C opt-level=0` and
`-C opt-level=3`, executes C11 at `-O0` and `-O2`, and compares the same
checked fixture's interpreter value. It also proves the generated canonical
transfer trace, its lexical-`Drop` reverse-order negative control, and
pre-emission refusal of a fixture-derived forged cleanup plan.

## Results

| Measure | Observed result |
| --- | --- |
| Focused-selector exit status | 0; 6 passed, 0 failed, 0 ignored, 5736 filtered |
| Compilation and selector time | Cargo reported 3m 08s compilation and 3.07s for the six tests; this warm target is not a cold-build benchmark |
| Generated Rust `-C opt-level=0` and `3` | Both executed the checked-HIR artifact and returned `[0,255,7,0]` with `Transfer(parameter -> temporary)`, then `Transfer(temporary -> provisional-result)` |
| Interpreter value | `42` from the checked source's `main` |
| C11 `-O0` and `-O2` | Both executed the same source's `main` and printed `42` |
| Generated Rust/C11 compiler diagnostics | No generated-source compiler errors; C11 compiled with `-Werror` |
| Generated wrapper count | One hand-written generic `Iterator::map` driver invokes one generated `spx_entry`; zero C ABI thunks inside this isolated Rust call |
| Type-check fidelity | The generated Rust driver compiled with pinned stable rustc; this does not test arbitrary Semaprax generic obligations |
| Allocations | Unavailable: no allocator counter in either lane |
| Optimized call overhead | Unavailable: one callback invocation is a correctness check, not a benchmark |
| RI-01 bridge comparison | Qualitative only: this island avoids a C thunk inside its Rust driver, but no matched overhead or allocation measurement exists |

## Negative controls

| Control | Expected observation | Recorded observation |
| --- | --- | --- |
| Rust lexical `Drop` | `lexical.second`, then `lexical.first`; differs from canonical transfer trace | Passed in generated executable |
| Forged same-fixture cleanup plan | `hir::validate` fails and lowering returns `InvalidHir` | Passed in focused unit test |
| Missing `SEMAPRAX_RI14_RUSTC` | Selector reports a skip; it is never counted as a pass | Not run; environment variable was set for this trial |
| Incorrect toolchain commit or host target | Selector refuses before generated-source compilation | Passed by `rejects_empty_target_and_noncanonical_compiler_commit_before_hir_use`; pinned live identity also matched |

## Decision record

| Criterion | Result |
| --- | --- |
| Semantic parity for this identity island | Partial: owned value and generated transfer trace pass; C11/interpreter expose only the source's scalar `main` result, not matching physical cleanup traces |
| Generic Rust callback reaches checked-HIR-generated source | Yes, through the generated driver and pinned stable `Iterator::map`; callback is not Semaprax-originated |
| C11/interpreter common physical cleanup trace | unavailable: routes expose no trace |
| Generic Semaprax callback lowering | unavailable: no admitted lowering exists |
| Allocation and optimized-call-overhead benefit | unavailable: no measurement harness |
| Decision | **No-go for backend promotion.** Keep this nondefault executable experiment; no benefit claim beyond the local island. |

The selector passed, but the parity and benefit gaps prevent a justified
promotion. A future backend proposal needs a shared failure/cleanup corpus,
Semaprax-originated callback lowering, bounded diagnostic provenance and a
matched RI-01 bridge benchmark before the maintenance cost can be assessed.
