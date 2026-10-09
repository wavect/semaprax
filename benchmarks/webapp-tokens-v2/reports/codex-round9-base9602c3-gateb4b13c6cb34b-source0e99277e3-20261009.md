# TeamDesk round 9: original frozen-gate campaign

> **Interpretation:** The original paid campaign below remains bound to its frozen gate (`b4b13c6cb34b`) and is preserved unchanged. A separate offline replay of all ten retained candidates under corrected gate source `810afdd` follows later in this report and in the linked sidecar. The original gate had known #698 browser locator/audit false negatives; TypeScript attempts also encountered offline-bootstrap/tooling friction. Neither the original nor corrected-gate outcome supports an intrinsic language/compiler advantage. Rejected attempts and their original costs remain recorded; no original attempt is reclassified.

Original recount SHA-256: `c287854aea2a175fc50ad6a99d5771486541e61eb026c28cdb106a45ccc2aa3d`. Original results SHA-256: `6eabba2789ffbc8c28163abee722b17d9d0e756acd625b64d92d2f249cd42c3b`. Original terminal receipt SHA-256: `8b19c01c4d86397d0b98b75b3b2935df18f6fb0e868538969b8ea0a828dbe454`.

# TeamDesk round 9 final recount

Terminal complete: 10/10 attempts; accepted 5/10; clean measured floor across the ten attempts. Calibration resource floor was not measured.

Every failed/rejected attempt remains in the ledger. Prices below are conditional API-equivalent estimates from reconciled request traces, not provider billing. The final-file count is an unverified inventory proxy; no authored-source ratio is supported.

| Attempt | Requests | Raw input | Cache-read subset | Cache-write subset | Legacy net proxy | Output | Reasoning subset |
|---|---:|---:|---:|---:|---:|---:|---:|
| semaprax-01 | 23 | 1048272 | 978688 | 0 | 732965 | 13045 | 2846 |
| typescript-01 | 33 | 1301538 | 1222016 | 0 | 850428 | 32429 | 8138 |
| typescript-02 | 35 | 1300335 | 1216640 | 0 | 821885 | 32079 | 7768 |
| semaprax-02 | 22 | 1062497 | 986752 | 0 | 760899 | 13364 | 1668 |
| semaprax-03 | 18 | 729153 | 667904 | 0 | 482391 | 12079 | 1661 |
| typescript-03 | 22 | 843112 | 779520 | 0 | 542372 | 35303 | 8013 |
| typescript-04 | 24 | 839468 | 789760 | 0 | 511388 | 26506 | 6847 |
| semaprax-04 | 17 | 654879 | 590080 | 0 | 421826 | 10108 | 1314 |
| semaprax-05 | 23 | 1151664 | 1068800 | 0 | 836357 | 14389 | 1931 |
| typescript-05 | 27 | 1042110 | 961280 | 0 | 673020 | 32754 | 10013 |

| Attempt | Saved status | Acceptance checks | Conditional estimate USD | Actual billing USD | Agent wall s | Acceptance wall s | Final-file proxy |
|---|---|---|---:|---:|---:|---:|---:|
| semaprax-01 | accepted | 912 passed | 0.367487 | null | 274.519 | 316.357 | 10014 |
| typescript-01 | not_accepted | 1 failed | 0.605536 | null | 696.052 | 35.239 | 253523 |
| typescript-02 | not_accepted | 1 failed | 0.609844 | null | 804.855 | 2.456 | 26824 |
| semaprax-02 | accepted | 912 passed | 0.383805 | null | 279.421 | 314.297 | 13335 |
| semaprax-03 | accepted | 912 passed | 0.310078 | null | 234.776 | 314.564 | 8952 |
| typescript-03 | not_accepted | 860 passed, 52 failed | 0.558166 | null | 748.477 | 276.021 | 26698 |
| typescript-04 | not_accepted | 858 passed, 7 failed, 47 unverified | 0.443452 | null | 714.17 | 607.397 | 249679 |
| semaprax-04 | accepted | 912 passed | 0.289686 | null | 195.65 | 312.778 | 52178 |
| semaprax-05 | accepted | 912 passed | 0.416498 | null | 283.913 | 316.615 | 10759 |
| typescript-05 | not_accepted | 887 passed, 25 failed | 0.585328 | null | 741.151 | 279.048 | 21715 |

## Failure-inclusive per-arm totals

