# LAW-16 current evidence report

The machine-readable [current report](evidence/law16-current-report-v1.json)
is generated from retained, offline-authenticated capsules. Its status is
**incomplete**. It does not claim issue closure.
The documented `law16_replay.py --verify-retained` sequence now authenticates
the available capsules, including all 230 recovered bounded-balance raw outputs.
The [unified fresh capture](evidence/law16-unified-fresh-v1/replay-status.json)
completed six non-agent routes in that command, with 882 retained artifacts.
The separate [caller-pinned guarded-i64 balance SMT source-proof capsule](evidence/law16-unified-fresh-guarded-i64-v1/result.json)
physically exercised the seventh non-agent route and retained its 16 raw
streams. Offline review of both capsules succeeds. The retained agent campaigns
were re-authenticated, not rerun. All seven non-agent routes have now been
physically exercised, but AC1 remains partial because the optional live agent
continuation was not rerun.

The [preceding failed capture](evidence/law16-unified-fresh-pin-failure-v1/replay-status.json)
retains all output from the first three routes and the following compiler-pin
format refusal. The runner now preserves the required `sha256:` prefix. RSS
review also authenticates original sample commands after the capsule is copied.
The fresh capture uses the historical compiler pin below and a newly built
Lean test harness associated locally with
`fdc908ee98ea706dd0e905f36260557a493b908b`. Its source/binary association is
not a reproducible-build attestation. Host quietness was not established, and
these fresh timings introduce no new performance comparison or winner.

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
Codex JSON records expose token counters but no monetary charge event. The
[cost-provenance receipt](evidence/law16-boolean-negation-agent-cost-provenance-v1.json)
authenticates all 20 provider event streams and their 333,999 total tokens;
its monetary cost is explicitly unavailable, with no price inferred from tokens.
A separate [Claude Haiku cost probe](evidence/law16-claude-cost-pilot-v1.json)
recorded a provider-reported **$0.023741** charge, but stopped with
`error_max_budget_usd` before producing a source outcome. Its sanitized receipt
does not retain the provider stream, and this failed probe does not supply
monetary cost for the ten matched Codex pairs or admit a Claude campaign.
Two later [bounded Claude Bend attempts](evidence/law16-claude-boolean-pilot-v1/capsule.json)
recorded provider costs of **$0.008302** and **$0.016636**. One generated
unsupported Bend syntax; the other returned prose instead of the declared
JSON source shape. Neither is a successful matched trial, and no ten-pair
Claude campaign was run. Provider streams and prompts are not retained in
that sanitized capsule.
The [Boolean annotation receipt](evidence/law16-boolean-negation-annotation-summary-v1.json)
binds all 20 final sources in those ten pairs to their fixed seeds and reports
explicit annotation, proof-term, and changed-byte counts. These textual counts
do not measure reasoning effort or make raw bytes comparable across languages.

A separate [matched Boolean refactor cell](evidence/law16-boolean-refactor-cell-v1/result.json)
retains 16 raw streams for real Bend ordinary/verdict and SEMAPRAX check/Z3
candidate and law-breaking attack routes. Both refactored candidates pass;
Bend ordinary/verdict and SEMAPRAX Z3 reject their attacks. SEMAPRAX `check`
accepts its attack as a nonproof observation. This scalar cell does not admit
the original checked-`u32` refactor or a project-sized incremental edit.

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
non-admission receipt records `SPX-P003`. The later
[Linux/Rosetta guest capsule](evidence/law16-guest-cache-thirty-v1/receipt.json)
authenticates 30 cold/warm guest file-page-cache pairs per ordinary Bend and
SEMAPRAX `check` route. Host and Rosetta caches remain unknown, while
Bend verdict and SEMAPRAX Z3 cold/warm routes remain unmeasured. There is no
monetary cost event for admitted matched agent trials or matched project-sized
incremental cell. The Lean list proof below covers its exact LAW15 source,
not the original LAW16 fixture. These gaps prevent honest closure of #392.
The separate [three-module calculator capsule](evidence/law16-project-incremental-cell-v1/result.json)
executes SEMAPRAX compiler cache tests for a provider body edit and a rejected
provider-signature edit. It is local incremental behavior, with no matched Bend
route or large-project timing result.
The [cache-isolation probe](evidence/law16-cache-isolation-probe-v1/receipt.json)
records an earlier ephemeral Apple Container guest with read-only `/proc/sys`
even as root. It collected zero checker timings and left no container running.
The later guest capsule used a different guest setting and measured its file
page cache directly; the earlier failed probe remains a separate nonresult.

The generated machine report also records retained tool identities, explicit
unavailable hardware/OS and optimization-flag provenance, Boolean agent-turn
proof-synthesis tokens, Boolean annotation/changed-byte counts, and historical
bounded-balance annotation/changed-byte rows. The bounded-balance rows remain
retained source evidence only and do not satisfy the checked-`u32` cell.

The remaining required list-theorem, law-preserving-refactor, and law-breaking
edit fixtures have dedicated controls but are also blocked before a matched
route by checked-`u32` non-admission. Their machine-readable result is
[`evidence/law16-remaining-u32-cells-admission-v1.json`](evidence/law16-remaining-u32-cells-admission-v1.json).
The original LAW16 list source-proof route, refactor-equivalence route, and
law-inventory preservation route remain unobserved; none is inferred from the
Boolean cell.

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

The retained [guarded-i64 balance source-proof capsule](evidence/law16-guarded-i64-balance-smt-v1/result.json)
authenticates seven installed-Z3 discharges for selected scalar debit and
credit postconditions: U32 range, exact guarded update, conservation, and
positive transfer. It checks source obligations for the guarded-i64 projection
on a locally pinned SEMAPRAX executable and Z3 4.12.5; it does not prove
lowering or execute the app. The no-op debit mutant was refused with
`SPX-LW140`. That refusal does not classify a solver counterexample, `unknown`,
or another refusal reason. The original structured full-u32 balance fixture
remains unsupported by this profile. These source proofs are distinct from
the native runtime controls and Bend checks above, and they do not close
LAW-16.

The additive [guarded-i64/U32 v2 profile](fixtures/full-u32-guarded-i64-profile-v2.json)
binds a fresh 12-route candidate/attack replay, four out-of-domain refusals,
and its separate bitvector representation checks. It keeps the original
checked-`u32` admission unchanged. Its [Bend universal sort capsule](evidence/bend-u32-sort-universal-v1/capsule.json)
retains a pinned `--verdict` and direct BendTT kernel pass for sortedness and
exact per-value multiplicity over every finite U32 list. The empty-sort
universal count law is refused, and a separate `[1]` count mismatch is
kernel-checked. The [SEMAPRAX Lean capsule](evidence/law16-i64-list-proof-v1/capsule.json)
retains a pinned Lean 4.34.0 check of source-authenticated LAW15 insertion
sort, proving sortedness, permutation, and multiplicity over all finite
`List<i64>` values; U32 values are a subset. These routes align in semantic
law strength over U32 values. They have distinct source algorithms, declaration
identities, and trusted computing bases. The Lean certificate does not cover
`law16.insert` or `law16.sort`, and neither route has matched timing or a
translation/lowering proof. The overall report remains incomplete.
