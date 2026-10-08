# LogLens round 7: fresh Codex repeat

This is a fresh five-attempt-per-arm repeat using the frozen round-6 LogLens
SPEC, sample, prompts, and qualification profile. It records a newer compiler
than the seed repository; it is not a same-compiler round-6 rerun or a causal
language comparison. The [saved recount](codex-round7-mainc50-20261008-recount.json)
was regenerated from the preserved run and provider traces. Its SHA-256 is
`b1b7204234146559d87c893d69b59a74fc3042c5f0dfc7da993e8b49a791069f`.

Both arms requested `gpt-6.1-sol` at medium effort, using Codex CLI 0.160.1.
The seed repository was `c50c3bd7623354840504687d64260edb8ff3895f`; compiler
source was `60002439e1b651ebbfbf3a887f12a20c927d720f`, and the executable
SHA-256 was `87e1d84bb618218159a3ebaec83dea285f2186486d7113962d08c71945474daa`.
All ten attempts were recorded, with zero resource-contaminated attempts.

| Metric (five attempts per arm) | SEMAPRAX | TypeScript |
|---|---:|---:|
| Accepted | 1/5 | 3/5 |
| Model requests | 35.6 | 9 |
| Raw input tokens | 2,008,295.6 | 237,564.4 |
| Cached input tokens | 1,931,827.2 | 195,200 |
| Legacy net-input proxy | 1,528,834.8 | 116,415.4 |
| Final authored-source proxy | 8,019.4 | 5,131 |
| Agent wall seconds | 653.8876 | 210.214 |
| Acceptance wall seconds | 29.0722896 | 5.1486842 |
| Conditional API-equivalent cost, all attempts | $2.671178 | $0.912694 |
| Conditional API-equivalent cost per accepted task | $2.671178 | $0.304231333 |

All six nonaccepted candidates passed the historical 33 checks and failed the
same two added boundary checks: text and JSON for `literal-plus-timezone`.
TypeScript attempt 05 has that same failure. The combined gate covers those
33 checks plus 16 boundary checks; it is not the full repository quality gate,
and it does not cover output continuation. These results do not establish a
language advantage or explain the outcome causally. SEMAPRAX attempt 04 used
`std.int.decimal`; accepted attempt 03 used its own decimal routine/helper.
Those source differences are descriptive only.

Token and time entries are means across all attempted tasks, including failed
candidates. Raw input includes cached subsets; cached tokens are not added again.
The legacy net-input figure is a historical proxy, not task-only input. The
authored-source count uses a legacy Claude tokenizer proxy, not exact GPT
billing tokenization. The cost figures are conditional Standard short-context
API-equivalent estimates; actual billing is unavailable. Fixed system/tool/task/
history context is unavailable and remains null; empty-task calibration is
separate and is not subtracted. The JSON retains failed-attempt costs and per-run
evidence hashes. Raw traces and candidate archives remain in the local campaign
artifacts and are not copied into Git.
