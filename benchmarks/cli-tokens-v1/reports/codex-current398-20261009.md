# LogLens: fresh current-compiler repeat (2026-10-09)

This completed Codex campaign retained five attempts per arm and all ten outcomes. SEMAPRAX accepted **3/5**; TypeScript accepted **4/5**. The three rejected attempts (SEM01, SEM04, TS02) each failed the same two `literal-plus-timezone` boundary outputs (text and JSON). Every trial completed, and the resource receipts classify all ten as uncontaminated. These are descriptive observations from one matched campaign; they do not establish a winner, a causal language comparison, or an efficiency advantage. Failed attempts and their full usage/cost estimates remain in every all-attempt total.

The adapter metadata retains `round: 6`; this is a new artifact namespace and plan, chronologically after the previously published round-seven report. It is not a replacement, selective retry, or corrected rescore of that earlier campaign. The stable label here is “fresh current-compiler repeat.” “Current-compiler” is a launch-time label for the pinned source `398b051e6e7a06ac49ecf77d9292831401430de0`; later source batch `700701` (`687cbed33`) and subsequent heads were not included in these observations.

## Frozen identity and acceptance

- Compiler source: `398b051e6e7a06ac49ecf77d9292831401430de0`; executable SHA-256 `594980a96bb3d7f74a168dfa6bc71d6355344f2963489f1e3ae854d2f3a01237`.
- Seed repository commit: `393432ccb1ec4a8da56284b5670887c4a8fd4b72`; frozen input commit `2d1b5c570221c66427704fb907c481780a84e008`.
- Model requested and recorded in client usage: `gpt-6.1-sol`, effort `medium`. Codex CLI `codex-cli 0.160.1`. Provider-resolved model identity is unavailable.
- Frozen SPEC SHA-256 `51bf564cb9f7eadd4b3bd12fac53b7857befa09b3ef757d9c34400a8509fdeae`; sample SHA-256 `b4574c3e627aff7760a934ec5dc10c45abf76ade1f6ae0c20518c56cc6660598`.
- Gate profile `loglens-historical33-plus-spec-boundaries.v1`: 33 historical checks plus 16 SPEC boundary checks (49 total). Corpus SHA-256 `9fd0993142aee8b47e5d3a4256379eeddcd903cadf69419717c40032d769c745`; boundary audit source SHA-256 `97eaa89fbac7dc0311154e5b0bda4e2ccb389518a3aa593639fdc36173e7acf8`; qualification script SHA-256 `d2214ae5922fee6949767ca661cba84fa07aab4ad66d4f446160927f838d6dd5`.
- Counterbalanced attempt order: `SEM01, TS01, TS02, SEM02, SEM03, TS03, TS04, SEM04, SEM05, TS05`. Per-attempt timeout: 1800 seconds. Every attempt used one outer Codex CLI turn; the model-request counts below are the internal requests recorded within those turns.
- The acceptance result is limited to this benchmark application suite, not the repository-wide quality gates.

## All-attempt measurements

Values are mean per attempt, with five attempts in each arm. Parentheses show the total across all five. “Raw input” includes cached input; cached input is a subset and must not be added to raw input. “Uncached input” is exactly raw input minus cached input; it still includes task, system, tool, and history context and is not task-only input. The legacy net-input convention is separate and is also not task-only input.

| Measure | SEMAPRAX | TypeScript |
|---|---:|---:|
| Model requests | 29.200 (146) | 8.800 (44) |
| Raw input tokens | 1,556,745 (7,783,725) | 233,993 (1,169,967) |
| Cached-input subset | 1,491,610 (7,458,048) | 192,973 (964,864) |
| Uncached input (raw minus cached subset) | 65,135.4 (325,677) | 41,020.6 (205,103) |
| Legacy net-input convention | 1,159,771 (5,798,855) | 114,419 (572,095) |
| Output tokens | 15,285 (76,424) | 8,527 (42,634) |
| Final-source token proxy | 8,014 (40,072) | 5,332 (26,661) |
| Agent wall seconds | 448.409 (2,242) | 200.263 (1,001) |
| Build + acceptance wall seconds | 32.220 (161) | 4.035 (20) |
| Accepted tasks | 3/5 | 4/5 |
| Conditional API-equivalent estimate, all attempts | $2.161400 | $0.933032 |
| Conditional estimate per accepted task | $0.720467 | $0.233258 |

