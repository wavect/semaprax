# ShiftSim Codex round 3: interrupted accounting

This campaign is incomplete and cannot support a comparative headline. Four of ten planned attempts finished: two SEMAPRAX and two TypeScript submissions passed the frozen 15-case independent acceptance corpus. SEMAPRAX attempt 02 incurred a disk-headroom incident; its acceptance, telemetry and estimated costs remain included. Six attempts were never launched. The process terminated with exit 2; this is not a clean five-per-arm campaign.

The matched requested model was `gpt-6.1-sol`, effort `medium`, timeout 1800 seconds. SEMAPRAX used qualified Project v27 and compiler source `402900a43f7e92f43c1b009da3b17a9ed70c84ea`. Requirements, TypeScript arm, prompts and acceptance corpus stayed frozen. The same disk policy applies to both arms: 5 GiB minimum free space. The contaminated attempt began with 7,616,745,472 bytes free, crossed the floor at 5,103,239,168 bytes and observed a minimum of 4,362,870,784 bytes.

All figures below describe only recorded attempts, including the contaminated attempt. They do not estimate the six unlaunched attempts.

| Arm | Accepted/recorded | Mean model requests | Mean legacy net input | Mean authored proxy | Conditional cost/accepted | Mean agent / acceptance seconds |
| --- | --- | --- | --- | --- | --- | --- |
| semaprax | 2/2 (5 planned) | 49 | 2,054,665 | 8,821 | $0.719639 | 919.785 / 24.038 |
| typescript | 2/2 (5 planned) | 6.5 | 29,253 | 4,807 | $0.127078 | 212.885 / 5.954 |

Raw input and cached input remain separate in the recount: SEMAPRAX totals 5,456,928 raw input tokens, 5,253,120 cached input tokens and 50,635 output tokens; TypeScript totals 237,152 raw input tokens, 193,152 cached input tokens and 14,684 output tokens. Both arms reported zero cache-write tokens. Model requests were 48 and 50 for SEMAPRAX, 6 and 7 for TypeScript.

Legacy net input is summed input minus first-request input times request count; it is a historical proxy, not exact task-only input. Authored tokens are the frozen Claude BPE final-source proxy, not current-model billing tokens or cumulative edits. Costs use the frozen conditional Standard short-context API-equivalent price book, include all recorded attempts, and divide by accepted tasks. Actual trial-attributed billing, provider-resolved model identity and per-request fixed system/tool/task/history composition remain unavailable; no calibration subtraction or language advantage is claimed. Calibration is preserved separately in the recount.

Immutable artifacts: `/Users/kevin/.codex/benchmark-runs/shiftsim-codex-round3-main5f8-20261008`. The recount binds source results and retained transcripts, inventories, acceptance and resource assessments. Source results SHA-256: `5d9f56e22a643ca50c51b28edc919c1f6bd30be2423e432948608516cfa46173`. Recount SHA-256: `689b6a7bd3dd7ea0c42f90b34f206ebd8cfb9272bfd53295a37f223935b4300c`.

Repair mining from accepted SEMAPRAX attempt 01 found repeated loan-analysis work-budget and cleanup initialization-history diagnostics, loop helper admission failures, and unauthenticated-slice transcript restrictions. These identify investigation targets, not proven compiler defects or causal savings. No candidate source, requirements, acceptance check or historical receipt was repaired after scoring.
