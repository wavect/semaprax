# Harness host bridge v1 (HP-14, HN-14)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

Audience: toolchain contributors and harness adapter authors.

`semaprax-harness bridge <project> --stdio | --mcp | --setup claude-code [--write] | --host claude-code [--hook pre-tool-use | --print-config]
[--settings-file F]... [--log F] [--harness-bin P] [--session ID] [--host-skills-dir D] [--harness-home D]`. Implementation:
`crates/semaprax-harness/src/bridge/`. Diagnostics `SPX-HPN`: 001 handshake/protocol, 002 recursion,
003 publication refused, 004 method or order, 005 params, 006 hook input, 007 usage/unsupported host,
008 delegated verb failed, 009 competing rewriter, 010 log write, 011 setup refused (HN-14), 012 too many in flight,
013 duplicate request id, 014 no trusted provider for `bridge/invoke`, 015 step not replayed (HN-18). 007 also covers an
unsupported host version.

The root MCP facade `src/semantic_service_mcp.rs` is untouched: it stays authority-free. The bridge
is a separate, host-side surface; it adds no process, filesystem or network authority to semantic methods.

## Protocol `semaprax.harness-bridge.v1`

LF-delimited JSON-RPC 2.0 on stdio, frames parsed with the strict `crate::json` parser (1 MiB limit).
`bridge/handshake` first; `params`: `{protocol, version: 1, host: {name, version}, capabilities:
{semantic_query, tool_result_observation, command_wrapper, model_routing, cancellation, publication:
bool}, command_rewriter: null|"name", bridge_depth?, lineage?}` (closed object).

The result names one owner per capability: `semaprax`, `external-host` or `unavailable`, each with a
reason. An undeclared capability is `external-host` and listed in `not_claimed_optimized`. `single_owner`
gives exactly one owner for command interception, compression, retries and model routing. A host that
reports an existing rewriter (for example RTK) keeps `command_wrapper`; Semaprax does not wrap again.
Publication is always `external-host`. `observed_scope` states that only calls routed through the
bridge are observed and that no whole-session saving is claimed; metrics belong to the #357 report
surface (`semaprax-harness report`, `editors/vscode/token-report.js`), not to the bridge.

| Method | Behaviour |
| --- | --- |
| `bridge/status` | the exact `status --json` document (single source) |
| `bridge/context` `{query, max_bytes?, symbol?, references?}` | the `context --json` document |
| `bridge/command_view` `{argv, raw?, timeout_ms?}` | `command_view::execute`; when another owner holds `command_wrapper` it runs with `external_owner`, so the view is not transformed again |
| `bridge/publish` | always refused (`SPX-HPN003`) |
| `bridge/invoke` `{capability, operation, payload?, deadline_ms?, step?, isolation?}` | one provider invocation through the adapter host (HN-18, below) |
| `bridge/cancel` `{id}` | cancels the in-flight `bridge/invoke` whose JSON-RPC `id` is `id` (HN-18, below) |
| `bridge/shutdown` | ends the session |

Recursion guard: any call is refused (`SPX-HPN002`) when `SEMAPRAX_HARNESS_BRIDGE_DEPTH` is at least 1,
the command-view lineage marker is set, the handshake carries `bridge_depth >= 1`, or a lineage entry
starts with `semaprax`.

## Lifecycle extension (HN-18): invoke and cancel

The handshake result gains `lifecycle {invoke, cancel, max_in_flight, cancel_states}`. Methods other than
`bridge/invoke` are still answered inline; `bridge/invoke` runs on a worker thread, so the session keeps
reading frames while an adapter is blocked and a second frame (`bridge/status`, another invoke, `bridge/cancel`)
is served concurrently. The invoke's JSON-RPC `id` (string or number, required) is the correlation id.

`bridge/invoke` resolves the project's selected, trusted provider for `capability` exactly like `run`
(`profile::resolve_project`, grant re-checked, adopted runtime), then calls `AdapterHandle::invoke` with a
per-request `CancelToken`. It invents nothing per adapter: deadline, cooperative cancel, process-group kill and reap,
the crash breaker and the retry boundary are the host's (`docs/HARNESS-HOST-V1.md`). Class by kind:
`model.generate` is `SideEffecting` (possibly billed), `decision.evaluate` is `Decision`, the rest `SafeRead`.
Optional controls are validated by presence before profile resolution, handle preparation, journal or dispatch
(`SPX-HPN005`): absent takes the default; present must have the documented type and value, never a silent default.
`deadline_ms` is an integer in `1..=600000` (default `30000`); outside it is refused, not clamped. `step` is a string of
1 to 128 characters from `[A-Za-z0-9._:@/+-]`. `isolation` is exactly the string `required`. `isolation: "required"` requests the OS-enforced restriction; without `sandbox-exec`/`bwrap` the call is an error
(`SPX-HPC003`), never a plain subprocess. Every result reports `isolation {mode, isolated}`: a plain subprocess is
`mode: subprocess, isolated: false`.

