# Hot Reload Watcher v1

Audience: Project hot-reload implementers and reviewers.

Status: local library profile for HR-02. This is development-session support,
not a hosted, editor, or release-support claim.

## Boundary

`project::HotReloadWatcher` starts only when a client explicitly selects a
Project manifest and a development session. It obtains the active revision and
the exact authoritative input inventory from `with_authenticated_project`.
The adapter does not glob source files, recurse through a parent directory,
write source, execute a Project command, activate a candidate, or grant any
capability.

The selected implementation is client-driven bounded polling. `poll` compares
metadata for the current admitted inventory only; clients with native file
notifications inject `HotReloadWatchEvent` values into the same coalescer.
Events are hints. Every dirty generation performs normal Project admission and
the retained candidate is reauthenticated immediately before it reaches the
reload session and before explicit activation. An overflow forces that same
admission path. There is no native thread or background queue in this profile,
so stop drops pending work before returning.

Clients that may need to stop from another scheduling context retain a
`HotReloadWatchControl`. Its non-blocking `request_stop` is observed before
and after admission and before activation; the watcher clears its dirty slot
and pending candidate at that boundary. The control carries no Project input,
filesystem, or activation authority.

## Coalescing

The adapter retains one dirty monotonic event generation and one coordinator
candidate. Repeated events collapse to the newest generation. An event during
future asynchronous admission cannot submit its older generation. Identical
admitted revisions are no-ops. Rename, create, delete and overflow events all
cause a rescan when they affect an admitted input. After a manifest change,
paths beneath the admitted root are hints until fresh admission establishes the
new exact inventory; they are never read directly by the adapter.

## Standalone control adapter

`semaprax dev <semaprax.toml> --jsonl|--human [--interpreter|--source-agent]` starts no session until it receives a
closed `semaprax.hot-reload-control.v1` `start` frame on standard input. Its
only operations are `start`, `status`, `plan`, `activate`, `invoke`, and `stop`. Frames
have strictly increasing unsigned request IDs; unknown or duplicate fields,
unknown operations, invalid UTF-8 JSON, and stale IDs are rejected. The adapter
accepts at most 64 newline-delimited 4 KiB frames and emits at most 8 KiB per
response. Plans remain opaque in-process values: `plan` can render facts for
inspection, while `activate` consumes only the retained plan. JSON responses
are the sole standard-output bytes; human diagnostics remain on standard error.
Each response is written and flushed before the next input frame is read; there
is no response queue, so a slow consumer applies backpressure without growing
session memory. EOF, `stop`, a response-bound refusal, or a disconnected
consumer release the watcher and discard any retained plan before the CLI
returns.

`invoke` explicitly runs the current prepared interpreter entry after a
successful start or activation; a save itself never runs it. The public
`semaprax` binary refuses `--source-agent` at `start`: it has no source-live
host authority. The unpublished `semaprax-full` toolchain may select that lane
only with the exact closed operands of `source-live migrate` after
`--source-agent`: prior and destination configuration and checkpoint paths,
migration function and bound, then explicit `--opencode` executable and empty
`--scratch` directory. At `activate`, the adapter passes the retained A
revision and opaque compiler handoff to the source-live owner. That owner
replays the handoff, authenticates the predecessor journal, claims one fresh
destination journal, and performs the sole destination traversal. It currently
refuses priced or I/O-profiled source-live configurations, and never treats
the selection row as checkpoint, provider, policy, or cancellation authority.

Both output modes expose terminal uncertainty. JSONL retains the closed
`terminal_uncertainty` boolean; human mode prints `terminal_uncertainty` on
the corresponding status line. Neither form implies rollback or retry.

Invalid, missing, inaccessible, over-bound, escaping, or symlinked inputs are
reported by the Project admission owner as a rejected candidate. The prepared
worker retains its earlier revision. A stopped watcher cannot poll or activate.

## Evidence

The focused local unit module is `project::hot_reload_watcher::tests`. It uses
real temporary Project directories for burst coalescing, overflow recovery,
A-to-B-to-invalid-C rejection, stale-plan refusal, manifest membership failure,
symlink rejection, first-over-bound inventory refusal, event-generation
exhaustion, atomic save/delete-recreate, an edit between admission and candidate
commit, an edit before activation, same-byte no-op, valid repair after a
rejected candidate, derived-output and lexical path-escape hints, and external
stop during admission. These tests do not establish native-notification,
hosted, editor, or production support.
The source-built `project::hot_reload_cli` integration child also covers
A-to-B activation, invalid-C refusal with continued B invocation, hostile
framing, and EOF/Stop through the bounded JSONL control stream. Its unit
partner exercises partial-write backpressure and disconnected-output shutdown.
