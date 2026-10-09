# Web application token benchmark v2: TeamDesk Enterprise

The large companion of [v1](../webapp-tokens-v1/README.md). [SPEC.md](SPEC.md)
freezes a 20-entity application with the features larger business apps need:
sign-in with role- and row-level permissions, workflows, unique keys,
rollups, an audit history, CSV export, validation, computed fields, a REST
API, persistence, and a browser UI. It measures how many tokens an AI agent
spends building it in TypeScript/React and in SEMAPRAX with
[Web Application Projection v2](../../docs/WEBAPP-PROJECTION-V2.md).

## Independent full-SPEC qualification

The additive [acceptance contract](acceptance/CONTRACT.md) and external API/Chromium
gate are implemented under [OPT #659](https://github.com/wavect/semaprax/issues/659).
They keep this benchmark's SPEC frozen and derive obligations independently of
either application schema. Missing or unverified requirements prevent qualification.
The [reference qualification receipt](acceptance/evidence/reference-r8-summary.json)
records 912 passing obligations for each reference arm from unpaid qualification
session 66510 (exit 0), using compiler source
`398b051e6e7a06ac49ecf77d9292831401430de0`, binary SHA-256
`594980a96bb3d7f74a168dfa6bc71d6355344f2963489f1e3ae854d2f3a01237`,
and corrected gate `810afdd907561dcfe6aab52823d43b1ffecd0a0e`.
The receipt SHA-256 is `4cf4b4dc025eb16de40dd41079fc32b68a86be56fe025c764c7ca209f26dd766`.
Qualification overlapped an independent Cargo/test job; its 332.544-second wall
time is not an isolated measurement. The [readiness record](../opt-batch-verification-v1/source398-pre-campaign-readiness.json)
binds the fresh dependency-only TypeScript bootstrap to this qualification and
preserves the two interrupted qualification attempts. These receipts qualify
the references and tooling only; a paid campaign still requires fresh resource
admission with zero heavy jobs and establishes no performance result in advance.
The earlier r5, r6, and r7 receipts remain preserved as historical evidence.
New Codex plans require the reference receipt's compiler source and binary
SHA-256 to match `--compiler-source-ref` and `--semaprax-bin`. A matching gate
and SPEC alone do not admit a receipt from an earlier compiler: run fresh
reference qualification and derive the TypeScript bootstrap from that summary.
The historical self-tests, partial permission scenario, and live token aggregates
below are separate evidence. A clean matched Codex campaign with bound qualification
reports and retained events is still required before a new comparative headline.

## Arms

| Arm | Written by the agent | Directory |
| --- | --- | --- |
| TypeScript/React | Strict TS client and Node server sharing one schema-driven module for every rule, permission, workflow, key and rollup | [`typescript/`](typescript/) |
| SEMAPRAX | One `.spx` module: records, variants, and convention functions; `semaprax webapp` generates the app | [`semaprax/teamdesk.spx`](semaprax/teamdesk.spx) |

Both reference arms pass end-to-end checks. The TS arm passes `tsc`,
`vite build`, and a curl scenario. The generated SEMAPRAX app passes
`node out/server.mjs --self-test`
([evidence](changes/semaprax-green-cycle.txt)). The self-test covers every
entity, and its permission check covers all four roles against the schema's
own predicates.

For observed cross-account row behavior, run the separate
[`checks/permission-api-self-test.mjs`](checks/permission-api-self-test.mjs)
against a fresh, disposable SEMAPRAX or TypeScript API server. It creates two
distinct Agent accounts and owned Task, Expense, and Leave rows; it checks
each owner's reads and updates, the other Agent's denied writes, and the
Expense row-hiding rule. This external API check is distinct from the
generated server's `--self-test` and does not exercise a browser UI.
The existing green-cycle transcript predates this external check and is not
evidence that these cross-account requests have run; retain each arm's output
as separate permission-API evidence when the scenario is executed.

Example launch and check commands (replace each data path with a newly created
empty disposable directory, use separate ports, and discard the directories
afterward). Run each server command in Terminal A and its matching checker in
Terminal B; stop the server and discard its data directory after the check:

```sh
# Terminal A: SEMAPRAX, with a fresh empty data directory
node out/server.mjs --port 3101 --data /tmp/teamdesk-semaprax-605 --setup
node benchmarks/webapp-tokens-v2/checks/permission-api-self-test.mjs \
  --arm semaprax --base-url http://127.0.0.1:3101

# Terminal A: TypeScript, with a separate fresh empty data directory
PORT=3102 DATA_DIR=/tmp/teamdesk-typescript-605 npm run server --prefix \
  benchmarks/webapp-tokens-v2/typescript
node benchmarks/webapp-tokens-v2/checks/permission-api-self-test.mjs \
  --arm typescript --base-url http://127.0.0.1:3102
```

## Latest live campaign: Codex round 9

The paid round-9 campaign recorded five attempts per arm. Its original gate
accepted 5/5 SEMAPRAX candidates and 0/5 TypeScript candidates, but that gate
had known browser locator and audit false negatives, and TypeScript attempts
also encountered offline-bootstrap/tooling failures. An offline replay under
the corrected gate still accepted all five retained SEMAPRAX candidates and
none of the TypeScript candidates; two TypeScript replays ended in runner
failures and three had failed checks. Neither result supports an intrinsic
language or compiler advantage. The [full report](reports/codex-round9-base9602c3-gateb4b13c6cb34b-source0e99277e3-20261009.md)
preserves the original paid measurements and separates them from the corrected
replay; the [trace-backed recount](reports/codex-round9-base9602c3-gateb4b13c6cb34b-source0e99277e3-20261009-recount.json)
and [corrected-replay sidecar](reports/codex-round9-base9602c3-gateb4b13c6cb34b-source0e99277e3-20261009-corrected-replay.json)
bind the retained evidence.

| Measure | SEMAPRAX | TypeScript |
| --- | ---: | ---: |
| Accepted / recorded | 5 / 5 | 0 / 5 |
| Reconciled internal model requests | 103 | 141 |
| Conditional estimate, all five attempts | $1.767554 | $2.802326 |
| Conditional estimate per accepted task | $0.353511 | unavailable (0 accepted) |
| Actual provider billing | unavailable | unavailable |

These are conditional price-card estimates from reconciled request traces,
not charges. Costs include every attempt and are divided by accepted tasks;
the zero-acceptance denominator remains unavailable. Fixed harness context is
null and no context baseline is subtracted. Final-file token counts are an
unverified inventory proxy, not verified authored tokens or cumulative work;
the recount explicitly disallows an authored-source ratio. Model-request
counts are distinct from outer CLI turns and tool calls.

## Historical Claude Code campaigns (rounds 3–6)

The tables below preserve earlier Sonnet runs and are not current Codex
measurements. Their fixed-context subtraction was a historical estimate, not
a per-request context measurement; current harness-context composition is
unavailable. Their “net” rows are historical proxies, not task-only input.

| Round 4 SEMAPRAX (2 runs) vs TS (rounds 3 and 4), means | TypeScript/React | SEMAPRAX | TS ÷ SEMAPRAX |
| --- | ---: | ---: | ---: |
| Summed input, historical net proxy | 304,915 | 68,836 | 4.4× |
| Summed input, including historical context estimate | 674,750 | 281,718 | 2.4× |
| Historical task-context growth estimate | 46,165 | 18,339 | 2.5× |
| Final-source token proxy (historical label: authored) | 12,192 | 4,653 | 2.6× |
| Turns | 8, 11 | 5, 6 | |
| Wall time | 295 s | 68 s | **4.3×** |
| Failed commands | 0, 2 | 0, 0 | |

Both round-4 SEMAPRAX runs compiled on the first attempt and verified with
the self-test alone. One TS run found and fixed a real defect in its own
code: deleting any row with id 1 removed the Admin's password hash. Its last
fix was never re-type-checked. The generated server's equivalents are shared,
tested runtime code.

Round 3, before the round-4 fixes, came out the other way:
SEMAPRAX needed 16 turns and 959k summed input against TypeScript's 8 turns
and 501k. Its transcript showed where the turns went:

- Two failed compiles: a missing last comma in `variant X { A, B }`. The
  last comma before `}` in declarations is now optional, and `fmt` still
  writes it.
- A `--setup` server run in the foreground until a 120 s timeout, then two
  turns grepping the generated server for its routes. The `web` card now
  lists the API and says the server runs until killed.
- Six turns of hand-written curl scripts, although the self-test had
  passed. The self-test now prints one line of observed evidence per
  feature, and the card says it is the end-to-end verification.

The turn and summed-input counts are observations from that historical run;
the fixed-context subtraction does not identify task-only usage.

### Rounds 5 and 6: after the language features

Round 5 added record invariants, variant equality, case or-patterns, row-aware
default policies, `webapp --api`, exact parameter help, and the self-test
cleanup line. Round 6 added bare payload-free cases (`Role::Admin`) and the
documented first-run account flow. Round 6 has three SEMAPRAX runs; it is
compared with the three TypeScript runs of rounds 3 to 5.

| Round 6 vs TS | TypeScript/React | SEMAPRAX | TS ÷ SEMAPRAX |
| --- | ---: | ---: | ---: |
| Turns | 8, 11, 8 | 12, 6, 4 | |
| Historical net-input proxy, median | 241,079 | 79,453 | 3.0× |
| Historical net-input proxy, mean | 283,636 | 109,264 | 2.6× |
| Historical conditional cost estimate, mean | $0.513 | $0.284 | 1.8× |
| Historical estimate after context-write adjustment | $0.416 | $0.187 | 2.2× |
| Final-source token proxy (historical label: authored), mean | 12,192 | 3,756 | 3.2× |

The best SEMAPRAX run took four turns: read, write and compile and
self-test, check, report. Its historical net-input proxy was 39,305, 6.1 times
below the TS median proxy. The worst took 12, because it hand-wrote a curl
verification on top of the passing self-test. The self-test now prints one
evidence line per role, so the role rules are visible without that.

## Static inventory model

`node measure.mjs --tokenizer <dir>` produces
[results-static.json](results-static.json). The models are those of v1.
This deterministic source-inventory calculation is separate from live usage;
its final-file token counts are not cumulative authored tokens.

| Arm | Reference | Final-source tokens | Green | Batched | Attributable | Per-file |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| TypeScript/React | 0 | 12,142 | 93 | 29,695 | 12,235 | 108,265 |
| SEMAPRAX (now: invariants, `==`, or-cases, row-aware defaults) | 1,116 | 3,591 | 336 | 15,068 | 5,043 | 15,068 |
| TS ÷ SEMAPRAX | | 3.4× (was 2.7×) | | 2.0× (was 1.8×) | 2.4× (was 2.1×) | 7.2× (was 6.5×) |

The latest campaign's acceptance limitations, recovered TypeScript dependency
identity, unknown provider billing/model identity, and non-isolated replay time
are documented in its report. The current evidence supports the retained
attempt-level observations above, not a language-advantage headline.
