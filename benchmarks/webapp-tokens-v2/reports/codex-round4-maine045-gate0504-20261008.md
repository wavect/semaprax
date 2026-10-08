# TeamDesk Enterprise: original round 4 and separately qualified rescore

**Publication state:** Terminal rescore validated. Original paid measurements and original acceptance outcomes below remain separate from this fresh qualification.

## Original campaign measurements

All ten retained attempts are included: five SEMAPRAX and five TypeScript. The campaign artifact is marked `interrupted`; these ten attempts and the original recount are retained. Arm values are per-attempt means across all five attempts.

| Measure | SEMAPRAX | TypeScript |
|---|---:|---:|
| Model requests per attempt | 22.0 | 31.2 |
| Raw provider input tokens per attempt | 1,041,099.6 | 1,198,507.4 |
| Cached input tokens per attempt (included in raw input) | 968,780.8 | 1,137,408.0 |
| Cache-write input tokens per attempt | 0.0 | 0.0 |
| Legacy net-input proxy per attempt | 745,705.6 | 780,739.4 |
| Final retained-inventory token proxy per attempt | 20,334.0 | 72,462.4 |
| Conditional standard short-context API-equivalent estimate per attempt | $0.3820118 | $0.5643816 |
| Original conditional estimate per accepted task (all trial costs; calibration separate) | $0.3820118 | unavailable (0 accepted) |
| Agent wall time per attempt | 405.9414 s | 985.0962 s |
| Original acceptance wall time per attempt | 316.5908 s | 34.411 s |
| Original acceptance outcome | 5/5 accepted | 0/5 accepted |

Raw input already includes cached input, which is shown separately and must not be added again. The historical net-input proxy is the frozen harness formula: summed input minus the first request’s input multiplied by request count. It includes the task prompt and harness context and is not task-only input. The final retained-inventory proxy is a tokenizer count over final files, not cumulative edits, provider output, or billing tokens. Conditional cost uses recorded standard short-context price assumptions and is not a receipt. Actual billed amount, provider-resolved model, and stable hidden-context composition are unavailable. The requested model was `gpt-6.1-sol` at medium effort; the provider-resolved model is null.

The original TypeScript attempts evaluated 597 cases, with 315 missing cases, nine missing groups, and 8–44 failed cases per report. Those historical outcomes carry false-negative risk from incomplete coverage; they do not establish that every unreported behavior is incorrect. TypeScript attempt 05 was marked resource-contaminated after disk headroom fell below the floor. It remains in the ten-attempt denominator and its recorded costs remain unchanged. The campaign itself is marked interrupted despite all ten attempt records being retained. No clean comparison or accepted-task efficiency result is claimed.

## Calibration and fixed context

The separate empty-task calibration recorded one model request, 13,091 raw input tokens (including 8,832 cached tokens), a legacy-net proxy of 0, a conditional estimate of $0.009451, and 6.691 agent seconds. It is retained separately and is not subtracted from trials or included in the accepted-task estimate above. Stable system/tool-schema/task/history composition and actual provider billing remain null; the calibration is not an exact measurement of repeated fixed harness context.

## Final-file source-component recount

The separate recount covers all 30 final candidate inventories across TeamDesk round 4, LogLens 7, and ShiftSim 3. The TeamDesk-only rows below are per-attempt means for the original ten retained candidates; they partition the legacy final-inventory proxy by reviewed file class.

| Arm | Attempts | Legacy inventory proxy | Authored-source class | Generated-output class | Dependency-lock class | Unresolved class |
|---|---:|---:|---:|---:|---:|---:|
| SEMAPRAX | 5 | 20,334.0 | 9,669.2 | 10,664.8 | 0.0 | 0.0 |
| TypeScript | 5 | 72,462.4 | 22,410.8 | 47,348.2 | 2,703.4 | 0.0 |

