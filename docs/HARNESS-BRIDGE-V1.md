# Harness host bridge v1 (HP-14)

`semaprax-harness bridge <project> --stdio | --host claude-code [--hook pre-tool-use | --print-config]
[--settings-file F]... [--log F] [--harness-bin P]`. Implementation:
`crates/semaprax-harness/src/bridge/`. Diagnostics `SPX-HPN`: 001 handshake/protocol, 002 recursion,
003 publication refused, 004 method or order, 005 params, 006 hook input, 007 usage/unsupported host,
008 delegated verb failed, 009 competing rewriter, 010 log write.

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
| `bridge/cancel` | honest no-op: requests are served one at a time |
| `bridge/shutdown` | ends the session |

Recursion guard: any call is refused (`SPX-HPN002`) when `SEMAPRAX_HARNESS_BRIDGE_DEPTH` is at least 1,
the command-view lineage marker is set, the handshake carries `bridge_depth >= 1`, or a lineage entry
starts with `semaprax`.

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