| Arm | Accepted / 5 | Raw input | Cache-read subset | Cache-write subset | Legacy net proxy | Output | All-five estimate USD | Estimate per accepted USD | Agent wall s | Acceptance wall s | Accepted final-file proxy |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| semaprax | 5/5 | 4646465 | 4292224 | 0 | 3234438 | 62985 | 1.767554 | 0.353511 | 1268.279 | 1574.611 | 95238 |
| typescript | 0/5 | 5326563 | 4969216 | 0 | 3399093 | 159071 | 2.802326 | null | 3704.705 | 1200.161 | null |

All ten conditional estimates: 4.56988 USD; estimate per accepted task: 0.913976 USD. Actual billing across all ten: null USD; provider billing known for 0/10 attempts.
All ten requests: 244; provider output: 222056; reasoning-output subset: 50199.

## Separate calibration

Status ready; requests 1; raw input 13315; cache-read subset 8832; cache-write subset 0; output 5; reasoning subset 0; legacy net proxy 0; conditional estimate 0.009899 USD; actual billing null USD. It is excluded from trial totals and never subtracted.

## Separate corrected-gate replay of the ten retained candidates

The corrected gate replay is a distinct offline qualification pass; it made no paid model requests and did not change the original cost, agent-time, acceptance-time, or resource records above. It used gate source `810afdd907561dcfe6aab52823d43b1ffecd0a0e` with the same retained compiler source `0e99277e3397bc9a878c015bf2bcb0c843903ce3` and binary SHA-256 `3839f57b3d4bb502f2c852e696f04347dfd45d5e0c9de82db0d20418c4200a0d`.

| Retained attempt | Corrected-gate result | Interpretation |
|---|---:|---|
| semaprax-01 | 912/912 | accepted |
| typescript-01 | runner fatal | own build/test terminated before the 912-case qualification |
| typescript-02 | runner fatal | own build/test terminated before the 912-case qualification |
| semaprax-02 | 912/912 | accepted |
| semaprax-03 | 912/912 | accepted |
| typescript-03 | 860/912 | 52 failures, mainly entity CSV/list checks |
| typescript-04 | 891/912 | 21 failures: 20 list/CSV checks and one audit-action defect |
| semaprax-04 | 912/912 | accepted |
| semaprax-05 | 912/912 | accepted |
| typescript-05 | 887/912 | 25 failures, mainly entity CSV/list checks |

All five SEMAPRAX replay reports passed 912/912. No TypeScript attempt was accepted: two ended in runner-fatal build/test failures and the other three had candidate checks fail. For TS04, the audit failure was a successful no-op update whose event had no action and an empty changed-field map; the contract requires a missing action to be inferable unambiguously from complete old/new field changes. The other 20 TS04 failures were list/CSV checks. This classification preserves the contract and does not excuse the candidate failure.

The replay rescore took 1,233.922 seconds across two workers, but this is not an isolated host-time measurement: an independent `release-ci-fix` Cargo/test job overlapped the replay, and the scoped verification lane also ran. A later cleanup removed the completed verification target (`3,926,020,096` allocated bytes) and completed test executables to restore disk headroom; it retained archived compiler binaries, dependencies, and paid evidence.

TypeScript dependency bundles were recovered for this replay, but historical byte identity is **false**. This replay therefore records how the retained candidates behaved with the disclosed recovered bundles; it does not establish the exact historical dependency bytes. The original paid measurements remain the authoritative source for requests, token subsets, costs, and agent/acceptance wall times. Billing remains null, model resolution and fixed context remain unavailable, and the authoring-token measure remains a legacy tokenizer proxy rather than verified authored tokens. No language-advantage or upper-bound claim is supported. A fresh matched TypeScript-bootstrap campaign remains necessary before closing OPT #606.

Replay terminal receipt: `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/round9-corrected-810afdd90-replay-terminal-20261009.json` (SHA-256 `088eeeffe9529f461f72a1853cc5c9b0b803eda7bfea7ef8be62096051232c0c`); replay session 46087 exited 0, sidecar validation session 7894 exited 0, and all ten trials are bound. The raw rescore SHA-256 is `eaf0179073fd53d4b8f861d2adaef33712282f4da79bade600896b7d916dd7c3`. The checked publication sidecar is [`codex-round9…-corrected-replay.json`](codex-round9-base9602c3-gateb4b13c6cb34b-source0e99277e3-20261009-corrected-replay.json) (SHA-256 `d02a053dd4b16c9cc72f3e8adf1905858213573efdf6b1b090b511ffcc7eb212`); the validation receipt binds the raw rescore digest `eaf017…` rather than this derived publication file.