The full all-dataset report and embedded per-file classification evidence are [authored-source-components-20261008.md](../../reports/authored-source-components-20261008.md) and its JSON companion. `Authored-source` means files classified as source-like program, test, configuration, or documentation in the final retained inventory; it does not verify authorship or cumulative effort. Generated-output entries require retained recipe and entrypoint hashes. The counts use the legacy Claude BPE proxy (`@anthropic-ai/tokenizer` 0.0.4 / `tiktoken` 1.0.22, bundled Claude BPE fingerprint `8e68c3fb830068e2405910a4a8bfce7e4574d7a911cf732f6c6666814b47c1ea`), not exact billed usage or a verified current model tokenizer. Authorship verification and ratio eligibility are false. These measurements do not establish a language advantage.

## Independent reference qualification (0504)

The fresh reference qualification is separate from the original paid attempt measurements. The receipt at `/Users/kevin/.codex/benchmark-runs/teamdesk-round4-rescore-0504ac737e52-deps-v1-20261008/gate-source/qualification-receipt.json` records both SEMAPRAX and TypeScript references passing all 912 cases, with no missing cases, missing groups, or failures. The gate source is `0504ac737e52ef9f9492f236e50bfe95d5658de5`; the frozen SPEC SHA-256 is `7658414ed2bbb53477a93e4a00f36269954c5bc45f3148fe72ddd02f203a50dc`. Reference report SHA-256 values are SEMAPRAX `ca1d2386d3c1a38f04c342a54f5d350b4cad63ca1208217a319c1caf4d6871fe` and TypeScript `958bfddea8782fb6ee5847597b71ca94ef0d8776c2b0f2b0ed9418074151fa03`. This qualifies the reference gates; it does not itself complete the candidate rescore.

## Frozen original provenance

- Original artifact directory: `/Users/kevin/.codex/benchmark-runs/teamdesk-codex-round4-maine045-20261008`
- Original campaign status: `interrupted`; ten retained attempts.
- Original `results.json` SHA-256: `64cee2f6057bbc78a4547a570f113c03c73ad41956f45612d8508694fb4309a7`
- Original recount SHA-256: `a996d7d4b38c28dfe5ebd8aa0444f3efe08037f9bfd93607c2a2072fc243b4ef`
- Campaign metadata SHA-256: `21163b3ad8b4605eb4a71230ed8ded09edb409298891943156b8403adb33267f`
- Candidate source repository commit: `5f9e50429e3adae51fae8a00148f2c081879c0de`
- Compiler source commit: `e045527a611a048515349ab2f970043a5d33b185`
- Supplied compiler binary SHA-256: `31891e0d229e217ebbdf71139f82654349e051eb06fb6b08e2984bea4ce6c8d1`
- Frozen harness manifest SHA-256: `3d91fd4d6827f80b56588d7986990ea984e96fbfb92fc894f06d0d9c72ccc96c`
- Separate reference qualification receipt SHA-256: `9916ba266514e1ad823a583b3902388bac99bc571f18fc229700ba8066b7b72b`
- All-dataset source-component recount SHA-256: `c4097fc45dfdbbd1b742686deed906d5863008323a94a108186696503443e529`


## Fresh candidate rescore

The terminal rescore retained all ten unchanged archived candidate identities under the qualified 0504 gate. Accepted attempts have exact passing 912-case coverage with no missing cases or groups. Rejected and unscorable attempts remain in the report with the validator-admitted evidence and status; partial coverage and failed checks are not converted into passes. SEMAPRAX: 5/5 accepted, 0/5 not accepted, 0/5 unscorable; TypeScript: 0/5 accepted, 5/5 not accepted, 0/5 unscorable. These fresh outcomes do not replace original outcomes or paid measurements.

The validated sidecar SHA-256 is 6a23ffb3bcf1aeb5027a16ec4ac6a0bb310017efa4874c83e016b281fd9ba6ac. The machine-readable JSON embeds original frozen measurements, terminal/campaign/results bindings, the 0504 reference receipt, dependency receipt, ten TeamDesk source-component records, and all ten rescore rows and report bindings.

The original TypeScript attempt 05 remains resource-contaminated. Dependency receipts retain historical_byte_identity_verified=false where historical bytes are unproven. Agent wall times may overlap on the shared host.

The gate0504 replay exposed two additional enum-control representation false negatives; their source repair is tracked in #691 and a separately pinned gateb6cf replay is underway. These old-gate observations are retained as prior-gate evidence, not a current-head or clean comparison.
