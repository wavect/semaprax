# LAW16 guarded-i64 balance source proofs

This fixture flattens the debit and credit projections into scalar `i64`
functions with explicit `[0, 2^32 - 1]` input guards. The source route proves
the output range, exact guarded update, conservation, and intended positive
state change as seven selected postconditions. The no-op debit mutation is
refused by the installed proof route. Its diagnostic is intentionally recorded
as a tool refusal; it does not identify whether Z3 returned a counterexample,
unknown, or another unsupported result.

The original structured full-u32 balance fixture remains unsupported by this
source-proof profile. This evidence does not prove lowering or application
execution and does not close LAW16.

The retained run is under
`benchmarks/bend2-law-v1/evidence/law16-guarded-i64-balance-smt-v1/`. Its
`result.json` pins source files, compiler commit and executable digest, Z3
version and executable digest, each selected obligation, and SHA-256/byte
counts for every raw stdout and stderr file. `noop_mutant.app.spx` retains the
exact negative source. The companion validator checks these bindings without
rerunning a solver:

```sh
python3 benchmarks/bend2-law-v1/law16_guarded_i64_balance_smt.py \
  --review benchmarks/bend2-law-v1/evidence/law16-guarded-i64-balance-smt-v1
python3 benchmarks/bend2-law-v1/test_law16_guarded_i64_balance_smt.py
```

To rerun the pinned local route, build the pinned source commit with the
low-footprint profile and use the exact installed tools recorded in
`result.json`. `/private/tmp` is required for canonical project paths on the
recorded macOS host. Choose a new output directory for each run:

```sh
CARGO_TARGET_DIR=target/law16-lowfootprint CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  cargo build --locked -p semaprax --bin semaprax
python3 benchmarks/bend2-law-v1/law16_guarded_i64_balance_smt.py \
  --semaprax target/law16-lowfootprint/debug/semaprax \
  --z3 /opt/homebrew/bin/z3 \
  --output /private/tmp/law16-guarded-i64-rerun
```

The checked-in evidence was produced on macOS arm64 with SEMAPRAX source
commit `5e3720672e441b0202b69b51862058493a1939e9` and Z3
`4.12.5 - 64 bit`. It is local trusted-tool evidence, not a hosted or
cross-platform result.