## Independent reference qualification (compiler source 878, gate 810)

A separate finalization binds the SEMAPRAX and TypeScript reference reports to 912/912 passing cases under gate source `810afdd907561dcfe6aab52823d43b1ffecd0a0e`, using compiler source `8785368bfcf984ece85db7f2ce2f55586dfc1775` and binary SHA-256 `78a7d91a5085fb45ef2cca28ad28f53ef21065e0eb24dc8523e9b3dce07d8355`. The exact qualification receipt is `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/teamdesk-reference-corrected-810afdd90-source878-finalized-20261009/qualification-receipt.json` (SHA-256 `2f53d95006d96ba736dfa17ab0103c2c49f468c8ba82a2ef2e34a947ae606003`). SEMAPRAX report SHA-256: `b1a17becb25936a31bd46a5cdb2e17d11101430f538e94017588f79ef9c7e74a`; TypeScript report SHA-256: `bfdd0cc112f1093183237686cd4cad6349dd7fdd4acb79c8156394b65de41f9b`. The tracked helper pin `acceptance/evidence/reference-r8-summary.json` now contains this receipt; its new SHA-256 is `2f53d95006d96ba736dfa17ab0103c2c49f468c8ba82a2ef2e34a947ae606003`. The original round 9 paid results remain bound to their original `b4b13c6` gate and `0e99277` compiler and are not changed by this pin update.

The original qualification collector exited 1 after both actual application reports passed; its source-inventory comparison treated SEM's declared `generated/` compiler-output directory as source. The retained `build.sh` declares `--output generated`; the offline finalizer separately binds original source inputs, generated-output inventories, and full reports and exited 0 without rerunning either application. Lineage: `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/teamdesk-reference-corrected-810afdd90-source878-finalized-20261009/finalization-lineage.json` (SHA-256 `4dc7d657e00a2f9f89dbda1811aa1a00c438d872a367985807f72aa6e0f947d4`). This qualification does not qualify a future `699` source commit and does not itself report any corrected-gate replay result.

## Scope and provenance

Base `9602c37abc924471b0a41b4da1244dc3099b5cba`; compiler source `0e99277e3397bc9a878c015bf2bcb0c843903ce3`; binary SHA-256 `3839f57b3d4bb502f2c852e696f04347dfd45d5e0c9de82db0d20418c4200a0d`; gate `b4b13c6cb34bcd2165a9d5aec06800e85324ee24`.
Terminal receipt `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/teamdesk-round9-base9602c3-gateb4b13c6cb34b-source0e99277e3-terminal-receipt.json`; SHA-256 `8b19c01c4d86397d0b98b75b3b2935df18f6fb0e868538969b8ea0a828dbe454`. Frozen results SHA-256 `6eabba2789ffbc8c28163abee722b17d9d0e756acd625b64d92d2f249cd42c3b`.

Ten terminal attempts with failure-inclusive accounting and measured clean disk-floor receipts are required. The frozen calibration harness does not measure a separate calibration-session disk floor. This evidence covers TeamDesk only; LogLens and ShiftSim current-compiler repeats remain outstanding until their own actual terminal evidence.

Raw provider-reported input tokens are shown with cached and cache-write subsets separately; cache subsets are not added twice.

Historical campaign helper convention only; not task-only input and not a context-calibrated subtraction.

Provider-reported output tokens, including any provider-counted reasoning output; not final source size.

Final candidate inventory proxy only, not verified authored source. Generated-output/lockfile/provenance components were not classified by this recount. Tokenizer-bound counts are not cumulative authored work or exact current-model tokenization; no authored-source advantage ratio is supported.

Conditional published-price API-equivalent estimate for all ten attempts, including failures, divided by accepted tasks only when accepted count is nonzero and every trial estimate is known. Provider-exposed actual billing is reported separately; missing billing remains null and is never replaced with the estimate.

Fixed harness context is null because request composition is unavailable. Calibration usage remains a separate session and is not subtracted.

The per-accepted-task estimate is null when accepted count is zero. Raw input already includes cache subsets; output already includes any provider-counted reasoning subset. Neither subset is added twice.
