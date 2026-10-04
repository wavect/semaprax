# LAW-16 current evidence report

The machine-readable [current report](evidence/law16-current-report-v1.json)
is generated from retained, offline-authenticated capsules. Its status is
**incomplete**. It does not claim issue closure.

## Matched Boolean-negation evidence

The exact two-value Boolean negation contract (`false → true`, `true → false`)
has retained candidate and assertion-retaining attack controls. The current
process-v2 capsule contains 30 fresh-process and 30 repeat-process samples for
each Bend verdict and SEMAPRAX Z3 candidate/attack route. Candidate p50/p95
times are Bend **106.15/166.51 ms** fresh and **96.38/160.04 ms** repeat;
SEMAPRAX Z3 **485.35/600.40 ms** fresh and **470.26/508.08 ms** repeat.
Attack p50/p95 times are Bend **84.43/100.81 ms** fresh and **88.10/102.98 ms**
repeat; SEMAPRAX Z3 **212.16/228.56 ms** fresh and **207.87/319.84 ms** repeat.
These are separate local process-provisioning observations, not a cross-route
ratio or winner. Fresh/repeat does not isolate operating-system, executable,
solver, or tool caches.

For each retained 30-sample timing cell, the report also calculates the median
absolute deviation (MAD) from its sample median. These values describe
within-cell spread; they are not confidence intervals or estimates of
cross-route uncertainty. The cells use different process captures and
assurance routes, so the values do not rank or compare the routes.

| Evidence cell | Fresh MAD (ms) | Repeat MAD (ms) |
| --- | ---: | ---: |
| Historical process v1, Bend candidate | 1.141 | 1.972 |
| Historical process v1, Bend attack | 0.508 | 0.435 |
| Historical process v1, SEMAPRAX candidate | 8.384 | 7.212 |
| Historical process v1, SEMAPRAX attack | 2.916 | 10.813 |
| Process v2, Bend candidate | 14.071 | 3.861 |
| Process v2, Bend attack | 6.288 | 5.172 |
| Process v2, SEMAPRAX candidate | 17.697 | 8.631 |
| Process v2, SEMAPRAX attack | 7.729 | 7.583 |
| Ordinary/check, Bend | 6.436 | 0.439 |
| Ordinary/check, SEMAPRAX | 2.445 | 3.332 |
| Proof/verdict, Bend | 2.346 | 3.339 |
| Proof/verdict, SEMAPRAX Z3 | 10.952 | 9.361 |

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

Matched Boolean-negation peak RSS v2 is retained separately in
[`evidence/law16-boolean-negation-peak-rss-v2/`](evidence/law16-boolean-negation-peak-rss-v2/):
30 wrapper-bound samples per Bend verdict and SEMAPRAX Z3 route. Bend p50/p95
was **118,300,672/122,978,304 bytes**; SEMAPRAX Z3 was
**50,970,624/51,167,232 bytes**. The report binds each route to its retained
input digest and keeps the observations separate; it makes no RSS ratio,
winner, or cold-cache claim. This current-checkout RSS observation does not
rebind the historical RSS capsule.

Supplemental [full-u32 encoding controls](evidence/full-u32-encoding-controls-v1/report.json)
passed 12 candidate/attack route checks and four out-of-domain boundary
controls, using a full-range representation profile. The controls preserve the
original manifest unchanged and are supplemental representation evidence only:
the original checked-u32 cells remain unadmitted, and the concrete witnesses
do not establish universal list or transfer proofs. They do not add builtin
SEMAPRAX `u32`, prove lowering, or close LAW-16. The checked-in machine report
continues to mark the overall result incomplete.

The added [full-u32 equal-spec profile](full_u32_equal_spec.py) records a
bounded SMT model check in
[`fixtures/full-u32-encoding-v1/sort-equal-spec.smt2`](fixtures/full-u32-encoding-v1/sort-equal-spec.smt2).
For every four-element U32 input and every U32 query, the counterexample query
for sorted output and exact multiplicity returns `unsat`; the separate
empty-output loophole check returns `sat` for a nonempty input. The matching
regression is
[`test_full_u32_equal_spec.py`](test_full_u32_equal_spec.py). This is a
model-level equal-spec result over lists of exactly four elements, not a
certificate that either source was translated into the model and not an
unbounded-list theorem.
