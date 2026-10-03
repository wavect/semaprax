# Hot Reload Session v1

Status: partial local library profile for HR-01. The prepared interpreter lane
has a checked revision coordinator; source-Agent checkpoint handoff and a
complete compatibility planner are not implemented by this profile.

## Boundary

`HotReloadSession` owns one prepared Project interpreter, its active checked
`ProjectRevision`, a monotonically increasing generation, one pending checked
candidate, and a terminal uncertainty bit. The caller supplies immutable
revisions and any capabilities needed to construct them. The session has no
watcher, source discovery, filesystem, network, provider, process, or
publication authority. It can replace the prepared interpreter only between
invocations. It cannot migrate an executing stack, native library, Wasm memory,
outstanding FFI resource, callback, or source-Agent state.

Candidate admission runs the ordinary Project check before retaining a pending
revision. Planning reads retained checked HIR; it does not touch the active
worker. The plan is an opaque in-memory value. Its JSON representation has
`authority: none` and cannot be parsed back into a plan. The plan digest binds
the generation, submission identity, exact Project and Program roots, selected
entry/test identities, decision, and reason. The digest is an integrity check,
not permission. Activation independently rebuilds the plan and delegates the
whole-state pivot to `PreparedProjectInterpreter::replace_revision`.

## Current compatibility rule

The local prepared-interpreter lane admits a changed code body only when the
checked entry and test programs retain their selected entrypoints, permit set,
type and interface records, the exact function stable-ID set, return/parameter types and
ownership, declared effects and yields, and checked pre/postconditions.
Source-Agent candidates require an explicit restart in this version. This
rule is conservative and incomplete: it does not yet compute a reachable
callable closure, distinguish display-only renames from structural changes,
or select an Agent checkpoint migration. A positive decision is limited to
the checked scalar prepared-interpreter profile; it is not a general hot
reload guarantee.

## Transition table

| State and request | Result | Active runtime |
| --- | --- | --- |
| Live; admit a valid candidate | Replace the one pending candidate and advance submission identity | Unchanged |
| Live; invalid candidate or exhausted submission identity | Reject with `invalid_candidate` or `generation_exhausted` | Unchanged |
| Live; plan pending candidate | Read-only eligible, unchanged, unsupported, or rejected decision | Unchanged |
| Live; activate matching eligible plan | Delegate one worker pivot; advance generation on acknowledgement | New complete revision |
| Live; activate stale generation or superseded candidate | Reject `stale_generation` or `stale_candidate` | Unchanged |
| Live; worker has an outstanding invocation | Reject `busy_boundary`; preserve pending plan for an explicit later attempt | Unchanged |
| Live; activate identical or incompatible plan | Reject `identical_revision`, `incompatible_closure`, `policy_changed`, or `unsupported_target` | Unchanged |
| Live; worker rejects candidate before pivot | Clear pending candidate and return its diagnostics | Old complete revision |
| Live; worker panic or lost acknowledgement | Enter `terminal_uncertainty`; refuse later operations | Unknown; no rollback claim |
| Terminal; any admission, plan, activation, or execution | Refuse `terminal_uncertainty` | Unknown |

At most one candidate and one worker request are retained. Generation and
submission counters use checked `u64` increments; their first over-bound
increment refuses before changing session state. Candidate source, checked HIR,
worker stack, trace, and report bounds remain the ordinary Project and
prepared-interpreter limits. No queue or automatic retry is introduced.

The session emits `SPX-HR400` for its own typed refusals and preserves the
underlying Project and prepared-interpreter diagnostics for those owners'
failures. A caller must inspect the typed reason as well as the diagnostic.

The focused local gate at `project::hot_reload::tests::` passed 3/3 with
`CARGO_TARGET_DIR=target/hr01 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline -p semaprax --lib
project::hot_reload::tests:: -- --nocapture`. This is local library evidence,
not hosted or source-Agent handoff evidence.

## Completion work

HR-01 still needs the source-Agent durable-checkpoint lane, compiler-derived
reachable closure and state compatibility, a shared table-driven transition
suite including a physical busy boundary and first-over-bound inputs, and broader adversarial
coverage of rename, entry, effects, contracts, and missing stable IDs.