The cost-per-accepted values divide the conditional short-context rate-card estimate for **all five attempts, including rejected attempts**, by the accepted count. They are not actual account charges: provider billing receipts are unavailable. The provider-resolved model and tier are unavailable. The displayed final-source measure is the legacy Claude BPE tokenizer proxy over each final inventory, including tests, docs, and scripts; it is neither cumulative authored work nor exact GPT tokenization. It is not labeled actual authored tokens.

## Attempt record

| Order | Arm/run | Result | Requests | Raw input | Cached subset | Uncached input | Output | Legacy net | Final-source proxy | Conditional estimate | Agent wall (s) | Acceptance wall (s) |
|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | SEM01 | not_accepted | 32 | 1,686,295 | 1,616,128 | 70,167 | 14,383 | 1,251,255 | 8,243 | $0.445777 | 432.276 | 34.925 |
| 2 | TS01 | accepted | 9 | 239,075 | 191,360 | 47,715 | 7,567 | 116,783 | 4,659 | $0.190236 | 179.802 | 4.171 |
| 3 | TS02 | not_accepted | 10 | 273,732 | 239,744 | 33,988 | 9,189 | 137,852 | 5,501 | $0.183840 | 195.620 | 4.536 |
| 4 | SEM02 | accepted | 21 | 1,037,805 | 976,640 | 61,165 | 14,774 | 752,310 | 7,959 | $0.367734 | 387.662 | 28.528 |
| 5 | SEM03 | accepted | 22 | 1,084,283 | 1,028,096 | 56,187 | 11,743 | 785,193 | 7,352 | $0.332614 | 328.086 | 27.477 |
| 6 | TS03 | accepted | 9 | 244,364 | 196,480 | 47,884 | 10,142 | 122,072 | 6,309 | $0.216836 | 253.219 | 3.996 |
| 7 | TS04 | accepted | 8 | 204,809 | 156,928 | 47,881 | 7,911 | 96,105 | 5,222 | $0.190565 | 194.881 | 3.542 |
| 8 | SEM04 | not_accepted | 30 | 1,611,085 | 1,545,728 | 65,357 | 16,736 | 1,203,235 | 7,897 | $0.452647 | 490.870 | 33.775 |
| 9 | SEM05 | accepted | 41 | 2,364,257 | 2,291,456 | 72,801 | 18,788 | 1,806,862 | 8,621 | $0.562628 | 603.152 | 36.394 |
| 10 | TS05 | accepted | 8 | 207,987 | 180,352 | 27,635 | 7,825 | 99,283 | 4,970 | $0.151555 | 177.792 | 3.931 |

Each row remains in the denominator. Per-attempt transcript and rollout SHA-256 values, candidate-archive verification, detailed resource receipts, and raw usage reconciliation are in the recount JSON linked below.

## Calibration and limits

Calibration was separate and was not subtracted: 5.086 seconds, one request, 13,203 raw input tokens, 0 cached-input tokens, therefore 13,203 uncached input tokens, and 5 output tokens; conditional API-equivalent estimate $0.026456. This uncached calibration input is a context diagnostic, not task-only input. Fixed harness/system/tool/task/history context composition remains unknown (`fixed_harness_context_tokens: null`); the calibration is not a measurement of per-trial fixed context.

Cache-write input was zero for every attempt. Reasoning-output counts are already subsets of output. Actual billed cost, provider-resolved model identity, and stable context tokens are null. The legacy Claude BPE final-source proxy uses `@anthropic-ai/tokenizer` 0.0.4 / `tiktoken` 1.0.22 and must not be treated as exact current-model tokenization.

