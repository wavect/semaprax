# Command view v1 (HP-08, HP-09 host side)

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
(closed): `min_bytes`, `timeout_ms`, `mem_cap_bytes`, `provider_timeout_ms`, `retention
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
