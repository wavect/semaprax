# ShiftSim fresh current-source398 campaign report

All ten planned attempts completed and were accepted: five SEMAPRAX and five TypeScript. Each candidate passed its build, own tests, and independent acceptance run. The pinned source398 native reference qualification passed all 15 frozen corpus cases. The campaign adapter retains `round: 3` metadata; this report identifies the distinct fresh current-source398 run by its new artifact namespace and pins. It is not a rerun of the historical e045 campaign.

The trial order was SEMAPRAX 01, TypeScript 01–02, SEMAPRAX 02–03, TypeScript 03–04, SEMAPRAX 04–05, and TypeScript 05. Each requested `gpt-6.1-sol` at `medium` effort. Each attempt used one outer Codex CLI turn; that is separate from the reconciled internal model-request turns below. The provider-resolved model identity is unavailable.

| Arm | Accepted / recorded | Model requests total / mean | Raw input total / mean | Cache-read total / mean | Uncached input (raw − cache-read) total / mean | Cache-write input | Output total / mean | Legacy net input proxy total / mean | Final source Claude-BPE proxy total / mean | Conditional cost / accepted task | Mean agent / acceptance seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| SEMAPRAX | 5 / 5 | 130 / 26.0 | 6,272,625 / 1,254,525.0 | 5,910,656 / 1,182,131.2 | 361,969 / 72,393.8 | 0 | 88,042 / 17,608.4 | 4,467,315 / 893,463.0 | 46,429 / 9,285.8 | $0.439085 | 557.011 / 33.941 |
| TypeScript | 5 / 5 | 35 / 7.0 | 655,818 / 131,163.6 | 528,896 / 105,779.2 | 126,922 / 25,384.4 | 0 | 37,941 / 7,588.2 | 170,088 / 34,017.6 | 24,382 / 4,876.4 | $0.137229 | 213.440 / 5.957 |

Uncached input is the derived subset `raw input − cache-read input`, not task-only input. Legacy net input is a separate historical proxy and subtracts first-request input once per request; it is not task-only input. Raw input, cache reads, cache writes, output, and this derived subset are shown separately. No fixed-context or stable-context amount is available, so no context baseline is subtracted.

| Attempt | Status | Model requests | Raw input | Cache-read input | Uncached input (derived) | Output | Legacy net input proxy | Final source proxy | Conditional API-equivalent estimate | Agent seconds | Acceptance seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| SEMAPRAX 01 | accepted | 28 | 1,337,369 | 1,279,232 | 58,137 | 16,006 | 948,533 | 8,642 | $0.404257 | 544.397 | 36.955 |
| TypeScript 01 | accepted | 9 | 175,977 | 153,728 | 22,249 | 7,798 | 51,075 | 4,847 | $0.137851 | 222.811 | 7.064 |
| TypeScript 02 | accepted | 8 | 154,968 | 133,120 | 21,848 | 7,766 | 43,944 | 4,868 | $0.134668 | 216.144 | 6.483 |
| SEMAPRAX 02 | accepted | 25 | 1,321,023 | 1,209,600 | 111,423 | 20,601 | 973,848 | 10,115 | $0.549816 | 603.467 | 26.691 |
| SEMAPRAX 03 | accepted | 29 | 1,448,115 | 1,375,488 | 72,627 | 17,424 | 1,045,392 | 8,870 | $0.457043 | 550.961 | 33.791 |
| TypeScript 03 | accepted | 5 | 85,118 | 54,656 | 30,462 | 7,396 | 15,728 | 4,713 | $0.140350 | 192.206 | 5.829 |
| TypeScript 04 | accepted | 6 | 106,277 | 79,744 | 26,533 | 6,921 | 23,009 | 4,543 | $0.130250 | 198.620 | 6.158 |
| SEMAPRAX 04 | accepted | 29 | 1,331,129 | 1,274,752 | 56,377 | 19,143 | 928,406 | 10,277 | $0.431659 | 618.574 | 48.339 |
| SEMAPRAX 05 | accepted | 19 | 834,989 | 771,584 | 63,405 | 14,868 | 571,136 | 8,525 | $0.352648 | 467.655 | 23.927 |
| TypeScript 05 | accepted | 7 | 133,478 | 107,648 | 25,830 | 8,060 | 36,332 | 5,411 | $0.143025 | 237.419 | 4.253 |

