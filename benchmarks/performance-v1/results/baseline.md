# Performance macrobenchmark results

- Schema: `benchmark.performance.v2`
- Recorded: 2026-09-06T07:11:22.088481Z
- Host: `darwin-arm64` (Darwin 25.5.0, 11 logical CPUs, load average at start [2.07, 2.64, 2.98])
- Toolchain: rustc 1.98.0 (88d9e12ae 2026-08-18) (Homebrew), cargo 1.98.0 (797e8a9bc 2026-08-05) (Homebrew)
- Subject: `semaprax 0.3.5 (commit unknown)` profile `debug`, binary digest `sha256:fd2f63f689b56f91c61392aced1fe893b214726ccafdc15e6be6c766fe319473`
- Revision: `06bd1d9f49ee2b2f3e64db827c749741b277bcf4` (dirty working tree: False)

Wall times are advisory local evidence for this one host and build. They are not hosted, release, or cross-platform claims.

| Scenario | Command | Expect | Status | p50 ms | p95 ms | Samples | Subject digest |
| --- | --- | --- | --- | --- | --- | --- | --- |
| check-analytics-pipeline | check | success | ok | 83.92 | 126.87 | 5 | `sha256:7b13150152e501b67ccdd11a45229be836af6eb5e01f3278d668d035962db961` |
| check-apex-supply-chain | check | success | ok | 188.64 | 190.88 | 5 | `sha256:88e9e485a26c292ea6afa88addae3201a9feafc7b0c721f7ac32596f6ecb4739` |
| check-banking-ledger | check | success | ok | 23.62 | 45.51 | 5 | `sha256:c66390b3999792d40edafc0222a53e001827aba82ba6f0e866b5616c11cab9f0` |
| check-calculator | check | success | ok | 11.81 | 12.02 | 5 | `sha256:77b745c7c443774f7420b1ed06f7ddd43fa21345239ac4d6236a1929d58efedf` |
| check-calculator-project | check | success | ok | 46.41 | 47.97 | 5 | `sha256:69c43c39dc6bcdb6ed5946ae8ca5a704b3ff6bda7299ba8916161ce295f76234` |
| check-expression-evaluator | check | success | ok | 24.01 | 24.08 | 5 | `sha256:46489ff12f277efe4b35e7c338f0abaf59c03a1aeb6ca88425309667d056905b` |
| check-math-algorithms | check | success | ok | 21.32 | 23.82 | 5 | `sha256:0db2ecc2d33e4168078605173730ca17ef1fc2e5480dd7bab926aae78d4b4a1d` |
| check-meaning | check | success | ok | 11.79 | 11.86 | 5 | `sha256:6ae56f8e8b0a578d5a5d87790bb09ed988290948d90f68d76c001b314439d36f` |
| check-order-lifecycle | check | success | ok | 200.0 | 200.72 | 5 | `sha256:9f0b57c926d70959aea93de5a0aa406dea840cbf940711d0be28270f2dbf6c79` |
| check-text-analytics | check | success | ok | 137.67 | 143.66 | 5 | `sha256:ce84f9e5156669dfa1bec4b0411a37c6196bc7144085ed3925b22a2d8d4ff831` |
| context-meaning | context | success | ok | 10.77 | 11.88 | 5 | `sha256:6ae56f8e8b0a578d5a5d87790bb09ed988290948d90f68d76c001b314439d36f` |
| context-ownership | context | success | ok | 11.84 | 11.93 | 5 | `sha256:c0654273b51bea199873ffd260c81cdd5767c08a1b69a4f684c989142aed1f42` |
| graph-meaning | graph | success | ok | 11.86 | 12.29 | 5 | `sha256:6ae56f8e8b0a578d5a5d87790bb09ed988290948d90f68d76c001b314439d36f` |
| graph-records | graph | success | ok | 11.81 | 11.85 | 5 | `sha256:b8a69ca83813b97b5751838934f5eb5d53bab161fcdd14dd3b67a8a673c69da8` |
| run-apex-supply-chain | run | success | ok | 198.31 | 203.19 | 5 | `sha256:88e9e485a26c292ea6afa88addae3201a9feafc7b0c721f7ac32596f6ecb4739` |
| run-banking-ledger | run | failure | ok | 45.46 | 47.72 | 5 | `sha256:c66390b3999792d40edafc0222a53e001827aba82ba6f0e866b5616c11cab9f0` |
| run-calculator | run | success | ok | 11.84 | 11.91 | 5 | `sha256:77b745c7c443774f7420b1ed06f7ddd43fa21345239ac4d6236a1929d58efedf` |
| run-calculator-project | run | success | ok | 46.5 | 48.17 | 5 | `sha256:69c43c39dc6bcdb6ed5946ae8ca5a704b3ff6bda7299ba8916161ce295f76234` |
| run-math-algorithms | run | success | ok | 22.57 | 23.94 | 5 | `sha256:0db2ecc2d33e4168078605173730ca17ef1fc2e5480dd7bab926aae78d4b4a1d` |
| run-meaning | run | success | ok | 10.64 | 11.33 | 5 | `sha256:6ae56f8e8b0a578d5a5d87790bb09ed988290948d90f68d76c001b314439d36f` |
| test-apex-supply-chain | test | success | ok | 196.42 | 203.98 | 5 | `sha256:88e9e485a26c292ea6afa88addae3201a9feafc7b0c721f7ac32596f6ecb4739` |
| test-calculator-project | test | success | ok | 43.32 | 47.53 | 5 | `sha256:69c43c39dc6bcdb6ed5946ae8ca5a704b3ff6bda7299ba8916161ce295f76234` |

Summary: 22 ok, 0 failed, 0 skipped, 0 drifted.
