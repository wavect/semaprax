# Authored-source component recount (2026-10-08)

This publication partitions the legacy final-inventory token proxy for all 30 retained candidates across TeamDesk round 4, LogLens 7, and ShiftSim 3. It does not revise any original campaign report or result. The accompanying JSON embeds every attempt’s complete per-file classification and hashes, including retained generation recipe and entrypoint hashes where applicable.

| Dataset | Arm | Attempts | Legacy proxy mean | Authored-source mean | Generated-output mean | Dependency-lock mean | Unresolved mean |
|---|---:|---:|---:|---:|---:|---:|---:|
| TeamDesk-round4 | semaprax | 5 | 20,334.0 | 9,669.2 | 10,664.8 | 0.0 | 0.0 |
| TeamDesk-round4 | typescript | 5 | 72,462.4 | 22,410.8 | 47,348.2 | 2,703.4 | 0.0 |
| LogLens-7 | semaprax | 5 | 8,019.4 | 8,019.4 | 0.0 | 0.0 | 0.0 |
| LogLens-7 | typescript | 5 | 5,131.0 | 5,072.4 | 0.0 | 0.0 | 58.6 |
| ShiftSim-3 | semaprax | 5 | 8,481.6 | 8,481.6 | 0.0 | 0.0 | 0.0 |
| ShiftSim-3 | typescript | 5 | 4,851.6 | 4,851.6 | 0.0 | 0.0 | 0.0 |

All rows are per-attempt means; the JSON also carries sums, original status counts, classification completeness, and file-level evidence. These data are descriptive counts, not a quality, acceptance, effort, or language-advantage result. `authorship_verified=false` and `ratio_eligible=false` for every attempt.

The tokenizer is the legacy Claude BPE proxy used by the original inventory metric. It is not actual provider-billed usage and does not establish the tokenizer or hidden context used by a provider. “Authored source” means only files classified as source-like program, test, configuration, or documentation in the retained final inventory; it is not proof of authorship or cumulative human/agent effort. “Generated output” is counted separately only where exact retained recipe and entrypoint hashes are provided. Unresolved files remain visible as unresolved rather than being silently assigned.

TeamDesk round 4’s original acceptance evidence is contaminated for clean comparison (5/5 SEMAPRAX accepted, 0/5 TypeScript accepted); The repaired gate separately qualified both reference applications against all 912 requirements at `0504ac737e52`; replay of the original attempts is in progress. This recount preserves those original statuses only as provenance and makes no acceptance comparison. The original results, recount, and frozen harness manifests are SHA-256-bound in the JSON; no originals were rewritten.

The helper source report and all three original campaign bindings, along with every per-candidate recount document, are embedded or identified by SHA-256 in [`authored-source-components-20261008-recount.json`](authored-source-components-20261008-recount.json).
