# Bend 2 law benchmark v1

Issue #392 compares law-preserving development under an explicitly pinned
local configuration. The owning runner is
[`benchmarks/bend2-law-v1/run.py`](../benchmarks/bend2-law-v1/run.py) and its
manifest is the only committed task inventory.

## Admission

A result is admitted only when the command file pins accessible Git roots for
both Bend and SEMAPRAX, the observed heads equal those declared roots, Bend is
at `947db722640c86247849343657bf2f7ef01cb7f1`, and every one of five execution
paths is declared separately. The five paths are ordinary Bend checking,
Bend `--verdict`, SEMAPRAX SMT, external Lean, and SEMAPRAX runtime/test.
Their receipts are never merged.

Each cell declares its numeric semantics and equal law inventory. The runner
executes the success subject once cold and at least thirty warm times, retains
all raw warm samples with p50/p95, and runs every seeded law-gaming control.
Any accepted attack fails that execution path. Missing tools, timeouts,
identity drift, unsupported targets, and mismatched domains are unavailable
or failed outcomes; they have no score and cannot establish a win. Smaller
runs need `--pilot` and remain pilot evidence.

## Scope and remaining evidence

The manifest has all six required cells: scalar contract bug, balance
transfer, list theorem, law-preserving refactor, law-breaking agent edit, and
project incremental edit. It binds sort permutation/multiplicity plus
sortedness, and balance conservation plus intended state change, closing the
empty-sort and no-op-transfer loopholes at the harness boundary.

Each cell has a digest-bound checked-`u32` input/output corpus under
`benchmarks/bend2-law-v1/fixtures/`. It is language-neutral because the
reviewed SEMAPRAX scalar profile does not admit `u32`; replacing it with its
smaller `i32` profile would make the comparison unequal. Bend's pinned `U32`
implementation/proof sources are also unavailable until its pinned executable
and checkout are provisioned locally. These are explicit unavailable cells,
not successful evidence.

No result is committed. Closure still requires matched executable source and
proof fixtures for each declared `u32 checked` domain, a quiet-host
30-repetition run with both pinned checkouts, and agent trials with ten
independent fixed-budget trials per admitted language/model plus token/cost
event provenance. Runtime throughput and GPU scaling remain a separate family.

`benchmarks/bend2-law-v1/agent_trial_plan.py` provides the prior static
preregistration boundary. It authenticates the manifest/fixture bytes and a
fixed model, tool-access, and budget configuration, requires at least ten
trials per language/cell, and labels every unmatched numeric-domain cell
`unsupported` without creating an outcome. The checked-in scalar profile has
no admitted checked-`u32` cell, so this is not execution evidence or a pilot.

The bounded Codex route in `benchmarks/bend2-law-v1/codex_agent_trial.py`
executes one preregistered Boolean source/proof edit prompt only in a fresh empty directory with
`codex exec --ephemeral --ignore-user-config -s read-only --json`. It exposes
no repository workspace, records the exact JSONL event stream, stderr and
Codex version bytes, and checks the reported input/cached-input/output token
total against the fixed plan budget. Since those events have no monetary usage
field and a single turn cannot partition proof synthesis, law-kernel checking,
and compilation/runtime, its result is explicitly `edit_artifacts_captured`
rather than an accepted trial. The prompt embeds the attack source, law and
witness; `boolean_pair_acceptance.py` v2 binds the resulting final response,
final source and attack claim. Their static distinction from the seeded attack
remains distinct from tool rejection. `agent_trial_capture.py` v2 separately requires every
transcript, telemetry, phase, and acceptance artifact as a bounded regular
file below a selected artifact root and re-hashes it; digest-only v1 exports
are rejected.

`boolean_pair_acceptance.py` adds a narrower read-only final-artifact review
for one preregistered Boolean ordinal. It authenticates each retained source
and verification file under a no-link artifact root, requires exact pinned
success source bytes for Bend and SEMAPRAX, and distinguishes Bend's raw
verdict marker from SEMAPRAX's runtime witness. The latter leaves the
SEMAPRAX formal proof phase unavailable; provider cost is also unavailable.
Its `source_pair_authenticated` result cannot claim a successful law repair,
agent authorship, command execution, or a matched comparative result.

## Pinned Boolean smoke route

`benchmarks/bend2-law-v1/bend_boolean_driver.py` is a narrow provisioning
check for the reviewed Bend revision. It verifies the checked-out source head,
runs `fixtures/bend-two-value-boolean-v1.bend` through the normal path and the
separate `--verdict` kernel path, and requires the normal output to be exactly
`0` then `1` for `False{}` and `True{}`. The fixture proves its match agrees
with Bend's `Bool.to_u32`; its result file names no SEMAPRAX subject and is not
one of the checked-`u32` comparison cells. A driver failure or unavailable Bun,
Lean kernel, or pinned checkout remains a non-result.
