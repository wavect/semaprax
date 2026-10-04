# Harness application-task benchmark report (HN-17): 2026-10-04 HN-17 campaign

Contract `semaprax.harness-apptask-summary.v1`. Task-set digest `sha256:a5bb023559bcd0a17d5a9942291626842c1d9336abee78afa8765ebff98c0e23`. Trials recorded: 1244. Repetitions per (task, arm, model) cell: 1 to 10 over 182 cells: **PILOT** (non-pilot needs at least 10).

- Model `haiku`: 10 to 10 repetitions per cell: **MATCHED-TRIALS**.
- Model `qwen`: 1 to 2 repetitions per cell: **PILOT**.

## Labels and identities

- Tokens: `tiktoken:o200k_base (tiktoken 0.12.0)` counts of the exact text sent (all attempts, skill prompt, retrieval pack and command view included) and of the exact answers. These are not billing units for any provider.
- Provider usage: the provider's own reported input/output tokens (Claude Code CLI JSON `usage`, Ollama `prompt_eval_count`/`eval_count`). Claude figures include the CLI's own system overhead; they are kept separate from the named-tokenizer counts and never merged.
- Cost: USD from the CLI's `total_cost_usd` for the larger model; the local model has no billing (`unavailable`, never zero). Cost is not estimated from tokens.
- Success: decided only by the immutable grader (compiler checks and test oracles the model cannot edit; protected paths are restored before grading). Structural validity (parsable file blocks, no protected edit) is reported separately and is never success.
- Delivery versus influence: `skill delivered` means the host framed and sent the byte-exact upstream skill; influence is only what the matched comparison shows in output tokens and accepted rate.
- Local macOS aarch64 evidence only. No Linux, Windows or hosted run.

