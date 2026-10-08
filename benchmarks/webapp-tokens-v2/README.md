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
The [reference qualification receipt](acceptance/evidence/reference-r7-summary.json)
records 912 passing obligations for each reference arm. The SEMAPRAX reference
was qualified with compiler source `aae2719e29df37438e55bf52b00da3d7954bbd7f`
and binary SHA-256 `51bc391ca4c106374333ba0d272c702df263c384824933e29c12dd75b63e5a11`;
the receipt binds both reports to the frozen SPEC and acceptance gate. It does
not establish current-head or fresh agent performance. The earlier r5 and r6
receipts remain preserved as historical evidence.
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

## Result 1: live agent runs (headline)

Matched prompts, Claude Sonnet as a Claude Code subagent. Every agent starts
with about 38.8k tokens of fixed harness context; "net" subtracts it from
each turn. The raw numbers and method are in [results-live.json](results-live.json).

| Round 4 SEMAPRAX (2 runs) vs TS (rounds 3 and 4), means | TypeScript/React | SEMAPRAX | TS ÷ SEMAPRAX |
| --- | ---: | ---: | ---: |
| Summed input, net of fixed context | 304,915 | 68,836 | **4.4×** |
| Summed input, including fixed context | 674,750 | 281,718 | 2.4× |
| Task context growth | 46,165 | 18,339 | 2.5× |
| Authored tokens | 12,192 | 4,653 | 2.6× |
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

Each removed turn saves the whole context it would have re-sent, which is
why the fixes cut summed input by about 3.4×.

### Rounds 5 and 6: after the language features

Round 5 added record invariants, variant equality, case or-patterns, row-aware
default policies, `webapp --api`, exact parameter help, and the self-test
cleanup line. Round 6 added bare payload-free cases (`Role::Admin`) and the
documented first-run account flow. Round 6 has three SEMAPRAX runs; it is
compared with the three TypeScript runs of rounds 3 to 5.

| Round 6 vs TS | TypeScript/React | SEMAPRAX | TS ÷ SEMAPRAX |
| --- | ---: | ---: | ---: |
| Turns | 8, 11, 8 | 12, 6, 4 | |
| Summed input net of fixed context, median | 241,079 | 79,453 | **3.0×** |
| Summed input net of fixed context, mean | 283,636 | 109,264 | 2.6× |
| Estimated cost, mean (lower bound) | $0.513 | $0.284 | 1.8× |
| Estimated cost, mean, net of the fixed context write | $0.416 | $0.187 | 2.2× |
| Authored tokens, mean | 12,192 | 3,756 | 3.2× |

The best SEMAPRAX run took four turns: read, write and compile and
self-test, check, report. It needed 39,305 net input tokens, 6.1 times fewer
than the TS median. The worst took 12, because it hand-wrote a curl
verification on top of the passing self-test. The self-test now prints one
evidence line per role, so the role rules are visible without that.

**Dollars.** The cost estimate (`cost.mjs`) is exact for input from the
transcripts' usage records and a lower bound for output, because thinking is
redacted. Prompt caching makes re-sent context cheap, so the dollar ratio
(1.8–2.2×) is lower than the net-token ratio. Each run also pays the same
fixed 38.7k-token cache write for the harness context. What SEMAPRAX saves in
dollars comes mainly from writing about 3× fewer output tokens and from
needing fewer cache writes.

## Result 2: static models (deterministic)

`node measure.mjs --tokenizer <dir>` produces
[results-static.json](results-static.json). The models are those of v1.

| Arm | Reference | Authored | Green | Batched | Attributable | Per-file |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| TypeScript/React | 0 | 12,142 | 93 | 29,695 | 12,235 | 108,265 |
| SEMAPRAX (now: invariants, `==`, or-cases, row-aware defaults) | 1,116 | 3,591 | 336 | 15,068 | 5,043 | 15,068 |
| TS ÷ SEMAPRAX | | 3.4× (was 2.7×) | | 2.0× (was 1.8×) | 2.4× (was 2.1×) | 7.2× (was 6.5×) |

## Why not 15×

Every arm must state the application's information: 20 entities, about 110
fields, 47 rules, 24 computed fields and rollups, 7 keys, 5 workflows, and a
permission matrix. The spec says it in 2,659 tokens. The SEMAPRAX source
says it in 4,486, 1.7 times the spec. The schema-driven TS arm needs 12,142,
4.6 times the spec. No honest source can shrink much below the spec it
encodes, so the authored-token ratio is bounded near 4–5× for this
application. Both arms also read the same spec.

The live ratio can exceed that because every turn re-sends the context: a
language that needs fewer, smaller, first-time-correct turns saves
multiplicatively. That is where SEMAPRAX's 4.4× net saving comes from. A 15×
gain would require either dropping requirements, or a TypeScript arm
deliberately worse than the schema-driven design used here. Neither would be
a meaningful result.

## Threats to validity

- **Sample size:** two runs per arm in the final round.
- **Scope of verification:** the browser UIs were exercised only by their
  own code (TS: `tsc` and `vite build`; SEMAPRAX: embedded runtime code),
  not by browser automation.
- **Excluded frameworks:** TS admin or auth frameworks were excluded by rule.
- **Tokenizer:** counts come from the public offline Claude tokenizer.
- **Fixed harness context:** the about 38.8k tokens of harness context are
  real cost in any agent session. The "including fixed context" row shows
  it; the net row isolates what the language controls.
