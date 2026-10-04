# Harness evolution v1 (HN-15)

Status: experimental, gated; local macOS aarch64 evidence only. No real evolution backend has run: the real-implementation acceptance item is NOT met (see Audit).

Audience: harness maintainers and adapter authors.

Implements the host capability `skill.evolve/v1` in `crates/semaprax-harness/src/evolution/`, payload validators in `contract/payload/evolve.rs`, verb `evolve`. Diagnostics use letter `W`. The kind is host-invoked only: it is deliberately not part of `CapabilityKind::ALL`, so profile resolution and negotiation never bind it as a default provider.

## Audit of WikiSkill implementations (4 October 2026)

Method: arXiv:2608.27454 (Tang, Rashtchian, Ferng, Tomkins, Juan, Vu; submitted 27 Aug 2026). The abstract and HTML pages list no code link; no author-maintained implementation was found. `gh search repos wikiskill` returns about twenty community repositories, none with Google Research ownership. None is treated as official.

Primary candidate: `ashutoshsinghpr7/wikiskill`, community, MIT, v0.1.5, commit `cdc291c4296e30488a5922860dc596763704ede4` (pure Python 3.10+, zero dependencies, CLI `init/bench/status/evolve/gate/compare/transfer/run-task/maintain/propose/reset`). It implements Algorithm 1 with strict `R_val > R_best` gating, wiki never rolled back, and `git reset --hard` on rejection inside its own `skills/active` workspace tree. Backends (`wikiskill/backends`): `hermes`, `claude`, `codex`, `copilot`, each an agent CLI with its own login. None accepts a local Ollama/OpenAI-compatible endpoint, and a local 0.5B model cannot drive an agent tool loop.

Other candidates read from READMEs only: `bcmcpher/wikiskill` (OpenCode and Ollama, but the loop is a roadmap item, steps 3-9 unbuilt), `kenhuangus/wikiskill` (license NOASSERTION), `srlabs/skillforge` (AGPL-3.0), `cbuntingde/wikiskill` (self-described replication, no efficacy result).

Install (Step 2) was NOT performed: the only suitable candidate needs a remote or account-bound agent backend, which the lane brief forbids. Required to run it for real: a pinned `uv` venv of the candidate, one of hermes/claude/codex/copilot installed and logged in (a paid or remote model), and roughly (13 train + 9 val tasks) x iterations x turns agent calls per `evolve --iters N`. Ollama (127.0.0.1:11434, `qwen2.5:0.5b`) was present and untouched. The bridge `packages/semaprax-harness-adapters/wikiskill/adapter.py` therefore only reports `unavailable` with the missing prerequisite.

## Experiment

`semaprax harness evolve run <experiment.json> [--json]`; spec schema `semaprax.evolution-experiment.v1`: `id`, `family` (one recurrent family, for example `compiler-diagnostic/SPX-0001`), `adapter.command` (absolute) and `adapter.env`, `workspace_root`, `consent{traces,retention}`, `parent{name,dir}` (an official snapshot, read-only), `protected[]`, `tasks[{id,split,prompt,expected}]` (train, validation, test; validation and test required), `caps{max_iterations,max_model_calls,max_seconds}`, `gate{max_test_regression,max_skill_bytes}`, `min_traces`. `auto_promote` other than false is refused (`SPX-HPW009`).

Flow: protected digests (parent, `protected[]`, graders) -> ingest -> `evolve` call -> candidate written to `derived/` -> host-graded baseline and candidate runs on validation and test -> gate -> `evidence/result.json`. Result outcomes: `no-action`, `rejected`, `accepted`, `unavailable`, `aborted`.

Ingestion admits only `semaprax.evolution-trace.v1` records (allowlist projection: `task_id, family, kind, tool, outcome, diagnostic_code, message, attempt, repair`) and workflow reports `semaprax.harness-run.v1/v2` (one record per compiler diagnostic: code, message, status; paths, context and notes are dropped). Observation events are counted, not forwarded. Whole records are refused for reasoning or credential-like members, secret-like values, held-out task ids, or text containing a held-out prompt or expected answer (`refused` counts by reason). Only in-family records count; fewer than `min_traces` (default 2) yields `no-action` without calling the backend. `retention` is `experiment-only` (sanitized `raw/` copy deleted at the end) or `keep`; wiki, evidence digests and results always remain.

Adapter protocol (`ProcessAdapter`): cwd is the experiment workspace, environment cleared except `adapter.env`, one JSON request on stdin, one JSON result on stdout, exit 69 with `{"unavailable": reason}` for an absent backend. `evolve` (request: traces path and digest, parent name and digest, caps, train tasks with answers; result: wiki entries under `wiki/` with digests, `model_calls`, `iterations`, exactly one of `candidate{name,description,body}` or `no_action_reason`) and `solve` (request: task id and prompt, optional skill; no expected answer ever; result: answer, `model_calls`). The adapter cannot write graders: the held-out prompts and answers live in host memory and their digest is asserted unchanged.

Gate (host-owned): validation passed count strictly above baseline, test passed count not below baseline minus `max_test_regression`, skill body within `max_skill_bytes`. Reject rolls back only `derived/<name>`; the rejected body is kept as `wiki/negative-evidence/<digest>.md`. Caps: the iteration cap, model calls (the sum of adapter-reported calls, at least one per `solve`) and wall-clock seconds abort the run (`SPX-HPW006`); a `CANCEL` file in the workspace or the in-process flag aborts between and during adapter calls (`SPX-HPW007`); aborted runs leave no derived skill. Any change to protected content during the run aborts and rolls back (`SPX-HPW008`). The workspace may not overlap a protected path or the current directory (`SPX-HPW002`). Limitation: there is no OS sandbox; protection is overlap refusal plus before/after digests, so an adapter that writes elsewhere is detected only for the protected paths listed.

## Derived identity and promotion

An accepted bundle is `derived/derived-<name>/{SKILL.md,provenance.json}`; the name always carries `derived-`, so it cannot equal an official name. `SKILL.md` carries a visible line naming the parent and its artifact-v2 digest and the scope. `provenance.json` (`semaprax.derived-skill.v1`) records parent digest, source digests (trace digest, wiki entries), scope family and the evaluation record. Identity is the artifact inventory v2 digest of the whole bundle ([HARNESS-ARTIFACT-IDENTITY-V1](HARNESS-ARTIFACT-IDENTITY-V1.md)), which differs from the parent's. `evolve promote <workspace> --to <skills-root> --approve` is the only promotion: it needs an accepted result, re-verifies the derived and parent digests and the copy. Nothing is promoted automatically, and an accepted result is a proposal, not evidence of general improvement. A rejected or no-action result is a valid completed experiment.

## Tests

`cargo test --offline -p semaprax-harness --test harness_v1 skills::evolution::` (fixture adapter in Rust: all five outcomes, harmful candidate, caps, cancellation, tamper, overlap, ingestion, retention, promotion and loading through `SkillService`, CLI, payloads, and real-process adapters via `/bin/sh`). Provisioned: `HARNESS_PYTHON=<python3> cargo test --offline -p semaprax-harness --test real_tools_v1 hp_hn15 -- --ignored` runs the shipped bridge and asserts the truthful `unavailable`. These are not evidence of WikiSkill evolution.
