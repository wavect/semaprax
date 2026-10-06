# Web application token benchmark v1

How many model tokens does an AI agent spend to build one large web
application in SEMAPRAX, compared with TypeScript/React? This benchmark
measures that for **TeamDesk**, a 10-entity project management and help-desk
application frozen in [SPEC.md](SPEC.md). It measured the compiler at
`a6ccb4a18` (before) and with
[Web Application Projection v1](../../docs/WEBAPP-PROJECTION-V1.md) (after).

## Arms

| Arm | What the agent writes | Directory |
| --- | --- | --- |
| TypeScript/React | Vite + React + strict TypeScript client, Node server, shared typed schema (schema-driven generic views, the strongest compact TS design) | [`typescript/`](typescript/) |
| SEMAPRAX before | The same React client (SEMAPRAX had no UI target) plus a SEMAPRAX HTTP server. One entity's server slice was written and verified; it alone is 19.7 KB and still lacks 409s, a persistent accept loop and JSON escapes | [`semaprax-today/`](semaprax-today/) |
| SEMAPRAX webapp | One `.spx` module: records, payload-free variants, `requires` rules and computed-field functions; `semaprax webapp` generates the rest | [`semaprax/teamdesk.spx`](semaprax/teamdesk.spx) |

Each arm's TeamDesk passed the same smoke scenario: create, a rejected invalid
Task listing its rules, computed fields, a 409 on deleting a referenced Team,
and persistence across a restart. The generated SEMAPRAX app also passes its
own `node server.mjs --self-test` (create, read, list, update, validation,
404, 409, persistence, delete for every entity). Its computed fields agree
with the natively compiled SEMAPRAX functions on the same inputs (both give
`34953730` for the combined probe in the evidence below).

## Result 1: static models (deterministic)

`node measure.mjs --tokenizer <dir>` counts tokens with the offline Claude BPE
(`@anthropic-ai/tokenizer`). Without that dependency it falls back to
bytes/4 and says so. The output is in [results-static.json](results-static.json).
Every model starts from the spec and the language reference the arm needs,
and ends by reading one green verification cycle.

- **Batched (headline):** every file is written in one turn. This matches how
  the recorded live agents worked.
- **Language-attributable:** reference + authored + verification output,
  each once, leaving out the spec both arms read identically.
- **Per-file (upper bound):** one file per turn, every turn re-sending what
  came before. It charges multi-file stacks for each extra turn; real agents
  batch, so treat it as a ceiling, not the expected saving.

| Arm | Reference | Authored | Files | Green | Batched | Attributable | Per-file |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| TypeScript/React | 0 | 7,045 | 10 | 93 | 17,181 | 7,138 | 62,375 |
| SEMAPRAX before (measured slice only, lower bound) | 10,737 | 13,211 | 9 | 148 | 51,042 | 24,096 | 201,774 |
| SEMAPRAX before (slice extrapolated to 10 entities) | 10,737 | 48,075 | 9 | 148 | 120,770 | 58,960 | 410,958 |
| **SEMAPRAX webapp** | **984** | **1,667** | **1** | **64** | **8,364** | **2,715** | **8,364** |

Before this change, SEMAPRAX cost 3–8 times the TypeScript tokens in every
model. With the projection, TypeScript costs **2.1 times** SEMAPRAX batched,
**2.6 times** language-attributable, and 7.5 times per-file (upper bound).
SEMAPRAX authors 4.2 times fewer tokens. Its reference is the 984-token
`help language web` topic instead of the 10,737-token language card.

An earlier version of this page led with the per-file 7.6 times. That
figure is an upper bound, not the expected saving: the live runs below wrote
all their files in one or two turns.

Maintenance ([CHANGES.md](CHANGES.md): a new validated field, a new
enumeration value, a new referenced entity). Both arms are schema-driven, so
each edit stays in one file.

| Change | TS read | TS edit | SEMAPRAX read | SEMAPRAX edit |
| --- | ---: | ---: | ---: | ---: |
| 1 Customer website | 3,421 | 134 | 1,667 | 65 |
| 2 Critical priority | 2,157 | 117 | 1,667 | 84 |
| 3 Tag entity | 3,115 | 93 | 1,667 | 56 |

SEMAPRAX edits are 1.4–2.1 times smaller and reads 1.3–2.1 times smaller.

## Result 2: live agent runs

Matched prompts, the same model (Claude Sonnet as a Claude Code subagent), and
one run per arm per round. Per-turn usage comes from the transcripts; the
method and raw numbers are in [results-live.json](results-live.json). Every
agent starts with about 38.8k tokens of fixed harness context (system prompt
and tools), identical across arms.

| Round 2 (agent chooses verification) | TypeScript/React | SEMAPRAX webapp | Ratio |
| --- | ---: | ---: | ---: |
| Task context growth | 19,861 | 10,248 | 1.9× fewer |
| Summed input net of fixed context | 61,795 | 25,999 | 2.4× fewer |
| Summed input including fixed context | 256,130 | 180,627 | 1.4× fewer |
| Authored tokens | 6,287 | 1,664 | 3.8× fewer |
| Wall time | 115 s | 41 s | 2.8× faster |
| Failed compiles | 0 | 0 | |
| Spec complete | no (README missing, last fix unchecked) | yes, self-test passed | |

Round 1 used a prescribed curl script in both arms and gave the same shape
(task growth 18,993 vs 10,324). It also exposed a real defect: the agent
spelled `TimeEntry` functions `timeentry_*`, and the projection silently
ignored them. Run-together prefixes are now accepted, and an orphan function
is `SPX-WA105`, so the mistake can no longer pass silently.

## How to read this

- The static model isolates what the language and compiler control: the
  reference to read, the source to write, and the tool output to read back.
  It is deterministic and reproducible from the committed files.
- The live runs include everything else: the shared spec, the agent's own
  reasoning, and the fixed harness context, none of which depend on the
  language. They shrink the ratio, and still favour SEMAPRAX on every axis.
- One live run per arm and round is a small sample. The static numbers are
  the benchmark; the live runs are a sanity check that real agents realise
  the saving.
- The "before" server is extrapolated from a verified one-entity slice. The
  lower-bound row uses only the measured slice and already loses to
  TypeScript by 3.2 times.
- Threats to validity. A TS admin framework (react-admin and similar) was
  excluded by rule and could narrow the gap. The TS arm's generic, schema-driven
  design is already far smaller than the usual hand-written page per entity.
  Token counts use the public offline Claude tokenizer, not the production
  tokenizer of a specific model.

## Evidence

- `semaprax/teamdesk.spx` generates with `10 entities, 7 enums, 25 rules, 10
  computed`, and `node out/server.mjs --self-test` passes
  ([changes/semaprax-green-cycle.txt](changes/semaprax-green-cycle.txt)).
- Native agreement probe: appending an `@id("app.main")` main that combines
  `task_weight(Urgent, 7)`, `task_remaining(Doing, 4, 9)`,
  `project_duration(3, 40)`, `ticket_breached(Pending, 4, 5)`,
  `customer_large(5, Enterprise)`, `time_entry_amount(3, true, 2.5)` and the
  escalation length gives `34953730` under `semaprax run --native` and in the
  generated JavaScript. A string-returning function called from `main` hit a
  pre-existing native ownership invariant failure, so the escalation length
  was folded in as a constant on the native side.
- Patches for the change requests are in [changes/](changes/), and the
  reference texts each arm reads are in [reference/](reference/).
