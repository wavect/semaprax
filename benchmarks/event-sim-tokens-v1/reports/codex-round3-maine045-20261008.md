# ShiftSim Codex round 3: complete campaign report

All ten planned attempts were recorded and accepted: five SEMAPRAX and five TypeScript. Each accepted all 15 independent cases. The campaign exited clean, every attempt was marked clean-comparison eligible, and the authoritative recount reports `complete: true`, `clean_comparison_eligible: true`, and no winner. The observed TypeScript runs had lower model requests, input usage, final source proxy, conditional API-equivalent estimate, and shared-host wall times. These are measurements from this run; they do not establish a language advantage or a causal explanation.

The trial order was SEMAPRAX 01, TypeScript 01–02, SEMAPRAX 02–03, TypeScript 03–04, SEMAPRAX 04–05, and TypeScript 05. Every row reconciled its exec transcript with its task-owned rollout trace. Each attempt used the requested `gpt-6.1-sol` model at `medium` effort; the provider-resolved model is unavailable. Outer Codex CLI turns were one per attempt (five per arm). Those outer turns are separate from reconciled model request turns and tool items.

| Arm | Accepted / recorded | Model requests | Raw input | Cached input | Cache-write input | Output | Legacy net input proxy | Final authored source proxy | Conditional cost / accepted | Mean agent / acceptance seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| SEMAPRAX | 5 / 5 | 159 total; 31.8 mean | 7,149,988 total; 1,429,997.6 mean | 6,852,608 total; 1,370,521.6 mean | 0 | 86,895 total; 17,379 mean | 4,965,010 total; 993,002 mean | 42,408 total; 8,481.6 mean | $0.4297942 | 589.767 / 20.145 |
| TypeScript | 5 / 5 | 31 total; 6.2 mean | 555,643 total; 111,128.6 mean | 442,624 total; 88,524.8 mean | 0 | 36,545 total; 7,309 mean | 129,920 total; 25,984 mean | 24,258 total; 4,851.6 mean | $0.1271502 | 213.194 / 5.513 |

The legacy net input value is a historical proxy, not exact task-only input: it subtracts first-request input multiplied by request count from the summed input. Raw input, cached input, cache writes, and output are reported separately. Authored source is the final-inventory Claude BPE proxy, not generated text, cumulative edits, current-model tokenization, or billing tokens.

| Attempt | Status | Model requests | Legacy net input proxy | Final authored source proxy | Conditional API-equivalent estimate | Agent seconds | Acceptance seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| SEMAPRAX 01 | accepted | 42 | 1,379,357 | 9,133 | $0.554322 | 787.458 | 19.405 |
| TypeScript 01 | accepted | 6 | 23,325 | 5,245 | $0.155827 | 254.993 | 4.800 |
| TypeScript 02 | accepted | 8 | 42,950 | 4,833 | $0.132038 | 248.003 | 4.107 |
| SEMAPRAX 02 | accepted | 25 | 761,539 | 9,126 | $0.393570 | 548.279 | 18.800 |
| SEMAPRAX 03 | accepted | 31 | 978,915 | 9,082 | $0.432969 | 732.170 | 18.920 |
| TypeScript 03 | accepted | 5 | 15,182 | 4,530 | $0.114348 | 181.030 | 6.404 |
| TypeScript 04 | accepted | 6 | 25,055 | 4,989 | $0.128030 | 218.917 | 7.963 |
| SEMAPRAX 04 | accepted | 36 | 1,212,785 | 7,882 | $0.458594 | 472.398 | 24.963 |
| SEMAPRAX 05 | accepted | 25 | 632,414 | 7,185 | $0.309516 | 408.530 | 18.638 |
| TypeScript 05 | accepted | 6 | 23,408 | 4,661 | $0.105508 | 163.025 | 4.289 |

The conditional estimate uses the campaign’s frozen Standard short-context price book dated 2026-10-08: $2 per million input tokens, $0.10 per million cached input tokens, $2.50 per million cache-write tokens, and $10 per million output tokens, with the recorded per-request input-size assumptions. It includes all recorded attempts and divides by accepted tasks. It is not actual provider billing; actual billed USD remains unavailable (`null`). Provider-resolved model identity, per-request fixed system/tool/task/history composition, and fixed harness context tokens also remain unavailable (`null`); no context baseline is subtracted.

The one-turn calibration is separate and was not subtracted from trial usage or cost: one outer CLI turn and one model request, 13,099 raw input tokens, 4,608 cached input tokens, 0 cache-write tokens, 5 output tokens, 5.612 seconds, and a conditional estimate of $0.017493. Calibration is diagnostic context, not a trial.

Wall times are elapsed times on the shared host and are descriptive. They combine agent work with the observed host environment; they are not isolated model-compute measurements.

Campaign provenance: campaign repository commit `e045527a611a048515349ab2f970043a5d33b185`; compiler source commit `e045527a611a048515349ab2f970043a5d33b185`; compiler binary SHA-256 `31891e0d229e217ebbdf71139f82654349e051eb06fb6b08e2984bea4ce6c8d1`. The independently qualified native binary SHA-256 was `dd44f27ad29bfcb60703740a6c7340d99fa43c32771e13ce0a3abe8dbd03e6f7`. The frozen ShiftSim specification SHA-256 was `5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00`; the acceptance corpus SHA-256 was `3c285999cfcf6a905e885d636ac55ba0ccb0f5999ef5b20ac3a8c17a2e023587`, with 15 cases. The final-source proxy uses `@anthropic-ai/tokenizer` 0.0.4 (`tiktoken` 1.0.22), package-bundled Claude BPE fingerprint `8e68c3fb830068e2405911a4a8bfce7e4574d7a911cf732f6c6666814b47c1ea`.

The source results file SHA-256 is `e2f7e18aacf41e858099b7c936263b1c8d25d7f68b5f0556b51f9ec034ad127a`; the authoritative recount SHA-256 is `bfb85b3fcb09077ddf6f205203cfa9b3ed83883027bac9a3f4f63251bcde6429`. The JSON recount binds each trial’s transcript, rollout, prompt, and candidate-manifest evidence digests. Historical interrupted reports are preserved unchanged.