## Evidence bindings

All evidence paths below remain in the local campaign archive; transcripts and candidate archives are intentionally not copied into Git.

| Evidence | Path | SHA-256 |
|---|---|---|
| Completed results | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/results.json` | `fb3cc013fc5990a87417c54d8fb62f1db029cd3489fcf5fff342e38d84c92ce4` |
| Campaign manifest | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/campaign.json` | `79d28a19fa1b7a9ee33edccfb74069fd8b10192997575bc74409b46f5cb0a548` |
| Reviewed plan | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/future-cli-shiftsim-source398b051e6-prep-20261009/loglens-current398.plan.json` | `21429250472e855378b1e9232b6a20385ac9d1b9ae4706af81ed66debbb84af1` |
| Launcher terminal receipt (session 11026, exit 0, 10 attempts) | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-terminal-receipt-20261009.json` | `ba4cacf4645045a46783aac05fff6a045b12a7aaf2ab16691e8989e7ca18fdb1` |
| Separate calibration record | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/calibration.json` | `bd3421739cd843be5a2933b16cb59a0995c378a3102632fc113edb9394b999ea` |
| Recount helper source | `benchmarks/cli-tokens-v1/codex_report.py` | `f939cea597416eb4097c97abd2aed2e39e34cb53b82bebe0967aab11168c9bc6` |
| Tracked trace-backed recount | `reports/codex-current398-20261009-recount.json` | `8cf37abde6d55e6e95561ee26d5fd16805ed972d8e3f404d87cde996633e4d60` |
| Resource receipt `calibration` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/calibration.json` | `304eef8232315f8ea0dbb7a2499aba4252fb7cc7eaa4c716387fd19062d10992` |
| Resource receipt `semaprax-01` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/semaprax-01.json` | `022c2aff68a264d9c0174b17e5e8a9866d247121d9a0c3a77a5be2bca66073ea` |
| Resource receipt `semaprax-02` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/semaprax-02.json` | `a3e55bb3ee9f786039272e69d205db64bfd6184a78f00e57b2db463e60ae10cc` |
| Resource receipt `semaprax-03` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/semaprax-03.json` | `172fd3471fb9134e9282488109f8ed07b5056e1ced2261227680a8e68d744ca2` |
| Resource receipt `semaprax-04` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/semaprax-04.json` | `eb4d6390ecec177d33d4d23d797812f4c8ab42a4f4f63a7f3d743da5fe1ce72a` |
| Resource receipt `semaprax-05` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/semaprax-05.json` | `49f676089cbbf08cc923e3edecc51636b52ea5aab0bb10be5ab8e16a3f5403df` |
| Resource receipt `typescript-01` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/typescript-01.json` | `e1980b7813e7e72752b5a0d64dc3df9be466705d974ebb85e8a8fdef84226338` |
| Resource receipt `typescript-02` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/typescript-02.json` | `368890e220562f0b2f0e7a6317ecac5f1851e607a09da24fe0a7fbbefc75c7bd` |
| Resource receipt `typescript-03` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/typescript-03.json` | `d246db3ba0a278a89823ae164d13b37ed463427a34496b76eb32fa8b2e10ce2c` |
| Resource receipt `typescript-04` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/typescript-04.json` | `0240da7e9213ac3dbdbdb21191c740b1a26ac4afb50b0a862c223d9c8dad724a` |
| Resource receipt `typescript-05` | `/Users/kevin/.codex/benchmark-runs/opt687-689-verification-20261008/loglens-current-source398b051e6-base393432ccb-20261009/resource-receipts/typescript-05.json` | `415835cf976c0e81b95bf3c0563fad84f4b3e0bed34bc53d533dc18fcb3aebdc` |

The tracked recount JSON validates the saved results against transcript/rollout usage, hashes every archived candidate file listed by the campaign, confirms each final-source inventory hash and token sum, and retains every attempt. It is accounting evidence, not a new application run or acceptance execution.
