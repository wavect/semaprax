# LAW-16 current evidence report

The machine-readable [current report](evidence/law16-current-report-v1.json)
is generated from retained, offline-authenticated capsules. Its status is
**incomplete**. It does not claim issue closure.

## Matched Boolean-negation evidence

The exact two-value Boolean negation contract (`false → true`, `true → false`)
has retained candidate and assertion-retaining attack controls. Each route has
30 fresh-path and 30 repeat-path child processes. The report retains every
route's p50/p95 separately; those numbers are local process-provisioning
observations only. They do not support a cross-route ratio or winner.

The retained [ordinary/check capsule](evidence/law16-boolean-negation-nonproof-process-v1/) adds the same exact Boolean-negation candidate as separate nonproof routes: Bend ordinary checking measured **83.05 ms / 100.95 ms** fresh p50/p95 and **75.53 ms / 78.08 ms** repeat; SEMAPRAX `check` measured **233.90 ms / 238.91 ms** fresh and **235.00 ms / 243.15 ms** repeat. The capsule validates its raw streams, source hashes, Bend commit, and SEMAPRAX executable SHA from the copied repository path. Ordinary Bend is not `--verdict`, and SEMAPRAX `check` is not external-Z3 proof checking.

The separate [bounded proof/verdict capsule](evidence/law16-boolean-negation-proof-verdict-v1/) runs the exact candidate through Bend `--verdict` and SEMAPRAX installed-Z3, with a **120-second per-process cap**. Bend verdict measured **102.80 ms / 112.45 ms** fresh p50/p95 and **100.18 ms / 109.77 ms** repeat; SEMAPRAX Z3 measured **485.31 ms / 577.50 ms** fresh and **480.24 ms / 528.56 ms** repeat. All 120 successful child processes are retained and authenticated. A preceding path form using `/tmp` is retained as a 60-sample infrastructure nonresult with `SPX-J102`; the successful command canonicalized the project manifest under `/private/tmp`. This route remains local process-provisioning evidence, not OS-cache coldness, a timing ratio, or a winner claim.

Ten fixed-budget Luna matched pairs were completed: ten Bend candidate verdict
acceptances and ten identity-mutant rejections; ten SEMAPRAX selected
`app.negate ensures[0]` Z3 discharges and ten false-mutant rejections. All
Codex JSON records expose token counters but no monetary charge event.

## Trust boundaries

Bend is a local historical pin at `947db722640c86247849343657bf2f7ef01cb7f1`.
SEMAPRAX is a local historical executable pin at
`9a9db7a8117ac8d292b24ffd5671ec3333272290`, with installed-Z3 source proof.
They are labeled historical pins; #392 requires pinned identities and does not
require a current-head observation. Bend verdict and SEMAPRAX Z3 use distinct
trusted computing bases; source proof does not prove lowering or
execution. Fresh/repeat paths do not isolate operating-system or tool caches.

## Remaining blockers

The pinned SEMAPRAX parser does not admit checked `u32`; its retained
non-admission receipt records `SPX-P003`. There is no retained cold-cache
isolation, Lean export/kernel route, monetary cost event, or matched
project-sized/list/refactor/incremental cell. These blockers prevent
honest closure of #392.

The generated machine report also records retained tool identities, explicit
unavailable hardware/OS and optimization-flag provenance, Boolean agent-turn
proof-synthesis tokens, and historical bounded-balance annotation/changed-byte
rows. Historical bounded-balance rows are retained source evidence only and do
not satisfy the checked-`u32` cell.

The remaining required list-theorem, law-preserving-refactor, and law-breaking
edit fixtures have dedicated controls but are also blocked before a matched
route by checked-`u32` non-admission. Their machine-readable result is
[`evidence/law16-remaining-u32-cells-admission-v1.json`](evidence/law16-remaining-u32-cells-admission-v1.json).
The list source-proof route, refactor-equivalence route, and law-inventory
preservation route remain unobserved; none is inferred from the Boolean cell.

Matched Boolean-negation peak RSS is also retained separately in
[`evidence/law16-boolean-negation-peak-rss-v1/`](evidence/law16-boolean-negation-peak-rss-v1/):
30 wrapper-bound samples per Bend verdict and SEMAPRAX Z3 route. The offline
review preserves each route's p50/p95 RSS and source digest while excluding an
RSS ratio or winner claim. It is not cold-cache isolation.
