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
