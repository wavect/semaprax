# Command view v1 (HP-08, HP-09 host side)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

Audience: toolchain contributors and harness adapter authors.

`semaprax-harness exec <project> [--raw] [--json] [--timeout-ms N] [--external-owner NAME]
[--observations FILE] [--env K=V]... -- <argv...>` and
`recover <project> <handle> [--offset N] [--limit N] [--stream stdout|stderr] [--json]`.
Implementation: `crates/semaprax-harness/src/command_view/`. Diagnostics `SPX-HPH` (command
view) and `SPX-HPI` (automatic provider use).

## Authoritative result versus view

The host executes the command **exactly once** (`executor.rs`: cleared environment plus the
granted names, explicit absolute executable and cwd, own process group, deadline and
cancellation kill the group). `CommandResult` carries original argv and its digest, selected
executable, cwd, environment grant names, exit/signal/timeout/cancelled status
(`status_certain` is false for timeout and cancellation), stdout and stderr byte counts and
SHA-256 digests over every byte, and the recovery handle. A `ModelView` carries
`{text, lossless, omissions, provenance, recovery_handle, incomplete, route, notes}`. A provider
can shape view text only; its payload may not carry a status (`SPX-HPA042`).

## Routes

- `provider`: post-execution `command.view` `view` over redacted, already-captured bytes. Used
  only when retention holds the complete raw output, output is within the provider frame, the
  status is certain, output is not machine JSON, and is at least `min_bytes`.
- `raw`: bounded head/tail display (`[budget] command_view_max_bytes`) with critical lines
  (`error|fail|panic`) re-attached; truncation sets `incomplete`.
- `wrapper`: opt-in (`allow_wrapper`) pre-execution `wrap` plan. Both the original and the
  effective argv are authorized before launch. Accepted only if it declares raw recovery (result
  diagnostic `wrapper.raw-recovery`), names the original executable with identical arguments or the
  provider's own granted upstream followed by the full original argv, and keeps cwd. Refusals:
  `HPH012` shell syntax, `HPH014` substituted executable, `HPH016` widened arguments, `HPH017` no
  declared recovery, `HPH018` cwd change. A wrapped view is never marked complete.

If the provider fails, the raw view is used; the command is never re-run. If a provider view
drops a critical raw line, the host appends it, marks the view `incomplete` and adds the recovery
reference. Undecodable bytes count as omissions.

## Contract operations (`command.view/v1`)

Operations `view`, `wrap`, `plan`; every payload is closed and bounded (`SPX-HPA040`/`041`) and no
result may carry an exit status or signal (`SPX-HPA042`).

