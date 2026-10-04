# Harness Workflow v1

Audience: toolchain contributors and harness users.

Status: HP-04 (#420) implementation contract for `crates/semaprax-harness/src/workflow/`.
Local executable evidence only; see `crates/semaprax-harness/tests/`. Diagnostics are
`SPX-HPD001..`. This document extends `docs/HARNESS-PROVIDER-V1.md`.

## Command

```text
semaprax harness run <project> [--task task.json] [--proposal proposal.json]
    [--apply-policy policy.json] [--disable] [--compiler <abs>] [--python <abs>]
    [--node <abs>] [--json]
```

The compiler is a service: an explicit executable (`--compiler` or `SEMAPRAX_COMPILER`),
run with a cleared environment. Plain `check`/`build`/`run` never reach this code. Providers
are enabled automatically when `[profile] enabled` is true and a resolved lock exists (written on
first run, then verified frozen). `--disable` is the single per-run switch (builtins only, zero
external calls); `[profile] enabled = false` is the persistent one. Exit 0 for
`approved-candidate-ready`, `published`, `no-repair-needed`; 1 otherwise.

## Pipeline (one lineage, one revision)

The lineage id is a digest of (snapshot revision, task, lock), so an identical restart finds its
journal. Every step re-verifies the snapshot; a changed tree is `SPX-HPD005`.

1. Authenticate the snapshot (manifest + every `.spx` digest, worktree identity).
2. Diagnose: `semaprax check --json`, then `semaprax test --json`. Seed = task seed or the failing function.
3. Context: native `semaprax context <project> <seed> --direction both --depth 1`. The external
   `context.repository` provider is called only when `external_context` is `always`, or `when-needed`
   and native context is missing/incomplete (`never` forbids it).
4. Final budget (`[budget] context_max_bytes`): native items are protected (overflow is `SPX-HPD020`);
   external items fill the remainder, the rest are dropped and counted.
5. Route with `decision::decide` (rules, zero router calls) and generate: `--proposal` file, else a selected
   `model.generate` provider (side-effecting class, never retried), else `SPX-HPD090`.
6. Validate: `semaprax project-candidate-preview` on a change the host builds (schema, base revision and the
   full nine requirements are host-owned). Protected facts are then checked on the compiler's output.
7. Checks: the compiler-produced candidate sources are overlaid on a private scratch copy; `check` and `test`
   run there and must report the previewed candidate revision. The verdict is the compiler's alone.
8. Present: `project-candidate-export` capsule saved in the cache; approval requirement reported.
9. Publish only with `--apply-policy`: `project-candidate-git-publish` under that policy.
   Otherwise stop at `approved-candidate-ready`.

## Task, proposal, apply policy

Task `semaprax.harness-task.v1`: `goal`, optional `seed`, `task_family`, `external_context`, `models`.
The goal is sent to the model route but never appears in reports or observations.

Proposal `semaprax.harness-proposal.v1`: `intent` (one of the compiler's candidate kinds), optional
`claims`, `summary`. Refused: raw source members (`SPX-HPD031`), unknown change kinds (`SPX-HPD031`, naming
the admitted kinds), `requirements`/`base_revision` (`SPX-HPD032`), publish/approve-like members (`SPX-HPD033`).
Claims (for example `tests_passed`) are listed as ignored and never used.

Apply policy `semaprax.harness-apply-policy.v1`: `auto_apply`, absolute `publication_policy` (a
`semaprax.candidate-git-host-policy.v1` file), optional `allowed_intents`. Both files must be regular files
outside the project (`SPX-HPD060`/`SPX-HPD082`).

## Composition

Stages: snapshot, diagnose, context, budget, route, generate, validate, check, present, publish. Each has exactly one
owner (`SPX-HPD011` duplicate), dependencies are a DAG (`SPX-HPD010` cycle, `SPX-HPD013` unknown), and interception
edges must not recurse (`SPX-HPD012`). Order is Kahn with stage order as tiebreak.

## Protected facts

Checked on the compiler's own preview: base revision equals the checked revision (`SPX-HPD041`); requirements equal the
fixed inventory (`SPX-HPD044`); no `requires`/`ensures`/`invariant` line of a changed file is deleted
(`SPX-HPD042`; rename/move compare counts); no new `uses { }` effect (`SPX-HPD043`); the base file digest matches
(`SPX-HPD005`). Failed candidate checks are `SPX-HPD050` (rejected), never success.

## Journal and resume

Append-only `<home>/cache/workflow/<project>/<lineage>.journal.jsonl`. Publication records `begin` then `done`/`refused`/
`uncertain`. A lineage with `done` reports the prior receipt; with `begin`/`uncertain` it reports
`uncertain: manual reconciliation required` (`SPX-HPD071`/`HPD062`) and never calls publication again (compiler
`SPX-G267` is the uncertain signal). A model generation that began without a result is not replayed (`SPX-HPD072`);
a completed one is reused from the cache.

## Diagnostics

001 no compiler, 002 compiler run failure, 003 malformed compiler output, 004 snapshot, 005 stale revision, 010-013
composition, 020 context budget, 021 context unavailable, 030-033 proposal, 040 compiler refused candidate,
041 revision binding, 042 deleted law, 043 widened effect, 044 weakened requirements, 050 candidate rejected,
060 policy location, 061 publication refused, 062 publication uncertain, 070 journal/cache io, 071-072 resume
uncertain, 080 usage, 081 task, 082 apply policy, 090 no proposal, 091 runtime path missing.

## Known limits

Decision providers (`decision.evaluate`) are reported but rules decide; the model bridge is a `model.generate`
adapter call, not the root `ProviderAdapter`; candidate checks use a scratch overlay because the compiler has no
CLI that tests a candidate in place.

## Integration (hpwire)

Flags and options added to `run`: `--observations <file>` (new JSONL file; one metadata-only observation per stage
plus a summary line); `--python`/`--node` are now optional (see Runtimes).

**Context.** When the resolved profile selects an external `context.repository` provider the context stage is
`workflow::BrokerContext`: a `context::Broker` over `SubprocessNative` (the compiler facts, protected by the byte
budget, provenance `compiler-verified`) plus `HostExternal` (structural items, provenance `external:<tier>`, droppable).
`external_context` is honored (`never` native only; `when-needed` asks the provider only when native facts are
incomplete; `always`). A provider failure keeps the compiler facts, marks them incomplete and adds a note. With no
external provider the plain native stage runs.

**Authorized checks.** `[workflow.check.<name>] argv = ["tools/test", "-q"]` (argv only, never a shell line;
committed configuration cannot hold an absolute path, so `argv[0]` is a bare name resolved on the host `PATH` or a
project-relative path). After the compiler's own check and test, each check runs once through
`command_view::execute` in the candidate scratch tree, with the project configuration. The authoritative exit status
decides: a non-zero or uncertain status is `SPX-HPD050` (rejected). The model-facing report (`checks.commands[]`)
carries the view, from the resolved `command.view` provider (for example RTK) or the raw view. `--disable` forces
the raw view. `SPX-HPD051`: a check was refused by the command layer or the stage cannot run checks.

**Decision.** An adopted `decision.evaluate` provider is wrapped by `decision::HostDecisionInvoker`
(`InvocationClass::Decision`; the router's latency ceiling becomes the request deadline and the host kills the
process group when it passes). It is consulted only when the project pins it
(`[capability."decision.evaluate"] provider = ...`, status `experimental`) or its `EnablementGate` passed
(no evidence source exists yet, so automatic selection stays on rules: `rules (learned provider not evaluated)`).
`route` in the report carries `status` and `source`.

**Skills.** Machine-approved roots (`adopt --skills <abs-dir> [--origin label]`, stored in `installations.json`,
never read from a project) feed the builtin `semaprax/plain-skills` provider (`SkillService`). `[skills]` maps to
`SkillCatalogConfig`; the rendered prompt for the tags of the task family enters the proposal request (`skills`
member) and its `model_visible_bytes` is the `skill_catalog` observation.

**Runtimes and environment.** `adopt <desc> --runtime <abs node|python>` records the runtime machine-locally.
Precedence: `--python/--node`, the adopted runtime, `HARNESS_PYTHON/NODE`. `Environment::from_process` forwards only
`HOME PATH TMPDIR`, the three host marker variables and the variables named by `credential_env` fields of the
endpoint catalog.

**Model policy.** `[model] local_only`, `strict_one_attempt`, `logical = "<id>"`: the logical model must be bound
machine-locally (`endpoints`); the policy is checked against the endpoint's ownership and every candidate model of
the route (`SPX-HPL010..012`). Endpoint adoption stays machine-local.

**Observations.** Stages observed: context (`context_select`), skills (`skill_catalog`), decision, generation,
command view. `report <obs.jsonl> --export token-observation [--output f] [--session id]` writes
`semaprax.token-observation.v1` rows for `scripts/token_report.py session`; byte-only sizes are
`tokenizer_unavailable`, never tokens.

Known gap: `harness context` (not the workflow) still reads the runtime from `HARNESS_PYTHON/NODE` only.

## HN-01: explicit task modes

Task `semaprax.harness-task.v2` (additive; v1 and no task are unchanged) adds `mode` (`repair` default, `change`,
`plan`/`inspect`), `acceptance` (strings are carried to the model; `{"stable_id","contains"}` items are host-verified
against the compiler-verified candidate source), `operation` (expected candidate kind), `checks` (names of authorized
checks to run), `budget`, `tokenizer_map` and `session`. `change` and `plan` require a stated nonempty `goal`.

Baseline health is a precondition, not completion. Green baseline tests no longer end a `change`/`plan` run: the goal,
`task_family`, selected `seed` and acceptance are carried through context, proposal, preview, checks. Failing baseline
tests or an unverified baseline without a `session` block are `diagnosed` (delegated to HN-02), never forced through a
verified-base API. Installed operations are discovered from the compiler (`CompilerService::supported_intents`: the
subprocess compiler probes each documented kind and treats `unsupported candidate intention kind` as absent) and
advertised in the report and the prompt; an unsupported goal (operation not installed, unknown kind, or the proposer's
`{"unsupported": reason}`) is `unsupported-goal` (`SPX-HPD092`) naming the installed operations, before any model call
when the task names the operation. `plan` stops after the compiler admits the proposal (or reports context only with no
proposal source): no candidate check, export, capsule or publication.

Report `semaprax.harness-run.v2` (only for v2 tasks; v1 reports are unchanged) adds `task` (mode, family, goal and
acceptance digests, never the goal text), `operations`, `session`. Statuses: `unchanged-repair-baseline` (explicit repair
on a healthy baseline, no model call), `planned`, `candidate-ready`, `unsupported-goal`, `rejected`, `published`, plus
`diagnosed`, `refused`, `uncertain`, `exhausted`, `no-progress`, `cancelled`. Exit 0 for `candidate-ready`, `planned`,
`unchanged-repair-baseline`, `published`, `no-repair-needed`, `approved-candidate-ready`.

## HN-11: one request budget

`workflow::budget` counts the exact serialized model-visible request (`canonical(prompt)`: context, skills, goal,
diagnostics, feedback, mode framing, intents) with the selected model's tokenizer, adds a protocol overhead and the output
reserve (`budget.protocol_overhead_tokens` default 256, `output_reserve_tokens` default 4096) and requires the total to fit
the model's `max_context`. The model-to-tokenizer mapping is explicit data (`DEFAULT_MODEL_TOKENIZERS`, longest prefix,
task `tokenizer_map` overrides naming a built-in (`cl100k_base`/`o200k_base`) or a host-approved tokenizer, see TC-11); nothing is guessed. Tokenizers are supplied to the run
(`--tokenizer-python`, `--tokenizer-script scripts/harness_tokenize.py`, `--tokenizer-cache`, repeated `--tokenizer
<name>`) and served by `ExternalTokenizer`; no helper means `unknown`: `request_tokens` is null, `measured` false, and
admission uses the UTF-8 byte length (an upper bound for byte-level tokenizers), reported as
`admission_basis: utf8-bytes-upper-bound`, never as tokens or cost. bytes/4 is not used anywhere.

Routing circularity: the routing estimate is the smallest protected-only requirement across the catalog; after the router
chooses, the request is fitted to that model before any provider call. Optional material is dropped whole (external
context items last first, then the whole skill section); native compiler facts, the goal, diagnostics, intents and the
output reserve are never truncated. A model that cannot fit is excluded and routing repeats; when none can, the run is
refused before generation (`SPX-HPD100`, explained). `context.request_budget` records the fit (`dropped_optional`,
`rerouted_from`, tokenizer identity). A per-task `TaskLedger` reserves input plus output reserve for every generation and
router call across attempts; `budget.max_task_tokens`/`max_task_cost_micros` refuse a call that would exceed them
(`SPX-HPD101`) before it starts. Each generation and router request is one incurred observation carrying the same named
(or byte-only) count the ledger holds, so the token-observation export reconciles without double counting; local counts
are never billed usage.

## HN-02: bounded session

`session` in a v2 task (`max_attempts` 4, `max_candidates` 8, `max_tool_calls` 400, `max_elapsed_ms` 600000,
`max_tokens`, `max_steps` 4) runs context -> propose -> preview -> check -> feedback -> revised proposal in a private
scratch copy, bounded across the whole task (`SPX-HPD111`, status `exhausted`; spent attempts stay counted). Exact
compiler/check diagnostics (code and text) of earlier attempts enter the next prompt as `feedback`; acceptance is
preserved. A repeated identical proposal, or the same diagnostic digest three times, stops with `SPX-HPD112`
(`no-progress`); `RunConfig.cancel` stops between steps with `SPX-HPD113` (`cancelled`). A step is admitted when the
compiler previews it and its checks (compiler check and tests, authorized checks) pass; admitted sources are applied to
the scratch tree only and the next step is previewed against the new revision. The session completes when the host-verified
acceptance holds (without any, after the first admitted step). `done`/`unsupported` proposals are supported.

Unverified baseline (check fails, `session` present, mode `repair`/`change`): the exact baseline bytes are captured, a
bounded `{"source_patch": {"edits": [{"path","find","replace"}]}}` (at most 16 edits; `find` must match once; baseline `.spx`
files only) is applied in scratch, and the compiler alone judges it. Operations start only after the scratch candidate is a
verified base. The acceptance oracle (the manifest and the modules listed under `tests`) cannot be patched
(`SPX-HPD114`) or targeted by a semantic proposal; compiler caller migrations (rename, signature, move, record field) may
touch it. Laws and effects cannot be removed or widened (`SPX-HPD042/043`), requirements stay host-fixed, and proposal
claims are ignored.

Journal: `session` records task, lock, toolchain, baseline, skill and bounds digests; each attempt records proposal and
diagnostic digests; generation `gen-<n>`/`repair-<n>` that began without a result is never replayed (`SPX-HPD072`);
completed generations are reused. Final application is a separate authority: `workflow::apply_result(snapshot,
result_dir, expected_revision, compiler)` re-verifies the result and rejects source drift (`SPX-HPD115`) before an
all-staged-then-rename write. A single admitted step against the project baseline has an ordinary capsule and publishes
only under the existing apply policy; multi-step and scratch-repair results are not one capsule and are applied only
through `apply_result` (no CLI verb yet; known gap).

Diagnostics added: 092 unsupported goal, 100 request cannot fit any model, 101 task budget exhausted, 111 session bound,
112 no progress, 113 cancelled, 114 oracle edit, 115 apply refused (drift), 116 acceptance/done unmet, 117 invalid
scratch patch.

### User path: `apply` and cancellation

`semaprax harness apply <project> --session <result-dir|report.json> --expected-revision <digest> [--compiler p] [--json]`
re-verifies the session result with the compiler, requires the project still to equal the session's captured baseline
(`semaprax.harness-session.json` in the result directory) and writes the changed files. Drift, a wrong expected revision or
an unverifiable result is `SPX-HPD115`; it never publishes (publication stays `run --apply-policy` or the compiler's own
route). `run --cancel-file <path>` cancels a session once the file exists (checked at start, polled every 50 ms, acted on
between steps: `SPX-HPD113`, status `cancelled`, journal `cancelled`); an in-flight side-effecting generation is recorded
`uncertain` and not replayed. SIGINT is not handled: it needs `unsafe`, which this crate forbids.

## HN-05, HN-12, HN-13, HN-16 wiring in `harness run`

- **Skills updates (HN-05).** A run is a new session: it performs the policy-gated, TTL-gated, bounded
  `updates::ops::maintenance` (a failure is a bounded `notes` entry and never blocks), loads
  `updates::effective_set(home)` (embedded + activated revisions) into `DefaultSkills`, and releases the
  previous run's revision locks that no longer match, so the NEXT run uses a newly activated revision while
  another session id keeps its locked revision from the immutable store. `--frozen` and `--offline` perform no
  update request at all (the fetcher is never built); `--updates-fixture-dir DIR` / `--updates-gh ABS_PATH`
  select the upstream (tests, air-gapped use).
- **Context (HN-13).** The broker stage (native slot) also serves as the plan stage: its native step runs with
  `external: never`, provider queries come from the task plan, and after a failed candidate in a session the
  pipeline makes exactly one focused follow-up (`pipeline::follow_up_context`) and, when that adds nothing,
  expands a continuation handle whose path the failure names (`expand_context`, no provider call). Provider
  answers are cached under `<harness home>/cache/context`; an identical second run is a cache hit, not a provider
  call. `context.providers` lists each stage once.
- **Check tokenizer (HN-12).** With `--tokenizer-python/--tokenizer-script/--tokenizer NAME`, the check stage gets
  its own helper instance (the first `--tokenizer`), so check-output measurements are named-token counts.
- **Routing (HN-16).** See [HARNESS-DECISION-V1](HARNESS-DECISION-V1.md#project-configuration-and-cli-wiring-hn-16).

## TC-09: local proposals before routing

`propose_step` first asks `workflow::acquire::local_proposal`. A supplied `--proposal` / scripted proposal
(`ProposalStage::local_source()`), or a completed same-lineage journal proposal, is taken before `route_and_fit`. It
makes zero router calls, zero model calls and no new inference reservation, and still goes through parsing,
installed-operation checks, protected facts, compiler preview, tests and approval. A journal artifact must exist, be at most
1 MiB and match its recorded digest, byte count and lineage/task/lock/revision identity. Otherwise the step is refused
with `SPX-HPD072` and is never replayed billably. A begun or uncertain step is refused before routing. A hit records
`context.proposal_acquisition` (`source`, `router_calls: 0`, `model_calls: 0`, `new_reservations: 0`,
`historical_incurred`) and, for journal hits, a `{step}.local-reuse` journal record. New spend stays separate from
historical spend. An empty scripted proposer still routes, so plan mode is unchanged.

## TC-11: host-approved tokenizers and the count memo

`workflow::tokenizers::TokenizerSet::add` is the host's approval act. It records the tokenizer's own fingerprint and
counting semantics; `add_pinned` refuses a fingerprint mismatch. `harness run` approves each `--tokenizer <name>` before it
parses the task, so a task's `tokenizer_map` may name a built-in or an approved tokenizer, and nothing else. A task can
never load code by naming one. A count needs a mapped, provisioned and approved tokenizer whose fingerprint matches the
approval. Anything else (including a tokenizer failure) is `tokens: null` with the UTF-8 byte bound, never zero. A bounded
per-run `CountCache` (FIFO, 1024 entries) is keyed by sha256 of the exact text, tokenizer name, fingerprint and semantics.
It stores integers only, never caches failures, and is shared by catalog candidates, `floor_estimate` and `fit`. A memo hit
is local work avoided, not provider tokens saved. A newly approved non-OpenAI tokenizer needs its own reference fixtures
before it is used to select smaller models.

## TC-05: initial context target and span deduplication

Opt-in through `[budget] context_target_bytes` (and `context_target_escalations`, default 2), which sets
`RunConfig.context_target`. When it is unset the pipeline is unchanged, and `[budget]` digests omit both keys.
`workflow::context_target` keeps three bounds separate: the model's hard capacity, the host context safety bound, and the
initial target. `gather_context` merges same-revision, same-path, same-provenance spans (`dedup_spans`) and keeps a
host-side provenance map; different revisions, external text and disagreeing overlaps stay distinct. `select` then
keeps compiler-verified and required-reference items unconditionally (or refuses with `SPX-HPD020` if they alone
exceed a bound) and ranks optional items deterministically: identifier hits, then novelty, then cost, then input
order. The follow-up path escalates the target only after a named missing dependency or validation failure. Escalation is
bounded by the escalation count, the hard capacity and the remaining task budget, and a refusal is reported as
`escalation_refused`. `r.context.target` reports chosen and omitted identities with reasons, the provenance map, the
current target and the escalation log, plus `exhaustive: false` whenever anything was omitted. None of this enters
the prompt. The cost unit is `tokens:<name>` when the first catalog model (`task.models`, else the configured model plans) has a mapped, provisioned tokenizer, and that model's `max_context` is the hard bound. Otherwise it is the labelled `bytes:byte-policy-upper-bound`. The `context_target_*` values are read in that unit. Required references come from the task seed, acceptance `stable_id`s, backtick-quoted identifiers in the goal and in diagnostics, and diagnostic paths, plus packet items labelled `required:`. Free prose is never mined for identifiers. The report lists them as `required_refs`. The handle path (`expand_context`) uses the same dedup and escalating merge as follow-ups. The final request fit still re-measures the routed model.