| Frame | Meaning |
| --- | --- |
| `bridge/cancel {id}` for a live id | `state: cancel-requested`. The token is tripped; the invoke's own response carries the final state |
| invoke response `state` | `completed`, `confirmed-terminated`, `uncertain-external-effect`, `refused`, `unavailable` |
| `bridge/cancel {id}` after settlement | the settled state (last 256 ids kept), `cancelled: true` only for `confirmed-terminated` |
| `bridge/cancel {id}` for a never-seen id | `state: unknown-id`; not remembered, so it cannot poison a later request reusing the id |

`confirmed-terminated` is returned only when the host killed and reaped the owned process group (or the request was
never written). A `SideEffecting` request that was sent is `uncertain-external-effect` (host `SPX-HPC018`): the process is
gone but the external effect is unknown. A result that arrives after cancellation was requested is discarded
(`result_discarded: true`) and never returned as current; for a side-effecting step it is also `uncertain-external-effect`.
Responses carry `retried: false`: cancellation never causes a second attempt.

Bounds: at most 4 invocations in flight (`SPX-HPN012`), unique ids (`SPX-HPN013`), 1 MiB frames, the adapter's own
queue and concurrency limits. A client that closes stdin (crash or hang-up) cancels every live invocation and the session
returns only after they are reaped. A bridge killed by `SIGKILL` cannot run that cleanup: its adapter's process group
survives until the adapter exits or its idle/deadline limits fire (Unix offers no parent-death signal in safe std).

**Side-effecting steps and the journal.** A `SideEffecting` invoke appends `begin` to the append-only journal
`<harness home>/cache/bridge/<project id>.journal.jsonl` (`workflow::journal`) and then `done`, `refused`, `cancelled`
(request never written) or `uncertain`. A later invoke with the same `step` is refused as `SPX-HPN015` unless its last
record is `refused` or `cancelled`; a begun, completed or uncertain step is never replayed, across restarts.

*Single writer.* `Journal::open` holds an exclusive advisory `flock` on `<project id>.journal.flock` for the journal's
lifetime and reads the records only after it holds the lock. The bridge opens the journal per claim and per settlement
(the lock is held only for the claim-and-`begin` critical section, never across the provider call), so concurrent
sessions and processes on one project serialise: exactly one claims a given `step`, the others get `SPX-HPN015` (or
`SPX-HPD070` busy if the lock stays held for 10 s). The lock is released by drop or process exit; the lock file is never
deleted, and durable `begin`/`uncertain` records are untouched. Projects and lineages lock independently.

*Step identity.* `step` is the caller's durable idempotency key and keeps replay protection across restarts. When omitted,
the host assigns `auto-<session>-<n>` (session token from pid, clock and a process sequence; `n` the invocation counter),
journals it with `begin` before dispatch and returns it as `step` in the response, so independent calls never collide and
a restart or reused JSON-RPC id cannot alias earlier work. Journals written by earlier versions (default
`<capability>:<operation>` keys) are read unchanged: an old `begin`/`uncertain` entry is never made retryable, and a caller
that passes that old key explicitly is still refused.

*Terminal persistence failure.* If appending `done`/`refused`/`cancelled`/`uncertain` fails (open, write or sync), the
provider outcome is returned unchanged and the response adds `durable: false` and `journal_error {code, message}`; absent
on healthy settlement. The provider is never retried, the surviving `begin` keeps the step non-replayable, and a torn
tail makes the next open fail closed (`SPX-HPD070`).

## Claude Code (pinned 2.1.289)