| identity | value |
| --- | --- |
| claude_cli | 2.1.289 |
| compiler | semaprax 0.7.0 (commit unknown) |
| harness_graft | `/private/tmp/claude-501/hp-tools/graft-0.21.1/node_modules/.bin/graft` 0.21.1 |
| harness_graphify | `/private/tmp/claude-501/hp-tools/graphify-0.9.75-venv/bin/graphify` graphify 0.9.75 |
| harness_node | `/Users/kevin/.nvm/versions/node/v24.3.0/bin/node` v24.3.0 |
| harness_python | `/Users/kevin/.local/bin/python3` Python 3.12.12 |
| harness_rtk | `/private/tmp/claude-501/hp-tools/rtk-0.51.0/rtk` rtk 0.51.0 |
| harness_tiktoken_cache | `/private/tmp/claude-501/hp-tools/tiktoken-cache` (no version output) |
| harness_tiktoken_python | `/private/tmp/claude-501/hp-tools/tiktoken-venv/bin/python` Python 3.12.12 |
| large_model | claude-haiku-4-5 via Claude Code CLI 2.1.289 (MAX_THINKING_TOKENS=0, system prompt replaced, no tools) |
| skill_blocks | {"caveman":{"bytes":5759,"delivered":true,"ids":["caveman"]},"concise":{"bytes":120,"delivered":true,"ids":["concise-baseline"]},"graft":{"bytes":0,"delivered":false,"ids":[]},"graphify":{"bytes":0,"delivered":false,"ids":[]},"native":{"bytes":0,"delivered":false,"ids":[]},"neg-output-keeps-first-file":{"bytes":0,"delivered":false,"ids":[]},"neg-skill-strips-work":{"bytes":249,"delivered":true,"id |
| small_model | qwen2.5:0.5b via local Ollama (model id a8b0c5157701); pilot: 2 repetitions, core arms plus one control |
| tokenizer | tiktoken:o200k_base (tiktoken 0.12.0) |

## Spend and ledger

Cap USD 15.00 (user-authorized), spent USD 10.5246, ledger calls 1732, calls refused 0. Models: haiku (claude-haiku-4-5, large, 10 repetitions, billed); qwen (qwen2.5:0.5b, small, pilot: 2 repetitions on core arms plus one control)

## Arms

| arm | role | description |
| --- | --- | --- |
| `native` | Core | native-only baseline |
| `ponytail` | Core | official Ponytail v4.10.3 primary skill, mode full |
| `caveman` | Core | official Caveman v3.1.0 primary skill (Ponytail's automatic coding selection switched off) |
| `ponytail+caveman` | Core | both official skills together |
| `concise` | Core | simple concise/reuse instruction baseline |
| `graft` | Ablation | Graft 0.21.1 retrieval pack instead of the full tree (ablation) |
| `graphify` | Ablation | Graphify 0.9.75 retrieval pack instead of the full tree (ablation) |
| `rtk-err` | Ablation | RTK 0.51.0 `err` view of the failing-test run (ablation) |
| `rtk-test` | Ablation | RTK 0.51.0 `test` view of the failing-test run (ablation) |
| `neg-skill-strips-work` | NegativeControl | negative control: a skill that strips required work (one file per reply) |
| `neg-output-keeps-first-file` | NegativeControl | negative control: lossy output compression applied by the harness (only the first file block of every answer survives), independent of model obedience |
| `neg-stripped-failure` | NegativeControl | negative control: failure lines stripped from the test output |
| `router` | untested | untested: the learned router (HN-16) is unavailable on this machine and its recorded gate is no-go (benchmarks/harness/2026-10-04-routing) |
| `wikiskill` | untested | untested: WikiSkill evolution (HN-14 lane) is not part of this measurement; no result is claimed |

## Results per model, task class and arm

### Model `haiku`

#### compile_repair

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 20/0 | 19/20 (0.76-0.99) | 10/20 | 20/20 | 5542 +/- 2531 | 648 | 6161/791 | 0.01010 | 0.01060 | 9861 | 1.80 | 2/2 | 17/18 | compile-repair-js:10/10 compile-repair-spx:9/10 |
| `concise` | 20/0 | 19/20 (0.76-0.99) | 10/20 | 20/20 | 2799 +/- 1123 | 673 | 3046/809 | 0.00710 | 0.00750 | 9763 | 1.65 | 2/2 | 17/18 | compile-repair-js:10/10 compile-repair-spx:9/10 |
| `graft` | 20/0 | 3/20 (0.05-0.36) | 0/20 | 20/20 | 5636 +/- 1538 | 1432 | 7521/3414 | 0.02620 | 0.17490 | 29418 | 2.90 | 0/2 | 3/18 | compile-repair-js:3/10 compile-repair-spx:0/10 |
| `graphify` | 10/10 | 2/10 (0.06-0.51) | 0/10 | 10/10 | 7281 +/- 295 | 1243 | 7788/1528 | 0.01540 | 0.07710 | 19277 | 3.00 | 0/1 | 2/9 | compile-repair-js:2/10 |
| `native` | 20/0 | 20/20 (0.84-1.00) | 11/20 | 20/20 | 2520 +/- 758 | 651 | 2668/778 | 0.00660 | 0.00660 | 8845 | 1.45 | 2/2 | 18/18 | compile-repair-js:10/10 compile-repair-spx:10/10 |
| `neg-output-keeps-first-file` | 20/0 | 16/20 (0.58-0.92) | 0/20 | 20/20 | 4977 +/- 706 | 865 | 5678/1041 | 0.01090 | 0.01360 | 13877 | 2.55 | 1/2 | 15/18 | compile-repair-js:10/10 compile-repair-spx:6/10 |
| `neg-skill-strips-work` | 20/0 | 14/20 (0.48-0.85) | 0/20 | 20/20 | 4355 +/- 543 | 582 | 5191/699 | 0.00870 | 0.01240 | 11380 | 2.40 | 2/2 | 12/18 | compile-repair-js:10/10 compile-repair-spx:4/10 |
| `ponytail` | 20/0 | 20/20 (0.84-1.00) | 10/20 | 20/20 | 6360 +/- 3095 | 738 | 6970/883 | 0.01540 | 0.01540 | 10470 | 1.80 | 2/2 | 18/18 | compile-repair-js:10/10 compile-repair-spx:10/10 |
| `ponytail+caveman` | 20/0 | 18/20 (0.70-0.97) | 9/20 | 20/20 | 8819 +/- 3853 | 604 | 9803/732 | 0.01900 | 0.02110 | 9702 | 1.75 | 2/2 | 16/18 | compile-repair-js:10/10 compile-repair-spx:8/10 |

#### failing_tests

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 20/0 | 20/20 (0.84-1.00) | 19/20 | 20/20 | 5045 +/- 1000 | 201 | 6190/247 | 0.01340 | 0.01340 | 5097 | 1.05 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `concise` | 20/0 | 20/20 (0.84-1.00) | 18/20 | 20/20 | 3950 +/- 1171 | 325 | 4921/386 | 0.01000 | 0.01000 | 6187 | 1.10 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `graft` | 20/0 | 20/20 (0.84-1.00) | 19/20 | 20/20 | 3571 +/- 843 | 363 | 4446/426 | 0.00930 | 0.00930 | 6407 | 1.05 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `graphify` | 20/0 | 20/20 (0.84-1.00) | 18/20 | 20/20 | 4443 +/- 1260 | 391 | 5596/464 | 0.01160 | 0.01160 | 6776 | 1.10 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `native` | 20/0 | 20/20 (0.84-1.00) | 19/20 | 20/20 | 3754 +/- 952 | 372 | 4612/436 | 0.00970 | 0.00970 | 6272 | 1.05 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `neg-output-keeps-first-file` | 20/0 | 20/20 (0.84-1.00) | 0/20 | 20/20 | 8383 +/- 1789 | 798 | 10202/936 | 0.02340 | 0.02340 | 12451 | 2.00 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `neg-skill-strips-work` | 20/0 | 20/20 (0.84-1.00) | 9/20 | 20/20 | 5919 +/- 2868 | 395 | 7440/471 | 0.01520 | 0.01520 | 7929 | 1.55 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `neg-stripped-failure` | 20/0 | 12/20 (0.39-0.78) | 6/20 | 20/20 | 3971 +/- 1809 | 670 | 4651/789 | 0.00860 | 0.01430 | 10649 | 1.70 | 1/2 | 11/18 | failing-tests-js:10/10 failing-tests-py:2/10 |
| `ponytail` | 20/0 | 20/20 (0.84-1.00) | 18/20 | 20/20 | 5764 +/- 1320 | 301 | 6902/366 | 0.01540 | 0.01540 | 5952 | 1.10 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `ponytail+caveman` | 20/0 | 19/20 (0.76-0.99) | 19/20 | 20/20 | 7219 +/- 1595 | 214 | 8597/265 | 0.01850 | 0.01950 | 5153 | 1.05 | 2/2 | 17/18 | failing-tests-js:10/10 failing-tests-py:9/10 |
| `rtk-err` | 20/0 | 20/20 (0.84-1.00) | 17/20 | 20/20 | 3410 +/- 1118 | 407 | 4095/477 | 0.00910 | 0.00910 | 6857 | 1.15 | 2/2 | 18/18 | failing-tests-js:10/10 failing-tests-py:10/10 |
| `rtk-test` | 20/0 | 10/20 (0.30-0.70) | 9/20 | 20/20 | 2878 +/- 1394 | 614 | 3248/724 | 0.00690 | 0.01370 | 9553 | 1.55 | 1/2 | 9/18 | failing-tests-js:10/10 failing-tests-py:0/10 |

#### feature

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 20/0 | 19/20 (0.76-0.99) | 16/20 | 20/20 | 3081 +/- 1359 | 360 | 3544/464 | 0.00630 | 0.00670 | 5538 | 1.20 | 2/2 | 17/18 | feature-js-cart:9/10 feature-py-shop:10/10 |
| `concise` | 20/0 | 20/20 (0.84-1.00) | 18/20 | 20/20 | 1254 +/- 598 | 475 | 1345/596 | 0.00430 | 0.00430 | 6359 | 1.10 | 2/2 | 18/18 | feature-js-cart:10/10 feature-py-shop:10/10 |
| `graft` | 20/0 | 20/20 (0.84-1.00) | 19/20 | 20/20 | 1468 +/- 485 | 487 | 1594/614 | 0.00470 | 0.00470 | 6838 | 1.05 | 2/2 | 18/18 | feature-js-cart:10/10 feature-py-shop:10/10 |
| `graphify` | 20/0 | 20/20 (0.84-1.00) | 18/20 | 20/20 | 2395 +/- 954 | 604 | 2624/748 | 0.00640 | 0.00640 | 7567 | 1.10 | 2/2 | 18/18 | feature-js-cart:10/10 feature-py-shop:10/10 |
| `native` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 1086 +/- 81 | 490 | 1085/612 | 0.00410 | 0.00410 | 6119 | 1.00 | 2/2 | 18/18 | feature-js-cart:10/10 feature-py-shop:10/10 |
| `neg-output-keeps-first-file` | 20/0 | 8/20 (0.22-0.61) | 0/20 | 20/20 | 3224 +/- 548 | 987 | 3494/1233 | 0.00970 | 0.02410 | 13390 | 2.00 | 0/2 | 8/18 | feature-js-cart:0/10 feature-py-shop:8/10 |
| `neg-skill-strips-work` | 20/0 | 17/20 (0.64-0.95) | 11/20 | 20/20 | 1778 +/- 847 | 471 | 2114/588 | 0.00510 | 0.00590 | 7694 | 1.45 | 2/2 | 15/18 | feature-js-cart:7/10 feature-py-shop:10/10 |
| `ponytail` | 20/0 | 18/20 (0.70-0.97) | 16/20 | 20/20 | 3844 +/- 1713 | 622 | 4107/786 | 0.00900 | 0.01000 | 7972 | 1.20 | 2/2 | 16/18 | feature-js-cart:8/10 feature-py-shop:10/10 |
| `ponytail+caveman` | 20/0 | 20/20 (0.84-1.00) | 17/20 | 20/20 | 5184 +/- 2100 | 394 | 5806/504 | 0.00590 | 0.00590 | 6231 | 1.15 | 2/2 | 18/18 | feature-js-cart:10/10 feature-py-shop:10/10 |

#### index_reuse

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 2677 +/- 135 | 150 | 3266/194 | 0.00420 | 0.00420 | 3724 | 1.00 | 2/2 | 18/18 | reuse-js-order:10/10 reuse-py-order:10/10 |
| `concise` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 1285 +/- 147 | 241 | 1634/298 | 0.00310 | 0.00310 | 4381 | 1.00 | 2/2 | 18/18 | reuse-js-order:10/10 reuse-py-order:10/10 |
| `graft` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 1549 +/- 218 | 287 | 1898/355 | 0.00370 | 0.00370 | 5088 | 1.00 | 2/2 | 18/18 | reuse-js-order:10/10 reuse-py-order:10/10 |
| `graphify` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 2267 +/- 452 | 267 | 2824/330 | 0.00450 | 0.00450 | 4910 | 1.00 | 2/2 | 18/18 | reuse-js-order:10/10 reuse-py-order:10/10 |
| `native` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 1293 +/- 136 | 272 | 1606/334 | 0.00330 | 0.00330 | 4795 | 1.00 | 2/2 | 18/18 | reuse-js-order:10/10 reuse-py-order:10/10 |
| `ponytail` | 20/0 | 19/20 (0.76-0.99) | 15/20 | 20/20 | 4024 +/- 1715 | 253 | 4773/319 | 0.00700 | 0.00740 | 5130 | 1.25 | 2/2 | 17/18 | reuse-js-order:10/10 reuse-py-order:9/10 |
| `ponytail+caveman` | 20/0 | 20/20 (0.84-1.00) | 17/20 | 17/20 | 5333 +/- 1942 | 161 | 6268/209 | 0.00440 | 0.00440 | 4406 | 1.15 | 2/2 | 18/18 | reuse-js-order:10/10 reuse-py-order:10/10 |

#### maintenance

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 10/0 | 9/10 (0.60-0.98) | 0/0 | 10/10 | 7112 +/- 1196 | 819 | 8286/1051 | 0.01350 | 0.01500 | 13374 | 3.00 | 1/1 | 8/9 | maintenance-py-notes:9/10 |
| `concise` | 10/0 | 10/10 (0.72-1.00) | 0/0 | 10/10 | 2630 +/- 89 | 980 | 3149/1239 | 0.00930 | 0.00930 | 14905 | 3.00 | 1/1 | 9/9 | maintenance-py-notes:10/10 |
| `graft` | 10/0 | 10/10 (0.72-1.00) | 0/0 | 10/10 | 4184 +/- 66 | 1070 | 5009/1334 | 0.01170 | 0.01170 | 17446 | 3.00 | 1/1 | 9/9 | maintenance-py-notes:10/10 |
| `graphify` | 10/0 | 10/10 (0.72-1.00) | 0/0 | 10/10 | 5477 +/- 114 | 1223 | 6365/1517 | 0.01390 | 0.01390 | 18351 | 3.00 | 1/1 | 9/9 | maintenance-py-notes:10/10 |
| `native` | 10/0 | 10/10 (0.72-1.00) | 0/0 | 10/10 | 2710 +/- 121 | 1128 | 3070/1411 | 0.01010 | 0.01010 | 15864 | 3.00 | 1/1 | 9/9 | maintenance-py-notes:10/10 |
| `neg-output-keeps-first-file` | 10/0 | 3/10 (0.11-0.60) | 0/0 | 10/10 | 4610 +/- 1587 | 1534 | 5334/1903 | 0.01480 | 0.04950 | 21343 | 4.10 | 0/1 | 3/9 | maintenance-py-notes:3/10 |
| `neg-skill-strips-work` | 10/0 | 8/10 (0.49-0.94) | 0/0 | 10/10 | 3744 +/- 819 | 925 | 4894/1172 | 0.01080 | 0.01340 | 17014 | 3.90 | 1/1 | 7/9 | maintenance-py-notes:8/10 |
| `ponytail` | 10/0 | 10/10 (0.72-1.00) | 0/0 | 10/10 | 9522 +/- 1851 | 966 | 10979/1232 | 0.01760 | 0.01760 | 15656 | 3.40 | 1/1 | 9/9 | maintenance-py-notes:10/10 |
| `ponytail+caveman` | 10/0 | 10/10 (0.72-1.00) | 0/0 | 10/10 | 14211 +/- 2537 | 1003 | 16058/1280 | 0.01690 | 0.01690 | 16359 | 3.30 | 1/1 | 9/9 | maintenance-py-notes:10/10 |

#### mixed_language

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 2342 +/- 34 | 292 | 2676/371 | 0.00450 | 0.00450 | 4657 | 1.00 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |
| `concise` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 882 +/- 41 | 315 | 1044/398 | 0.00300 | 0.00300 | 4958 | 1.00 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |
| `graft` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 1189 +/- 74 | 360 | 1382/448 | 0.00360 | 0.00360 | 5642 | 1.00 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |
| `graphify` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 1451 +/- 480 | 377 | 1698/467 | 0.00400 | 0.00400 | 5691 | 1.00 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |
| `native` | 20/0 | 20/20 (0.84-1.00) | 20/20 | 20/20 | 881 +/- 47 | 337 | 1016/422 | 0.00310 | 0.00310 | 5074 | 1.00 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |
| `neg-output-keeps-first-file` | 20/0 | 0/20 (0.00-0.16) | 0/20 | 20/20 | 2549 +/- 145 | 735 | 2921/885 | 0.00730 | n/a | 10745 | 2.00 | 0/2 | 0/18 | mixed-config-units:0/10 mixed-py-js:0/10 |
| `neg-skill-strips-work` | 20/0 | 6/20 (0.15-0.52) | 5/20 | 20/20 | 1558 +/- 384 | 209 | 2241/262 | 0.00360 | 0.01180 | 6507 | 1.75 | 0/2 | 6/18 | mixed-config-units:2/10 mixed-py-js:4/10 |
| `ponytail` | 20/0 | 20/20 (0.84-1.00) | 16/20 | 20/20 | 3527 +/- 1516 | 431 | 3933/533 | 0.00720 | 0.00720 | 6643 | 1.20 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |
| `ponytail+caveman` | 20/0 | 20/20 (0.84-1.00) | 19/20 | 20/20 | 4475 +/- 1148 | 300 | 5043/382 | 0.00390 | 0.00390 | 4836 | 1.05 | 2/2 | 18/18 | mixed-config-units:10/10 mixed-py-js:10/10 |

#### refactor

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 10/0 | 10/10 (0.72-1.00) | 10/10 | 10/10 | 2293 +/- 8 | 245 | 2688/316 | 0.00430 | 0.00430 | 4144 | 1.00 | 1/1 | 9/9 | refactor-py-validators:10/10 |
| `concise` | 10/0 | 10/10 (0.72-1.00) | 10/10 | 10/10 | 824 +/- 9 | 259 | 1055/333 | 0.00270 | 0.00270 | 4266 | 1.00 | 1/1 | 9/9 | refactor-py-validators:10/10 |
| `graft` | 10/0 | 10/10 (0.72-1.00) | 10/10 | 10/10 | 1232 +/- 21 | 269 | 1556/345 | 0.00330 | 0.00330 | 4726 | 1.00 | 1/1 | 9/9 | refactor-py-validators:10/10 |
| `graphify` | 10/0 | 10/10 (0.72-1.00) | 10/10 | 10/10 | 716 +/- 48 | 290 | 889/367 | 0.00270 | 0.00270 | 4523 | 1.00 | 1/1 | 9/9 | refactor-py-validators:10/10 |
| `native` | 10/0 | 10/10 (0.72-1.00) | 10/10 | 10/10 | 829 +/- 31 | 287 | 1028/364 | 0.00280 | 0.00280 | 4514 | 1.00 | 1/1 | 9/9 | refactor-py-validators:10/10 |
| `neg-output-keeps-first-file` | 10/0 | 0/10 (0.00-0.28) | 0/10 | 10/10 | 2097 +/- 81 | 596 | 2571/753 | 0.00630 | n/a | 9543 | 2.00 | 0/1 | 0/9 | refactor-py-validators:0/10 |
| `neg-skill-strips-work` | 10/0 | 3/10 (0.11-0.60) | 3/10 | 10/10 | 1327 +/- 303 | 181 | 2000/233 | 0.00320 | 0.01060 | 6192 | 1.70 | 0/1 | 3/9 | refactor-py-validators:3/10 |
| `ponytail` | 10/0 | 10/10 (0.72-1.00) | 9/10 | 10/10 | 3029 +/- 965 | 289 | 3500/366 | 0.00530 | 0.00530 | 5024 | 1.10 | 1/1 | 9/9 | refactor-py-validators:10/10 |
| `ponytail+caveman` | 10/0 | 10/10 (0.72-1.00) | 10/10 | 10/10 | 4179 +/- 16 | 248 | 4764/320 | 0.00300 | 0.00300 | 4423 | 1.00 | 1/1 | 9/9 | refactor-py-validators:10/10 |

### Model `qwen`

#### compile_repair

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 15734 +/- 1959 | 3902 | 13222/4181 | n/a (local) | n/a | 53671 | 3.00 | 0/2 | 0/2 | compile-repair-js:0/2 compile-repair-spx:0/2 |
| `concise` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 2/4 | 7990 +/- 3841 | 2320 | 6704/2589 | n/a (local) | n/a | 27510 | 3.00 | 0/2 | 0/2 | compile-repair-js:0/2 compile-repair-spx:0/2 |
| `graft` | 1/0 | 0/1 (0.00-0.79) | 0/1 | 0/1 | 8634 +/- n/a | 1206 | 9394/1209 | n/a (local) | n/a | 28384 | 3.00 | 0/1 | 0/0 | compile-repair-js:0/1 |
| `graphify` | 1/1 | 0/1 (0.00-0.79) | 0/1 | 0/1 | 10011 +/- n/a | 2401 | 9891/3043 | n/a (local) | n/a | 28092 | 3.00 | 0/1 | 0/0 | compile-repair-js:0/1 |
| `native` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 2/4 | 8894 +/- 3397 | 2540 | 7557/2828 | n/a (local) | n/a | 40207 | 3.00 | 0/2 | 0/2 | compile-repair-js:0/2 compile-repair-spx:0/2 |
| `neg-output-keeps-first-file` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 3/4 | 8846 +/- 3342 | 2518 | 7550/2807 | n/a (local) | n/a | 38151 | 3.00 | 0/2 | 0/2 | compile-repair-js:0/2 compile-repair-spx:0/2 |
| `neg-skill-strips-work` | 1/0 | 0/1 (0.00-0.79) | 0/1 | 0/1 | 8842 +/- n/a | 1354 | 9424/1584 | n/a (local) | n/a | 34739 | 3.00 | 0/1 | 0/0 | compile-repair-js:0/1 |
| `ponytail` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 14765 +/- 4213 | 2954 | 12826/3010 | n/a (local) | n/a | 36325 | 3.00 | 0/2 | 0/2 | compile-repair-js:0/2 compile-repair-spx:0/2 |
| `ponytail+caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 17287 +/- 3155 | 1347 | 17334/1348 | n/a (local) | n/a | 32325 | 3.00 | 0/2 | 0/2 | compile-repair-js:0/2 compile-repair-spx:0/2 |

#### failing_tests

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 14056 +/- 2264 | 2600 | 12475/2795 | n/a (local) | n/a | 43167 | 2.00 | 0/2 | 0/2 | failing-tests-js:0/2 failing-tests-py:0/2 |
| `concise` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 2/4 | 10749 +/- 2690 | 2382 | 9226/2490 | n/a (local) | n/a | 34603 | 2.00 | 0/2 | 0/2 | failing-tests-js:0/2 failing-tests-py:0/2 |
| `native` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 10111 +/- 3042 | 1854 | 9101/1928 | n/a (local) | n/a | 26195 | 2.00 | 0/2 | 0/2 | failing-tests-js:0/2 failing-tests-py:0/2 |
| `neg-output-keeps-first-file` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 10423 +/- 2216 | 2026 | 9283/2118 | n/a (local) | n/a | 27847 | 2.00 | 0/2 | 0/2 | failing-tests-js:0/2 failing-tests-py:0/2 |
| `ponytail` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 14942 +/- 1552 | 2510 | 13626/2988 | n/a (local) | n/a | 40184 | 2.00 | 0/2 | 0/2 | failing-tests-js:0/2 failing-tests-py:0/2 |
| `ponytail+caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 17206 +/- 2472 | 2198 | 16114/2348 | n/a (local) | n/a | 32464 | 2.00 | 0/2 | 0/2 | failing-tests-js:0/2 failing-tests-py:0/2 |

#### feature

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 7994 +/- 1136 | 1967 | 6374/2072 | n/a (local) | n/a | 25690 | 2.00 | 0/2 | 0/2 | feature-js-cart:0/2 feature-py-shop:0/2 |
| `concise` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 2954 +/- 349 | 744 | 2397/770 | n/a (local) | n/a | 13812 | 2.00 | 0/2 | 0/2 | feature-js-cart:0/2 feature-py-shop:0/2 |
| `native` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 3866 +/- 1200 | 1517 | 2540/1567 | n/a (local) | n/a | 21463 | 2.00 | 0/2 | 0/2 | feature-js-cart:0/2 feature-py-shop:0/2 |
| `neg-output-keeps-first-file` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 3520 +/- 1366 | 1192 | 2516/1223 | n/a (local) | n/a | 11876 | 2.00 | 0/2 | 0/2 | feature-js-cart:0/2 feature-py-shop:0/2 |
| `ponytail` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 8756 +/- 1636 | 1970 | 7131/2020 | n/a (local) | n/a | 26649 | 2.00 | 0/2 | 0/2 | feature-js-cart:0/2 feature-py-shop:0/2 |
| `ponytail+caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 10804 +/- 1348 | 1395 | 9884/1462 | n/a (local) | n/a | 16814 | 2.00 | 0/2 | 0/2 | feature-js-cart:0/2 feature-py-shop:0/2 |

#### index_reuse

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 5920 +/- 547 | 255 | 5924/262 | n/a (local) | n/a | 8078 | 2.00 | 0/2 | 0/2 | reuse-js-order:0/2 reuse-py-order:0/2 |
| `concise` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 2770 +/- 346 | 130 | 2792/132 | n/a (local) | n/a | 3276 | 2.00 | 0/2 | 0/2 | reuse-js-order:0/2 reuse-py-order:0/2 |
| `native` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 3031 +/- 756 | 446 | 2735/454 | n/a (local) | n/a | 5295 | 2.00 | 0/2 | 0/2 | reuse-js-order:0/2 reuse-py-order:0/2 |
| `ponytail` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 7733 +/- 1816 | 964 | 7053/978 | n/a (local) | n/a | 9073 | 2.00 | 0/2 | 0/2 | reuse-js-order:0/2 reuse-py-order:0/2 |
| `ponytail+caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 9538 +/- 331 | 155 | 9769/162 | n/a (local) | n/a | 3844 | 2.00 | 0/2 | 0/2 | reuse-js-order:0/2 reuse-py-order:0/2 |

#### maintenance

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 2/0 | 0/2 (0.00-0.66) | 0/0 | 0/2 | 4870 +/- 764 | 608 | 4444/616 | n/a (local) | n/a | 12794 | 2.00 | 0/1 | 0/1 | maintenance-py-notes:0/2 |
| `concise` | 2/0 | 0/2 (0.00-0.66) | 0/0 | 1/2 | 3088 +/- 301 | 1365 | 1796/1368 | n/a (local) | n/a | 28098 | 2.00 | 0/1 | 0/1 | maintenance-py-notes:0/2 |
| `native` | 2/0 | 0/2 (0.00-0.66) | 0/0 | 1/2 | 2618 +/- 284 | 1003 | 1688/1008 | n/a (local) | n/a | 8752 | 2.00 | 0/1 | 0/1 | maintenance-py-notes:0/2 |
| `neg-output-keeps-first-file` | 2/0 | 0/2 (0.00-0.66) | 0/0 | 1/2 | 2254 +/- 248 | 627 | 1700/630 | n/a (local) | n/a | 13900 | 2.00 | 0/1 | 0/1 | maintenance-py-notes:0/2 |
| `ponytail` | 2/0 | 0/2 (0.00-0.66) | 0/0 | 0/2 | 7014 +/- 1595 | 1439 | 5778/1456 | n/a (local) | n/a | 13139 | 2.00 | 0/1 | 0/1 | maintenance-py-notes:0/2 |
| `ponytail+caveman` | 2/0 | 0/2 (0.00-0.66) | 0/0 | 0/2 | 11660 +/- 1050 | 2558 | 9432/2608 | n/a (local) | n/a | 24907 | 2.00 | 0/1 | 0/1 | maintenance-py-notes:0/2 |

#### mixed_language

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 7786 +/- 2068 | 2206 | 5896/2322 | n/a (local) | n/a | 23607 | 2.00 | 0/2 | 0/2 | mixed-config-units:0/2 mixed-py-js:0/2 |
| `concise` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 2/4 | 3666 +/- 1211 | 1442 | 2383/1512 | n/a (local) | n/a | 19868 | 2.00 | 0/2 | 0/2 | mixed-config-units:0/2 mixed-py-js:0/2 |
| `native` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 2/4 | 3760 +/- 1036 | 1648 | 2257/1709 | n/a (local) | n/a | 17493 | 2.00 | 0/2 | 0/2 | mixed-config-units:0/2 mixed-py-js:0/2 |
| `neg-output-keeps-first-file` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 2/4 | 3882 +/- 990 | 1752 | 2275/1826 | n/a (local) | n/a | 19051 | 2.00 | 0/2 | 0/2 | mixed-config-units:0/2 mixed-py-js:0/2 |
| `ponytail` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 7289 +/- 1244 | 1410 | 6137/1433 | n/a (local) | n/a | 22670 | 2.00 | 0/2 | 0/2 | mixed-config-units:0/2 mixed-py-js:0/2 |
| `ponytail+caveman` | 4/0 | 0/4 (0.00-0.49) | 0/4 | 0/4 | 10651 +/- 1759 | 1620 | 9407/1650 | n/a (local) | n/a | 21188 | 2.00 | 0/2 | 0/2 | mixed-config-units:0/2 mixed-py-js:0/2 |

#### refactor

| arm | trials ok/other | accepted (Wilson 95%) | first attempt (single-step) | valid blocks first | tokens o200k total mean+/-sd | answer tokens | provider in/out (mean) | USD/trial | USD/accepted | completion ms mean | attempts | cold acc. | warm acc. | per-task accepted |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `caveman` | 2/0 | 0/2 (0.00-0.66) | 0/2 | 0/2 | 5449 +/- 792 | 1033 | 4558/1008 | n/a (local) | n/a | 19682 | 2.00 | 0/1 | 0/1 | refactor-py-validators:0/2 |
| `concise` | 2/0 | 0/2 (0.00-0.66) | 0/2 | 0/2 | 1682 +/- 286 | 261 | 1454/255 | n/a (local) | n/a | 5249 | 2.00 | 0/1 | 0/1 | refactor-py-validators:0/2 |
| `native` | 2/0 | 0/2 (0.00-0.66) | 0/2 | 0/2 | 1947 +/- 86 | 490 | 1488/480 | n/a (local) | n/a | 13003 | 2.00 | 0/1 | 0/1 | refactor-py-validators:0/2 |
| `neg-output-keeps-first-file` | 2/0 | 0/2 (0.00-0.66) | 0/2 | 0/2 | 1956 +/- 81 | 494 | 1494/482 | n/a (local) | n/a | 4381 | 2.00 | 0/1 | 0/1 | refactor-py-validators:0/2 |
| `ponytail` | 2/0 | 0/2 (0.00-0.66) | 0/2 | 0/2 | 7745 +/- 1175 | 2099 | 5810/2106 | n/a (local) | n/a | 16297 | 2.00 | 0/1 | 0/1 | refactor-py-validators:0/2 |
| `ponytail+caveman` | 2/0 | 0/2 (0.00-0.66) | 0/2 | 0/2 | 8662 +/- 263 | 436 | 8493/426 | n/a (local) | n/a | 9454 | 2.00 | 0/1 | 0/1 | refactor-py-validators:0/2 |

## Matched comparison against native, with predeclared gates

Matched = same task, repetition and model. Gates (declared in `docs/HARNESS-BENCHMARK-V1.md` before measurement): N at least 10 matched cells; Q accepted delta >= 0 (no loss); C total o200k tokens (skill, retrieval and failed attempts included) down at least 20% and billed cost down at least 20% where billed; T added completion time <= 2000 ms per cell. Negative controls are judged by whether the oracle caught them.

| model | class | arm | matched | accepted delta/cell | total tokens | output tokens | billed cost | added ms/cell | N | Q | C | T | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| haiku | compile_repair | `caveman` | 20 | -0.050 | +119.9% | -0.4% | +54.2% | 1015 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | compile_repair | `concise` | 20 | -0.050 | +11.1% | +3.4% | +8.2% | 918 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | compile_repair | `graft` | 20 | -0.850 | +123.6% | +119.9% | +300.1% | 20572 | pass | FAIL | FAIL | FAIL | **not-recommended** |
| haiku | compile_repair | `graphify` | 10 | -0.800 | +290.7% | +408.4% | +324.1% | 13924 | pass | FAIL | FAIL | FAIL | **untested-cells** |
| haiku | compile_repair | `neg-output-keeps-first-file` | 20 | -0.200 | +97.5% | +32.9% | +66.0% | 5032 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | compile_repair | `neg-skill-strips-work` | 20 | -0.300 | +72.8% | -10.6% | +32.5% | 2535 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | compile_repair | `ponytail` | 20 | 0.000 | +152.3% | +13.3% | +134.5% | 1624 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | compile_repair | `ponytail+caveman` | 20 | -0.100 | +249.9% | -7.3% | +189.5% | 857 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | failing_tests | `caveman` | 20 | 0.000 | +34.4% | -46.0% | +38.4% | -1175 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | failing_tests | `concise` | 20 | 0.000 | +5.2% | -12.7% | +3.7% | -85 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | failing_tests | `graft` | 20 | 0.000 | -4.9% | -2.5% | -3.9% | 134 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | failing_tests | `graphify` | 20 | 0.000 | +18.4% | +5.0% | +19.5% | 503 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | failing_tests | `neg-output-keeps-first-file` | 20 | 0.000 | +123.3% | +114.2% | +141.1% | 6178 | pass | pass | FAIL | FAIL | **control-NOT-detected** |
| haiku | failing_tests | `neg-skill-strips-work` | 20 | 0.000 | +57.7% | +6.0% | +57.3% | 1657 | pass | pass | FAIL | pass | **control-NOT-detected** |
| haiku | failing_tests | `neg-stripped-failure` | 20 | -0.400 | +5.8% | +79.9% | -11.3% | 4377 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | failing_tests | `ponytail` | 20 | 0.000 | +53.5% | -19.1% | +59.4% | -321 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | failing_tests | `ponytail+caveman` | 20 | -0.050 | +92.3% | -42.4% | +91.1% | -1119 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | failing_tests | `rtk-err` | 20 | 0.000 | -9.2% | +9.2% | -6.5% | 585 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | failing_tests | `rtk-test` | 20 | -0.500 | -23.3% | +64.8% | -29.1% | 3281 | pass | FAIL | pass | FAIL | **not-recommended** |
| haiku | feature | `caveman` | 20 | -0.050 | +183.7% | -26.5% | +52.5% | -581 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | feature | `concise` | 20 | 0.000 | +15.4% | -3.1% | +4.4% | 240 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | feature | `graft` | 20 | 0.000 | +35.2% | -0.6% | +12.5% | 720 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | feature | `graphify` | 20 | 0.000 | +120.6% | +23.2% | +53.6% | 1448 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | feature | `neg-output-keeps-first-file` | 20 | -0.600 | +196.9% | +101.4% | +133.1% | 7271 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | feature | `neg-skill-strips-work` | 20 | -0.150 | +63.8% | -3.8% | +21.9% | 1576 | pass | FAIL | FAIL | pass | **control-NOT-detected** |
| haiku | feature | `ponytail` | 20 | -0.100 | +254.0% | +26.9% | +116.4% | 1853 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | feature | `ponytail+caveman` | 20 | 0.000 | +377.4% | -19.5% | +42.0% | 112 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | index_reuse | `caveman` | 20 | 0.000 | +107.0% | -44.9% | +29.3% | -1071 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | index_reuse | `concise` | 20 | 0.000 | -0.7% | -11.6% | -4.6% | -414 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | index_reuse | `graft` | 20 | 0.000 | +19.8% | +5.5% | +12.2% | 292 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | index_reuse | `graphify` | 20 | 0.000 | +75.3% | -2.0% | +36.7% | 115 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | index_reuse | `ponytail` | 20 | -0.050 | +211.2% | -7.1% | +115.2% | 335 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | index_reuse | `ponytail+caveman` | 20 | 0.000 | +312.4% | -40.7% | +35.7% | -389 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | maintenance | `caveman` | 10 | -0.100 | +162.4% | -27.4% | +33.7% | -2490 | pass | FAIL | FAIL | pass | **not-recommended** |
| haiku | maintenance | `concise` | 10 | 0.000 | -3.0% | -13.1% | -7.7% | -959 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | maintenance | `graft` | 10 | 0.000 | +54.4% | -5.1% | +15.4% | 1582 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | maintenance | `graphify` | 10 | 0.000 | +102.1% | +8.5% | +37.8% | 2487 | pass | pass | FAIL | FAIL | **available-no-lift** |
| haiku | maintenance | `neg-output-keeps-first-file` | 10 | -0.700 | +70.1% | +36.0% | +46.7% | 5479 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | maintenance | `neg-skill-strips-work` | 10 | -0.200 | +38.1% | -18.0% | +6.2% | 1150 | pass | FAIL | FAIL | pass | **control-detected** |
| haiku | maintenance | `ponytail` | 10 | 0.000 | +251.3% | -14.4% | +74.1% | -208 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | maintenance | `ponytail+caveman` | 10 | 0.000 | +424.3% | -11.1% | +66.6% | 495 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | mixed_language | `caveman` | 20 | 0.000 | +165.8% | -13.4% | +44.9% | -417 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | mixed_language | `concise` | 20 | 0.000 | +0.1% | -6.6% | -3.1% | -116 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | mixed_language | `graft` | 20 | 0.000 | +34.9% | +6.8% | +15.8% | 568 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | mixed_language | `graphify` | 20 | 0.000 | +64.6% | +11.8% | +28.9% | 617 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | mixed_language | `neg-output-keeps-first-file` | 20 | -1.000 | +189.2% | +118.1% | +134.8% | 5671 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | mixed_language | `neg-skill-strips-work` | 20 | -0.700 | +76.8% | -37.9% | +13.5% | 1433 | pass | FAIL | FAIL | pass | **control-detected** |
| haiku | mixed_language | `ponytail` | 20 | 0.000 | +300.2% | +27.9% | +131.0% | 1569 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | mixed_language | `ponytail+caveman` | 20 | 0.000 | +407.7% | -10.8% | +23.6% | -238 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | refactor | `caveman` | 10 | 0.000 | +176.6% | -14.7% | +49.8% | -370 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | refactor | `concise` | 10 | 0.000 | -0.5% | -9.6% | -4.5% | -248 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | refactor | `graft` | 10 | 0.000 | +48.6% | -6.2% | +15.1% | 212 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | refactor | `graphify` | 10 | 0.000 | -13.6% | +0.9% | -4.3% | 8 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | refactor | `neg-output-keeps-first-file` | 10 | -1.000 | +153.0% | +107.7% | +122.4% | 5029 | pass | FAIL | FAIL | FAIL | **control-detected** |
| haiku | refactor | `neg-skill-strips-work` | 10 | -0.700 | +60.1% | -36.8% | +11.2% | 1677 | pass | FAIL | FAIL | pass | **control-detected** |
| haiku | refactor | `ponytail` | 10 | 0.000 | +265.4% | +0.7% | +87.1% | 509 | pass | pass | FAIL | pass | **available-no-lift** |
| haiku | refactor | `ponytail+caveman` | 10 | 0.000 | +404.2% | -13.5% | +4.8% | -92 | pass | pass | FAIL | pass | **available-no-lift** |
| qwen | compile_repair | `caveman` | 4 | 0.000 | +76.9% | +53.6% | n/a | 13464 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | compile_repair | `concise` | 4 | 0.000 | -10.2% | -8.6% | n/a | -12697 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | compile_repair | `graft` | 1 | 0.000 | -26.4% | -58.1% | n/a | -8217 | FAIL | pass | pass | pass | **pilot-only** |
| qwen | compile_repair | `graphify` | 1 | 0.000 | -14.6% | -16.7% | n/a | -8509 | FAIL | pass | FAIL | pass | **untested-cells** |
| qwen | compile_repair | `neg-output-keeps-first-file` | 4 | 0.000 | -0.5% | -0.9% | n/a | -2056 | FAIL | pass | FAIL | pass | **control-NOT-detected** |
| qwen | compile_repair | `neg-skill-strips-work` | 1 | 0.000 | -24.6% | -53.0% | n/a | -1862 | FAIL | pass | pass | pass | **control-NOT-detected** |
| qwen | compile_repair | `ponytail` | 4 | 0.000 | +66.0% | +16.3% | n/a | -3882 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | compile_repair | `ponytail+caveman` | 4 | 0.000 | +94.4% | -46.9% | n/a | -7882 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | failing_tests | `caveman` | 4 | 0.000 | +39.0% | +40.2% | n/a | 16972 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | failing_tests | `concise` | 4 | 0.000 | +6.3% | +28.5% | n/a | 8408 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | failing_tests | `neg-output-keeps-first-file` | 4 | 0.000 | +3.1% | +9.2% | n/a | 1652 | FAIL | pass | FAIL | pass | **control-NOT-detected** |
| qwen | failing_tests | `ponytail` | 4 | 0.000 | +47.8% | +35.3% | n/a | 13989 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | failing_tests | `ponytail+caveman` | 4 | 0.000 | +70.2% | +18.6% | n/a | 6269 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | feature | `caveman` | 4 | 0.000 | +106.7% | +29.7% | n/a | 4227 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | feature | `concise` | 4 | 0.000 | -23.6% | -51.0% | n/a | -7651 | FAIL | pass | pass | pass | **pilot-only** |
| qwen | feature | `neg-output-keeps-first-file` | 4 | 0.000 | -8.9% | -21.4% | n/a | -9587 | FAIL | pass | FAIL | pass | **control-NOT-detected** |
| qwen | feature | `ponytail` | 4 | 0.000 | +126.5% | +29.8% | n/a | 5186 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | feature | `ponytail+caveman` | 4 | 0.000 | +179.4% | -8.0% | n/a | -4649 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | index_reuse | `caveman` | 4 | 0.000 | +95.3% | -42.8% | n/a | 2783 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | index_reuse | `concise` | 4 | 0.000 | -8.6% | -71.0% | n/a | -2020 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | index_reuse | `ponytail` | 4 | 0.000 | +155.1% | +115.9% | n/a | 3778 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | index_reuse | `ponytail+caveman` | 4 | 0.000 | +214.7% | -65.3% | n/a | -1451 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | maintenance | `caveman` | 2 | 0.000 | +86.0% | -39.4% | n/a | 4042 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | maintenance | `concise` | 2 | 0.000 | +17.9% | +36.1% | n/a | 19346 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | maintenance | `neg-output-keeps-first-file` | 2 | 0.000 | -13.9% | -37.5% | n/a | 5148 | FAIL | pass | FAIL | FAIL | **control-NOT-detected** |
| qwen | maintenance | `ponytail` | 2 | 0.000 | +167.8% | +43.5% | n/a | 4386 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | maintenance | `ponytail+caveman` | 2 | 0.000 | +345.3% | +155.0% | n/a | 16154 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | mixed_language | `caveman` | 4 | 0.000 | +107.1% | +33.8% | n/a | 6114 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | mixed_language | `concise` | 4 | 0.000 | -2.5% | -12.5% | n/a | 2375 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | mixed_language | `neg-output-keeps-first-file` | 4 | 0.000 | +3.2% | +6.3% | n/a | 1558 | FAIL | pass | FAIL | pass | **control-NOT-detected** |
| qwen | mixed_language | `ponytail` | 4 | 0.000 | +93.9% | -14.5% | n/a | 5177 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | mixed_language | `ponytail+caveman` | 4 | 0.000 | +183.3% | -1.7% | n/a | 3695 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | refactor | `caveman` | 2 | 0.000 | +179.9% | +110.6% | n/a | 6678 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | refactor | `concise` | 2 | 0.000 | -13.6% | -46.8% | n/a | -7754 | FAIL | pass | FAIL | pass | **pilot-only** |
| qwen | refactor | `neg-output-keeps-first-file` | 2 | 0.000 | +0.5% | +0.7% | n/a | -8622 | FAIL | pass | FAIL | pass | **control-NOT-detected** |
| qwen | refactor | `ponytail` | 2 | 0.000 | +297.8% | +327.9% | n/a | 3294 | FAIL | pass | FAIL | FAIL | **pilot-only** |
| qwen | refactor | `ponytail+caveman` | 2 | 0.000 | +344.9% | -11.1% | n/a | -3550 | FAIL | pass | FAIL | pass | **pilot-only** |

Token and cost columns show the change against native: `-` = fewer tokens or cheaper, `+` = more.

## Mixed small-then-large cascade (derived)

The large model is asked only when the small model's answer failed the grader; computed from paired trials of the two sizes, not an independent run.

| class | arm | pairs | accepted | escalation rate | mean tokens | mean USD |
| --- | --- | --- | --- | --- | --- | --- |
| compile_repair | `caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 21168 | 0.01010 |
| compile_repair | `concise` | 4 | 4/4 (0.51-1.00) | 1.00 | 10551 | 0.00640 |
| compile_repair | `graft` | 1 | 0/1 (0.00-0.79) | 1.00 | 16116 | 0.01630 |
| compile_repair | `graphify` | 1 | 0/1 (0.00-0.79) | 1.00 | 17254 | 0.01520 |
| compile_repair | `native` | 4 | 4/4 (0.51-1.00) | 1.00 | 11458 | 0.00650 |
| compile_repair | `neg-output-keeps-first-file` | 4 | 3/4 (0.30-0.95) | 1.00 | 14298 | 0.01140 |
| compile_repair | `neg-skill-strips-work` | 1 | 1/1 (0.21-1.00) | 1.00 | 13614 | 0.00730 |
| compile_repair | `ponytail` | 4 | 4/4 (0.51-1.00) | 1.00 | 19833 | 0.01200 |
| compile_repair | `ponytail+caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 23520 | 0.01410 |
| failing_tests | `caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 19004 | 0.01340 |
| failing_tests | `concise` | 4 | 4/4 (0.51-1.00) | 1.00 | 14347 | 0.00910 |
| failing_tests | `native` | 4 | 4/4 (0.51-1.00) | 1.00 | 13712 | 0.00930 |
| failing_tests | `neg-output-keeps-first-file` | 4 | 4/4 (0.51-1.00) | 1.00 | 18959 | 0.02410 |
| failing_tests | `ponytail` | 4 | 4/4 (0.51-1.00) | 1.00 | 21998 | 0.01810 |
| failing_tests | `ponytail+caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 24062 | 0.01760 |
| feature | `caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 10407 | 0.00480 |
| feature | `concise` | 4 | 4/4 (0.51-1.00) | 1.00 | 4022 | 0.00400 |
| feature | `native` | 4 | 4/4 (0.51-1.00) | 1.00 | 4921 | 0.00400 |
| feature | `neg-output-keeps-first-file` | 4 | 1/4 (0.05-0.70) | 1.00 | 6820 | 0.01010 |
| feature | `ponytail` | 4 | 4/4 (0.51-1.00) | 1.00 | 13718 | 0.01200 |
| feature | `ponytail+caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 16630 | 0.01140 |
| index_reuse | `caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 8585 | 0.00420 |
| index_reuse | `concise` | 4 | 4/4 (0.51-1.00) | 1.00 | 4043 | 0.00300 |
| index_reuse | `native` | 4 | 4/4 (0.51-1.00) | 1.00 | 4332 | 0.00330 |
| index_reuse | `ponytail` | 4 | 4/4 (0.51-1.00) | 1.00 | 10829 | 0.00480 |
| index_reuse | `ponytail+caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 14078 | 0.00650 |
| maintenance | `caveman` | 2 | 2/2 (0.34-1.00) | 1.00 | 13486 | 0.01640 |
| maintenance | `concise` | 2 | 2/2 (0.34-1.00) | 1.00 | 5786 | 0.00970 |
| maintenance | `native` | 2 | 2/2 (0.34-1.00) | 1.00 | 5396 | 0.01050 |
| maintenance | `neg-output-keeps-first-file` | 2 | 0/2 (0.00-0.66) | 1.00 | 4544 | 0.00720 |
| maintenance | `ponytail` | 2 | 2/2 (0.34-1.00) | 1.00 | 16766 | 0.01760 |
| maintenance | `ponytail+caveman` | 2 | 2/2 (0.34-1.00) | 1.00 | 24262 | 0.02960 |
| mixed_language | `caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 10126 | 0.00450 |
| mixed_language | `concise` | 4 | 4/4 (0.51-1.00) | 1.00 | 4593 | 0.00330 |
| mixed_language | `native` | 4 | 4/4 (0.51-1.00) | 1.00 | 4611 | 0.00300 |
| mixed_language | `neg-output-keeps-first-file` | 4 | 0/4 (0.00-0.49) | 1.00 | 6388 | 0.00720 |
| mixed_language | `ponytail` | 4 | 4/4 (0.51-1.00) | 1.00 | 11011 | 0.00720 |
| mixed_language | `ponytail+caveman` | 4 | 4/4 (0.51-1.00) | 1.00 | 16146 | 0.01020 |
| refactor | `caveman` | 2 | 2/2 (0.34-1.00) | 1.00 | 7743 | 0.00430 |
| refactor | `concise` | 2 | 2/2 (0.34-1.00) | 1.00 | 2510 | 0.00270 |
| refactor | `native` | 2 | 2/2 (0.34-1.00) | 1.00 | 2752 | 0.00270 |
| refactor | `neg-output-keeps-first-file` | 2 | 0/2 (0.00-0.66) | 1.00 | 4148 | 0.00670 |
| refactor | `ponytail` | 2 | 2/2 (0.34-1.00) | 1.00 | 10486 | 0.00500 |
| refactor | `ponytail+caveman` | 2 | 2/2 (0.34-1.00) | 1.00 | 12840 | 0.00660 |

## Scoped recommendations

Advisory only. No entry changes configuration, routing or skill state. A `qualified-scoped` entry is an input for the HN-05 lock, HN-06 skill preset and HN-16 evidence contracts (`recommendations.json`, `outcomes.json`); anything else keeps its tool available without an automatic-performance claim.

| arm | class | model | verdict | matched | reason |
| --- | --- | --- | --- | --- | --- |
| `caveman` | compile_repair | haiku | **not-recommended** | 20 | accepted delta -0.050 per matched cell (quality loss against native) |
| `concise` | compile_repair | haiku | **not-recommended** | 20 | accepted delta -0.050 per matched cell (quality loss against native) |
| `graft` | compile_repair | haiku | **not-recommended** | 20 | accepted delta -0.850 per matched cell (quality loss against native) |
| `graphify` | compile_repair | haiku | **untested-cells** | 10 | 10 trial(s) did not run with the real tool or model |
| `neg-output-keeps-first-file` | compile_repair | haiku | **control-detected** | 20 | accepted delta -0.200 per matched cell: the oracle caught the stripped work |
| `neg-skill-strips-work` | compile_repair | haiku | **control-detected** | 20 | accepted delta -0.300 per matched cell: the oracle caught the stripped work |
| `ponytail` | compile_repair | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `ponytail+caveman` | compile_repair | haiku | **not-recommended** | 20 | accepted delta -0.100 per matched cell (quality loss against native) |
| `caveman` | failing_tests | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `concise` | failing_tests | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graft` | failing_tests | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graphify` | failing_tests | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `neg-output-keeps-first-file` | failing_tests | haiku | **control-NOT-detected** | 20 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `neg-skill-strips-work` | failing_tests | haiku | **control-NOT-detected** | 20 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `neg-stripped-failure` | failing_tests | haiku | **control-detected** | 20 | accepted delta -0.400 per matched cell: the oracle caught the stripped work |
| `ponytail` | failing_tests | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `ponytail+caveman` | failing_tests | haiku | **not-recommended** | 20 | accepted delta -0.050 per matched cell (quality loss against native) |
| `rtk-err` | failing_tests | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `rtk-test` | failing_tests | haiku | **not-recommended** | 20 | accepted delta -0.500 per matched cell (quality loss against native) |
| `caveman` | feature | haiku | **not-recommended** | 20 | accepted delta -0.050 per matched cell (quality loss against native) |
| `concise` | feature | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graft` | feature | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graphify` | feature | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `neg-output-keeps-first-file` | feature | haiku | **control-detected** | 20 | accepted delta -0.600 per matched cell: the oracle caught the stripped work |
| `neg-skill-strips-work` | feature | haiku | **control-NOT-detected** | 20 | accepted delta -0.150: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | feature | haiku | **not-recommended** | 20 | accepted delta -0.100 per matched cell (quality loss against native) |
| `ponytail+caveman` | feature | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `caveman` | index_reuse | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `concise` | index_reuse | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graft` | index_reuse | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graphify` | index_reuse | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `ponytail` | index_reuse | haiku | **not-recommended** | 20 | accepted delta -0.050 per matched cell (quality loss against native) |
| `ponytail+caveman` | index_reuse | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `caveman` | maintenance | haiku | **not-recommended** | 10 | accepted delta -0.100 per matched cell (quality loss against native) |
| `concise` | maintenance | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graft` | maintenance | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graphify` | maintenance | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `neg-output-keeps-first-file` | maintenance | haiku | **control-detected** | 10 | accepted delta -0.700 per matched cell: the oracle caught the stripped work |
| `neg-skill-strips-work` | maintenance | haiku | **control-detected** | 10 | accepted delta -0.200 per matched cell: the oracle caught the stripped work |
| `ponytail` | maintenance | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `ponytail+caveman` | maintenance | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `caveman` | mixed_language | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `concise` | mixed_language | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graft` | mixed_language | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graphify` | mixed_language | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `neg-output-keeps-first-file` | mixed_language | haiku | **control-detected** | 20 | accepted delta -1.000 per matched cell: the oracle caught the stripped work |
| `neg-skill-strips-work` | mixed_language | haiku | **control-detected** | 20 | accepted delta -0.700 per matched cell: the oracle caught the stripped work |
| `ponytail` | mixed_language | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `ponytail+caveman` | mixed_language | haiku | **available-no-lift** | 20 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `caveman` | refactor | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `concise` | refactor | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graft` | refactor | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `graphify` | refactor | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `neg-output-keeps-first-file` | refactor | haiku | **control-detected** | 10 | accepted delta -1.000 per matched cell: the oracle caught the stripped work |
| `neg-skill-strips-work` | refactor | haiku | **control-detected** | 10 | accepted delta -0.700 per matched cell: the oracle caught the stripped work |
| `ponytail` | refactor | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `ponytail+caveman` | refactor | haiku | **available-no-lift** | 10 | no measurable lift on this class and model: stays available, no automatic-performance claim |
| `caveman` | compile_repair | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `concise` | compile_repair | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `graft` | compile_repair | qwen | **pilot-only** | 1 | 1 matched cells (< 10): supports no default |
| `graphify` | compile_repair | qwen | **untested-cells** | 1 | 1 trial(s) did not run with the real tool or model |
| `neg-output-keeps-first-file` | compile_repair | qwen | **control-NOT-detected** | 4 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `neg-skill-strips-work` | compile_repair | qwen | **control-NOT-detected** | 1 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | compile_repair | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `ponytail+caveman` | compile_repair | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `caveman` | failing_tests | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `concise` | failing_tests | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `neg-output-keeps-first-file` | failing_tests | qwen | **control-NOT-detected** | 4 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | failing_tests | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `ponytail+caveman` | failing_tests | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `caveman` | feature | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `concise` | feature | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `neg-output-keeps-first-file` | feature | qwen | **control-NOT-detected** | 4 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | feature | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `ponytail+caveman` | feature | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `caveman` | index_reuse | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `concise` | index_reuse | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `ponytail` | index_reuse | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `ponytail+caveman` | index_reuse | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `caveman` | maintenance | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `concise` | maintenance | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `neg-output-keeps-first-file` | maintenance | qwen | **control-NOT-detected** | 2 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | maintenance | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `ponytail+caveman` | maintenance | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `caveman` | mixed_language | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `concise` | mixed_language | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `neg-output-keeps-first-file` | mixed_language | qwen | **control-NOT-detected** | 4 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | mixed_language | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `ponytail+caveman` | mixed_language | qwen | **pilot-only** | 4 | 4 matched cells (< 10): supports no default |
| `caveman` | refactor | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `concise` | refactor | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `neg-output-keeps-first-file` | refactor | qwen | **control-NOT-detected** | 2 | accepted delta +0.000: the oracle did not separate the control; the benchmark cannot be trusted here |
| `ponytail` | refactor | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |
| `ponytail+caveman` | refactor | qwen | **pilot-only** | 2 | 2 matched cells (< 10): supports no default |

