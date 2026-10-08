# LogLens Codex round5: historical-corpus results

Five attempts per arm used matched SPEC-only seeds, gpt-6.1-sol at medium effort,
and the frozen compiler source94fadd14cf27d045d22b23a79def640c95131c31.
All ten candidates passed the original33 independent checks and workspace guards.
This is historical-corpus acceptance, not a full-SPEC qualification claim.
The separate integer, UTF-8 and64KiB boundary audit is tracked by #663.

| Mean per attempted task | SEMAPRAX | TypeScript |
| --- | ---: | ---: |
| Model requests | 40.0 | 8.4 |
| Raw input tokens (includes cache subsets) | 2,272,689.4 | 216,162.0 |
| Legacy net input proxy | 1,735,009.4 | 103,308.0 |
| Final authored tokens proxy | 7,595.4 | 4,752.4 |
| Output tokens (includes reasoning) | 18,993.0 | 7,409.6 |
| Agent wall seconds | 516.277 | 156.060 |
| Independent acceptance wall seconds | 48.813 | 3.227 |
| Conditional API-equivalent USD per historical accepted task | 0.562094 | 0.146046 |

These results favor TypeScript on this task. They do not establish a SEMAPRAX
advantage or an upper-bound savings claim. Each CLI invocation contains one
outer turn; model requests count the actual internal inference requests.

The legacy net proxy subtracts first-request input once per request; it is not
isolated task input. Fixed system/tool/task/history composition is unavailable
and reported separately as null. The tool-free READY calibration is separate
and is never subtracted. Authored tokens use the legacy Claude BPE on final
source, including scripts/tests/docs; they are neither exact GPT tokens nor
cumulative authored edits. Raw input already contains cached input; cache
subsets are not added again. Output already includes reasoning.

Costs are conditional published Standard short-context API-equivalent estimates,
with all attempted costs divided by accepted tasks. Actual billed receipts,
provider-resolved model and service tier are unavailable. Model and effort are
observed in client turn_context. The adjacent independent recount JSON preserves
per-trial counters, evidence hashes, assumptions, calibration and provenance;
all per-request usage sums reconcile with final CLI usage. Raw traces and
candidate archives remain in the local immutable campaign directory:
`/Users/kevin/.codex/benchmark-runs/loglens-codex-round5-20261008`.

SEMAPRAX candidates include native and interpreter execution; the prompt allowed
compile or validate. No saved candidate was repaired after seeing hidden checks.
Language-integrity review and expanded boundary qualification remain separate
from these historical33-check results.