Documented API: PreToolUse hooks (https://code.claude.com/docs/en/hooks) with `claude --help` flags
`--settings`, `--setting-sources`, `--allowedTools`. The hook reads the PreToolUse JSON on stdin. For a
Bash command that is one plain, resolvable command (no shell syntax, admitted by
`command_view::intent::check_syntax`, not excluded by `policy::exclusion`) it prints
`hookSpecificOutput.updatedInput.command = '<harness>' 'exec' '<project>' '--' <argv...>`; it grants no
permission decision. Everything else passes through with no output. An RTK hook found in the disclosed
settings text (`--settings-file`, read-only; port of `rtk/hook_detect.py`) disables the rewrite, and
`--print-config` then refuses to print a second rewriter.

`--print-config` prints a project-local settings snippet for review. Semaprax never writes or reads
`~/.claude/*`. Model choice is host-controlled: Claude Code does not delegate it, and nothing here
advertises Jev/Laya routing. Observed scope is Semaprax-routed Bash calls only. `--log` appends one JSON
line per hook invocation. Other hosts need their own adapter; none is inferred from a name.

## Editor

`editors/vscode/harness.js` parses the same `status --json` document; commands "SEMAPRAX: Show Harness
Status" and "SEMAPRAX: Inspect Harness Provider" call the configured compiler's `harness` verbs. Enable
and disable map to the existing `trust`/`revoke` verbs. No new activation event or dashboard.

## Default skills (HN-14): protocol `semaprax.harness-bridge.v2`, MCP and setup

One adapter, `bridge/skills_bridge.rs`, serves `DefaultSkills` (the catalog native runs and `skills ...` use). No
prompt text exists in the bridge, MCP server, setup output or editor: every delivered byte is
`DefaultSkills::load`/`load_resource` output, framed by `skills::frame`. The authority-free compiler MCP facade is untouched.

`bridge/handshake` with `protocol: semaprax.harness-bridge.v2`, `version: 2` additionally accepts `session` (identifier,
default `default`, the CLI default) and `host_skills: [{name, digest?}]` (official skills the host already has). The result adds
`delegation` (per capability `delegated`/`not-delegated`), `methods`, `identity {project, session}` and `skill_injection`.
v1 sessions are unchanged and refuse `bridge/skills/*` (`SPX-HPN004`). The project id is
`skills::cli_defaults::project_id` of the canonical project path, so CLI, bridge and editor address one state file.

| Method | Behaviour |
| --- | --- |
| `bridge/skills/list` | catalog metadata and per-skill state (no body) |
| `bridge/skills/load` `{name, force?}` | framed body of the session-locked revision, or `host-owned` |
| `bridge/skills/use` `{name, mode?, scope?, force?}` | explicit mode (`skills use`) then delivery; `off`/`all` deliver nothing |
| `bridge/skills/off` `{name, scope?}` | explicit stop (final for the session) |
| `bridge/skills/resource` `{name, path}` | one bounded resource (`SPX-HPM033` otherwise) |
| `bridge/skills/status` `{updates?}` | the `skills status --json` document plus `schema semaprax.bridge-skills-status.v1`, `project`, `session`, `host`, `host_owned`, `model_routing`, `delivery`; `updates: true` adds pending updates from `updates status --offline` |

Unknown params are `SPX-HPN005`. Skill errors keep their `SPX-HPM` codes.

**Ownership.** An official skill the host already installs is never injected twice. Detection: the v2 handshake
`host_skills`, or `--host-skills-dir D` (for Claude Code the project `.claude/skills`), read once, read-only, no symlinks,
`SKILL.md` hashed against the catalog (`same-revision`, `different-revision`; a name-only declaration is
`revision-not-verified`). A host-owned `load`/`use` returns `delivery.state = host-owned` without text; `use` still records the
mode, `force: true` delivers anyway. Nothing is read from `$HOME`. Model routing stays `not-delegated` unless the handshake
declares `model_routing`; nothing here advertises Jev or Laya.

**Delivery observations.** `--log F` appends one canonical JSON line per observation: `skills.session` (client name and
version), `skills.list`, `skills.mode`, `skills.delivered` (skill, version, exact `revision`, mode, bytes, `forced`),
`skills.host-owned`, `skills.off`, `skills.resource`; each carries `host`, `project`, `session` and a per-process `seq`.
These are deliveries to the host. Model consumption is not observed, so `applied_to_model` stays false.

**MCP (`bridge <project> --mcp`).** Newline-delimited JSON-RPC MCP stdio (protocol 2025-06-18, 2025-03-26, 2024-11-05):
tools `skills_list`, `skills_load`, `skills_use`, `skills_off`, `skills_status`, `skills_resource` and one prompt per official
skill (`prompts/get` is an explicit `use`). `initialize.clientInfo` is the host declaration: a `claude-code` older than 2.x is
refused (`SPX-HPN007`, tested 2.1.289); tool calls before `initialize` are `SPX-HPN004`.

**Setup (`bridge <project> --setup claude-code [--write] [--session ID] [--log F] [--harness-home D]`).** Prints a plan;
with `--write` merges one `mcpServers.semaprax-skills` stdio entry into `<project>/.mcp.json` (documented project scope,
https://code.claude.com/docs/en/mcp). Unrelated servers and keys are kept (key order is normalized); a repeat is `noop`; an
unparseable file is refused (`SPX-HPN011`) and left alone. Nothing under `~/.claude` is read or written; permission rules are
printed as a suggestion only. No skill file is copied into `.claude/skills`. Other hosts are refused (`SPX-HPN007`).

**Editor.** `Show Harness Status` also runs `skills status --json --project <id>` and `updates status --json --offline` and
shows default availability, mode and pinned revision per skill, host ownership, model routing, pending updates and disabled
reasons; a failed query is shown as a stated reason. No second dashboard and no new activation event.

### Evidence (local macOS aarch64, 2026-10-04)

- Claude Code 2.1.289 (`--model haiku`, `--setting-sources project,local`, `--strict-mcp-config --mcp-config <setup-written .mcp.json>`):
  `real_tools_v1 external_host::hn14_claude_code_...` (two `claude -p` turns): list, `skills_use ponytail full`, a coding edit,
  then `skills_use caveman` and `skills_off caveman`. The log records delivered revision `sha256:34dc9057...0ec7` (Ponytail
  v4.10.3) and `sha256:006eda17...1502` (Caveman v3.1.0). Cost 0.1075 USD. Behavioural compliance with a skill is not asserted.
- opencode 1.18.33 as the second client (project `opencode.json`, temp XDG dirs, local Ollama `qwen2.5:0.5b`, no paid call):
  `external_host::hn14_opencode_...` reaches the same catalog and records the same Ponytail revision. opencode is a fixture
  config in the test, not a supported `--setup` target.
- A generic stdio MCP client (`bridge::hn14_real_process_generic_mcp_client_over_stdio`) runs against the real binary.
