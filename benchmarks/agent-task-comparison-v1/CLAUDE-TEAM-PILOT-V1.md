# Native Claude Team coding-agent pilot v1

This additive private adapter uses the existing task, lane, drift, sandbox,
compiler gateway, MCP byte capture, candidate archive and independent acceptance
mechanisms. `claude_pilot.py` supplies native Claude receipts through the optional
`run_tuple` transport interface. The default OpenCode route and its receipt
schema retain their existing behavior. No OpenCode event/export is synthesized.
The native `session.json` is the exact native JSON result, also retained as the
raw stdout; its schema and model-usage decoder are distinct.

## Schedule and review policy

The owning manifest has three tasks, two available lanes and three repetitions:
18 scheduled positions. The current two-model protocol runs that whole schedule
for each model, retaining 36 records. This preserves the denominator rather than
silently applying the historical 18-record audit to a two-model cohort. Zero
remains external/unrun. The counterbalanced task/lane schedule is reused without
changing prompts, fixtures, drift, acceptance or lane restrictions.

The operator may record the user's explicit review waiver as
`review.mode = operator_recorded_user_waiver` with a non-secret message reference.
This is an operator assertion, not independent authentication of the user. The
legacy human-review eligibility/status remains unchanged. Native records expose
separate technical eligibility, retain all other unavailable metrics, and keep
human reviewer ID and active time null. Optional AI technical review is separate;
it is never entered as human review or human active time.

## Frozen authority and limits

`freeze` binds the exact runner HEAD, implementation hashes, manifest/schedule,
native platform, Claude/compiler executable hashes, explicit auth home/login,
private evidence root, authorization reference and review mode. The compiler
artifact may be a pinned installed release; retain its own reported version and
revision without relabelling it as a compiler built from the runner HEAD.

Only Claude 2.1.286 Team subscription authentication is used. The two identities
are the exact Haiku request/usage key `claude-haiku-4-5-20251001` with canonical
model `claude-haiku-4-5`, and exact Sonnet `claude-sonnet-5-5` for both fields.
Usage keys and canonical models are checked independently. The adapter accepts
no API-key, endpoint, secret environment or billing-configuration argument.
The provider CLI is trusted for subscription authentication and reported usage;
its counters are not invoice proof or cryptographic provider attestation.

Limits are fixed: 300 seconds per CLI trial, 32 agentic turns, 65,536 prompt
bytes, 1 MiB captured output, 131,072 fresh input/output/cache-creation tokens and
1,048,576 cached-read tokens (post-response admission),
$0.25 CLI API-equivalent budget per trial, and $9 aggregate reserved budget.
The provider's budget enforcement is not a guarantee about invoice charges.
A locked, fsynced before-spawn reservation consumes the full per-trial amount;
failed/ambiguous attempts never refund it. A pending, unknown or overrun cost
blocks further dispatch. Calls are sequential and trial directories are
create-new. There are no agent retries. Failed and unattempted positions stay
visible in the final inventory.

HOME, evidence root, candidate and private host state must be canonical and
pairwise disjoint. Use a private directory outside HOME for execution evidence;
after all work stops, preserve it at a durable private location with a hash/path
mapping. The native CLI alone receives the explicit subscription HOME. It has no
built-in tools, uses restricted/strict-MCP flags with empty setting sources, disabled slash
commands, and explicit hook/memory/CLAUDE.md suppression, refuses managed
settings, and receives only the named compiler MCP tool. Those settings suppress
user/project settings, hooks, skills, auto-memory and CLAUDE.md discovery. A real metadata-only
positive/negative gate verifies that a hostile SessionStart hook executes
without these flags and is suppressed with them, using zero user/model turns.
The MCP subtree receives a closed private environment and the existing physically
probed seatbelt profile denying repository/auth-home/evidence reads and external
writes. Exact argv, provider bytes, MCP configuration and receipt hashes are
retained privately. The preexisting gateway's per-command deadline remains
separate from the provider CLI deadline.

## Commands and evidence

Prepare a private authority JSON containing exactly `claude`, `compiler`, `home`,
`login`, `evidence_root`, `authorization`, and `review` (`mode`, `authorization`).

```sh
python3 benchmarks/agent-task-comparison-v1/claude_pilot.py freeze \
  --authority /private/path/authority.json --output /private/path/protocol.json
python3 benchmarks/agent-task-comparison-v1/claude_pilot.py run \
  --protocol /private/path/protocol.json --protocol-sha256 APPROVED_SHA256 \
  --model-id haiku45 --position 1
python3 benchmarks/agent-task-comparison-v1/claude_pilot.py audit \
  --protocol /private/path/protocol.json --protocol-sha256 APPROVED_SHA256 \
  --output /private/path/audit.json
```

Freeze only after implementation commits settle. Repeat each scheduled position
once for `haiku45` and `sonnet55`. Audit retains every position, rejects unexpected
records, binds model/task/lane/trial/revision/waiver, verifies primary evidence
hashes and re-derives observed native usage. It does not execute missing trials,
repair old receipts, infer model tokens from bytes, or claim human review.

Focused gate: `python3 -m unittest discover -s
benchmarks/agent-task-comparison-v1 -p test_claude_pilot.py -v`.
`SEMAPRAX_PILOT_CLAUDE` opts into the real zero-inference customization probe;
`SEMAPRAX_PILOT_METADATA_EVIDENCE` optionally preserves it at a create-new path.
The initial six-selector run passed in 0.723s, including that physical metadata
probe. The unchanged OpenCode source/graph boundary selector passed 1/1 (both
lanes) in 3.850s. These gates are transport fixtures, not trial results.

Any native integrity/admission failure halts later dispatch even when the provider
reported an in-cap cost. The private ledger retains that reported cost and the
selected admission failure; raw response evidence remains available.

Native safe-mode is deliberately absent: CLI 2.1.286 disables explicit MCP
servers in that mode. A zero-user-turn physical gate requires the confined
`semaprax` server to connect and advertise only `command`, with no slash commands
or hostile SessionStart execution. `--tools ""` disables built-ins while this
explicit MCP server remains admitted. Use canonical `TMPDIR=/private/tmp` on
macOS. The earlier two-position packet remains an aborted cohort.

The first MCP-enabled cohort halted after one genuine 15-tool trial reported
$0.0509904 against its frozen $0.05 CLI limit. It remains immutable failed
evidence. The explicitly authorized next cohort uses the larger bounds above,
with a distinct protocol digest; the halt and no-refund rules are unchanged.

A separately frozen continuation profile admits only the pinned CLI's observed
`error_max_turns` / `is_error=true` / `terminal_reason=max_turns`, exit 1, with
`num_turns <= max_turns + 1` (the CLI's terminal exhaustion count). Exact model,
provenance, counters and cost must still pass admission. It runs independent
acceptance on the retained partial candidate and records a failed task, without
halting other trials. Other provider errors, unknown cost, timeout, identity
mismatch and overrun retain the global halt. This does not reinterpret any
previous halted protocol or record; fresh/cache-read counts are retained exactly.

The final separately frozen profile allows 300 seconds for the unchanged
32-turn and token/cost limits. The prior 120-second timeout packet remains
immutable and halted. MCP receives explicit `TMPDIR` equal to its private state
root, avoiding ambient xcrun cache attempts without enlarging file authority.