- `view` request: `form:"post-execution"`, `argv`, each of stdout and stderr by exactly one of
  `stdout|stdout_b64|stdout_path` (and `stderr...`), optional `min_bytes`, `max_bytes`,
  `recovery_handle`, `config` (flat scalar object, at most 32 members). A `*_path` is relative to the
  provider's own retention directory (no root, `..`, backslash). Result: `{form, view:{text, lossless,
  omissions, recovery_handle?}}`.
- `plan` request: `argv`, `cwd_rel` (`.` or relative), optional `estimated_output_bytes`,
  `external_hooks`, `lineage`, `form` (`post-execution|wrapper`), `config`. Result: `route`
  `post-execution | wrapped | bypass`; `bypass` carries `reason`; `wrapped` carries validated string
  `argv`; optional `form`, `family`, `filter`, `operation`, `raw_recovery`, `env`, `resolves_via` and a
  bounded `recovery` object (string and string-array members). A wrapped route counts as having raw
  recovery only when `recovery.coverage` is `complete`.
- `wrap` (older, kept): `{form:"wrapper", argv}` to `{form, plan:{argv, cwd?}}` plus the
  `wrapper.raw-recovery` diagnostic.

The host uses `plan` when the provider declares it: `post-execution` runs once and calls `view`;
`bypass` runs once and shows the raw view without consulting `view`; `wrapped` is taken only with
`allow_wrapper`, `recovery.coverage:"complete"` and no extra `env`, and is then authorized like a `wrap`
plan; otherwise the command runs unwrapped, once, and `view` is used. Without `plan`, `view` is used
directly. Output up to half the provider frame is sent inline; larger output (up to 8 MiB) is staged as
0600 files under `<provider retention>/views/` and passed by `*_path`, then removed.

## Exclusions and bypass

`--raw`, `semaprax*` executables, signing tools, machine-output flags (`--json`, `--format`,
`--message-format`, `--porcelain`, ...), JSON stdout, small output, outer ownership. Ownership: a
`SEMAPRAX_HARNESS_COMMAND_VIEW_LINEAGE` marker (set in the command's environment), an external
owner (`--external-owner` or `SEMAPRAX_HARNESS_EXTERNAL_VIEW_OWNER`), a nested `semaprax-harness
exec`, or a `known_wrappers` executable.

## Refused before launch

Free-form shell (`sh -c`, `eval`, via `env`), pipelines, redirections, `;`, `&&`, `||`, backticks,
`$(` as argv elements, remote execution tools, unresolvable executables. Bare names resolve only
through the `PATH` in the supplied `Environment`.

## Policy and retention

`$SEMAPRAX_HARNESS_HOME/command-view.json`, schema `semaprax.harness-command-view-policy.v1`
(closed): `min_bytes`, `min_tokens`, `timeout_ms`, `mem_cap_bytes`, `provider_timeout_ms`, `retention
{enabled, ttl_secs, max_bytes, max_stream_bytes}`, `redact_lines_containing`, `known_wrappers`,
`runtimes {python, node}`, `env_grant`, `allow_wrapper`. Retention is off unless configured; files
live in a 0700 directory (0600 files) under `retention/<project>/command-view/`, pruned by TTL and
size. Redaction applies before a provider sees data; retained raw bytes are unredacted and private.

## HP-09 (provider selected by profile)

No product is named in host code: the profile's resolved `command.view` provider is used
automatically after adopt+trust, with the min-bytes policy and `--raw` escape hatch. A missing or
incompatible provider falls back to raw unless the capability mode is `required` (refused before
launch, `SPX-HPB04x`/`SPX-HPI001`). The host never installs, upgrades or edits configuration.
Observations record byte counts at the final display envelope (`byte_only`); vendor gain figures
are not tokens.

## HN-12: delivered views at development boundaries

Authorized check output (`workflow::checks::HostCommandChecks`) flows through the same `execute` and the same
post-execution provider (RTK `pipe`, reused, not reimplemented). When a session check fails, the model-facing view
enters the next request as `feedback[].check_output` (`check`, `status`, `route`, `incomplete`, `recovery_handle`,
`recovery_project_id`, `output` bounded to 8192 bytes, `delivered`); the verdict stays the authoritative exit status,
and the feedback digest includes the output so a different failure is progress and an identical one is not. The request
budget (`docs/HARNESS-WORKFLOW-V1.md`, HN-11) counts the serialized request including this text.

- **Measurement** (`ModelView.measurement`, `command_view::measure`): `ExecOptions.tokenizer` / `HostCommandChecks.tokenizer`
  lend a named tokenizer. The count covers what the model is shown (view text plus its recovery reference) against the
  host's own raw view. Fields: `decision` (`provider-smaller`, `provider-grew-raw-used`,
  `below-token-threshold-raw-used`, `provider-failed-raw-used`, `provider-not-consulted`, `raw-by-policy`), `basis`
  (`tokens` or `bytes-only`), raw/delivered bytes and tokens, `saved_tokens` (null unless both counts exist; may be
  zero or negative), `rejected_view_tokens` and `overhead_ms` (time spent consulting the provider). A provider view
  that is not strictly smaller (tokens when measured, bytes otherwise) is replaced by raw; no tokenizer means no saving
  is ever claimed. Policy `min_tokens` skips the provider for raw output below that measured size, only with a tokenizer.
  With a tokenizer the observation records named before/after counts; otherwise it stays byte-only.
- **Delivered versus report compaction.** `Report.session.delivered_to_model` sums tokens the model was actually shown;
  `checks.commands[].view` is the post-run report and is not counted as a model saving.
- **Bypass.** Beyond JSON and machine flags: binary output (NUL or many undecodable bytes), digest-only output, hash
  tools, compressors/archivers, interactive commands (editors, pagers, `git add -p`, `git rebase -i`), edit tools
  (`patch`), and machine git subcommands (`apply`, `rev-parse`, `hash-object`, ...) never reach a lossy filter. One
  owner per transform: an external owner, nested host, known wrapper or lineage marker leaves the output raw.
- **Raw recovery** without re-execution: `command_view::recover` / `recover_by_id` (and `HostCommandChecks::recover_raw`)
  read bounded retained bytes, after success and failure, including for scratch trees that no longer exist.
- **Qualification.** The RTK adapter accepts only versions in `rtk_families.QUALIFIED_VERSIONS` (currently `0.51.0`);
  a newer version bypasses (`rtk-version-unqualified`) until re-measured. Findings per family are in
  `packages/semaprax-harness-adapters/rtk/RESEARCH.md` section 8.

Known gaps: the `semaprax harness run` CLI does not yet pass a tokenizer to the check stage (the library path does);
failed-check feedback exists for the HN-02 session loop only (scratch-repair feedback and HN-14 delegated results do
not carry it yet).

### Check stage tokenizer (HN-12)

`harness run --tokenizer-python P --tokenizer-script S --tokenizer NAME` spawns a tokenizer helper for the check
stage (`HostCommandChecks.tokenizer`) in addition to the request-budget helper, so check views are measured in named
tokens; without the flags measurements are bytes-only.

## TC-07: Caveman input-compression adapter (opt-in)

`packages/semaprax-harness-adapters/caveman/` (`ai.caveman/caveman-command-view`) is a `command.view` provider. It acts
as a client of a Caveman 3.1.0 runtime that the user starts (loopback `127.0.0.1:8787`), speaking
middleware protocol 1.1. The pin is the GitHub tag `v3.1.0` (commit `8af1f1b9`, full
`8af1f1b9b1346bca0722a1556f119b4e6675cc96`) built from source (`go build ./proxy/cmd/caveman-proxy`); npm has no 3.1.0
(`@caveman-ai/cli` stops at 2.0.1 and is the agent wrapper, not the runtime). It was checked against the upstream source
(`docs/technical/middleware-protocol.md`, `packages/sdk/python/caveman_cloud/middleware/*`) and, on 2026-10-04, run against
a real runtime built that way; the recorded exchange is `crates/semaprax-harness/tests/fixtures/caveman/recorded/` and is
replayed by a test.

The adapter never installs, starts or logs in to Caveman. It is adopted with `adopt --upstream <caveman>`, which only
probes `--version`, and is selected only by an explicit `command.view` provider pin. Setup never adopts it
implicitly, and RTK and raw remain the alternatives.

Each call takes these steps:
- It authenticates with a host-provisioned runtime bearer credential (`<retention>/caveman-token`). Upstream §2 requires
  one on every route, loopback included. Without a credential the plan bypasses and no request is made.
- It sends `Caveman-Middleware-Features: http_status_v2, revision_tolerant`. It never sends an Origin header or a model
  provider key.
- It refuses non-loopback endpoints before connecting and follows no redirects or proxies.
- It sends the already-captured output once, as a `tool_result` segment in `compress` mode under a fresh session id.
  The command is never re-run.
- It validates the plan as upstream `validate.py` does (`optimized | bypassed | record`; decisions are 200).
- It removes Caveman's own `caveman_retrieve` marker from the view. Semaprax retention stays the only recovery path
  offered to the model.
- Afterwards it calls `sessions/delete` to revoke the upstream originals.

A record-mode runtime, a bypassed plan, a view that is not smaller, a view that drops an error or fatal line, a bad
replacement digest, tiny output, patches, JSON, or a runtime failure or timeout all deliver raw output, with no
claimed saving. Telemetry and work tags are the runtime owner's settings; start it with `DO_NOT_TRACK=1
CAVEMAN_WORK_TAGS=0`. No upstream headline percentage is a Semaprax result. Only the TC-12 comparison of raw, RTK and
Caveman on the same tasks can qualify it.