Cache-write input was zero in all ten attempts. The final-source metric is a legacy Claude BPE tokenizer proxy over each final candidate inventory; it is not verified authored-token count, cumulative edits, current-model tokenization, generated-output accounting, or billing. The proxy can include candidate documentation, tests, and scripts. It supports no authorship ratio or source-efficiency claim.

The conditional estimate uses the campaign’s frozen standard short-context price book dated 2026-10-08 and includes every recorded attempt; actual provider billing is unavailable (`null`). Since all ten attempts were accepted, the per-accepted-task divisor is five for each arm. This is a conditional equivalent, not a billed cost.

The separate empty-task calibration was one outer CLI turn and one model request: 13,227 raw input tokens, 0 cache-read tokens, 0 cache-write tokens, 5 output tokens, 4.799 seconds, and a conditional short-context equivalent of $0.026504. It is diagnostic only and was not subtracted from trial usage or cost.

Elapsed times were recorded on a shared host. All ten attempt receipts mark the resource assessment clean with no incidents; these wall times remain descriptive and are not isolated model-compute measurements.

No language-advantage, savings, or causal conclusion is asserted from this single run. The table reports observed outcomes for the pinned source398 configuration.

## Reproduction and provenance

Campaign adapter: `codex-matched-shiftsim-v2`; retained round metadata: `3`; report label: fresh current-source398 run. Repository/compiler source commit: `398b051e6e7a06ac49ecf77d9292831401430de0`. Pinned compiler binary SHA-256: `594980a96bb3d7f74a168dfa6bc71d6355344f2963489f1e3ae854d2f3a01237`. The independent source398 reference qualification binds that compiler to the 15-case corpus and qualified native binary SHA-256 `dd44f27ad29bfcb60703740a6c7340d99fa43c32771e13ce0a3abe8dbd03e6f7`.

Frozen inputs: SPEC SHA-256 `5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00`; acceptance corpus SHA-256 `3c285999cfcf6a905e885d636ac55ba0ccb0f5999ef5b20ac3a8c17a2e023587`; oracle SHA-256 `bdaeb7910f525271445493f3fe651f09c572ed28b132d01c608de5712d7fba01`. Each attempt’s prompt, transcript, rollout, and candidate manifest digests are bound in the recount JSON. Requested model/effort were `gpt-6.1-sol` / `medium`; Codex CLI was `0.160.1`.

The authoring-token proxy uses `@anthropic-ai/tokenizer` 0.0.4 / `tiktoken` 1.0.22, package-bundled Claude BPE fingerprint `8e68c3fb830068e2405910a4a8bfce7e4574d7a911cf732f6c6666814b47c1ea`. This tokenizer is not the model’s billing tokenizer.

Immutable run `results.json` SHA-256: `bb4a73c13f639c08f2beec17a82d63a4e416c41bd265c61fbd12737afbcdabb1`. `campaign.json` SHA-256: `ae3be5ff2dee6c4ee6452f103cc28359dcc9517dc81896a8ddfea95aef95155a`. Terminal receipt SHA-256: `a5cf3595023ab1a14b5dcc0f020e98f4b5dac5377d14d3560cb6c6866f98d247`. Source398 qualification evidence SHA-256: `6b41f57873ea94954c121539c99f28ab96663c1fa00ecd5316f5b8bb749f33d8`. Calibration source SHA-256: `d1730fb3b4463ba20afffe67013eb0d78a9e9e4a92d6186738e476436746827e`. The terminal receipt records launcher exit 0 and binds the complete result set. The trace-backed recount is generated by the offline `codex_report.py` and adds a supplemental `derived_measurements` block for raw-minus-cache input; it retains raw counters unchanged. Recount JSON SHA-256: `2937306bc7baa92a6850de7e6e073a9c79744bf33d0ccbbd594cf3b01e4252ac`. See the [trace-backed recount](./codex-current-source398b051e6-base398b051e6-20261009-recount.json).
