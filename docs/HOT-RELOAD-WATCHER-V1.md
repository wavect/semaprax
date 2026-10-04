# Hot Reload Watcher v1

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
the retained candidate is reauthenticated immediately before explicit
activation. An overflow forces that same admission path. There is no native
thread or background queue in this profile, so stop drops pending work before
returning.

## Coalescing

The adapter retains one dirty monotonic event generation and one coordinator
candidate. Repeated events collapse to the newest generation. An event during
future asynchronous admission cannot submit its older generation. Identical
admitted revisions are no-ops. Rename, create, delete and overflow events all
cause a rescan when they affect an admitted input. After a manifest change,
paths beneath the admitted root are hints until fresh admission establishes the
new exact inventory; they are never read directly by the adapter.

Invalid, missing, inaccessible, over-bound, escaping, or symlinked inputs are
reported by the Project admission owner as a rejected candidate. The prepared
worker retains its earlier revision. A stopped watcher cannot poll or activate.

## Evidence

The focused local unit module is `project::hot_reload_watcher::tests`. It uses
real temporary Project directories for burst coalescing, overflow recovery,
B-to-C supersession and stop. These tests do not establish native-notification,
hosted, editor, or production support.
