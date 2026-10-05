# Bend 2 law benchmark v1

Status: local pinned benchmark harness; no hosted result claim.
Audience: benchmark operators and reviewers.

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

The unified `law16_replay.py` sequence retains a per-route
`<name>.command.json` receipt with argv, exit status, timeout budget, timeout
disposition, and digests of stdout/stderr. A timed-out route retains the
partial streams returned by the process runner and has a null exit status;
it cannot become a successful route or a law-gaming rejection. Failure of a
later route preserves the generated artifact inventory in `replay-status.json`
so earlier output remains reviewable. These orchestration receipts do not
replace the individual routes' semantic validators or timing samples.

`--verify-fresh CAPSULE --output-dir NEW_DIRECTORY` authenticates a completed
non-agent capture offline: the exact artifact inventory and tool-pin digest,
ordered successful command receipts, Boolean and RSS route validators, Bend
universal proof validation, supplemental control outcomes, and retained Lean
test output. Copying a capsule preserves original command paths. RSS sample
commands must equal the captured provenance commands, which bind the pinned
tools and exact retained input names; a same-named foreign input is rejected.
This review does not execute a tool or reproduce timing measurements.

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

The supplemental project-incremental capsule exercises the real three-module
`examples/calculator-project` cache path. Its provider body edit reparses the
provider and reuses the two consumers; a changed provider signature is rejected
by both warm and cold test routes. The capsule records exact source and local
test-binary identities and raw test streams. It does not establish a matched
Bend route, checked-`u32` admission, large-project scale, proof, or timing.

The supplemental Boolean native-phase capsule runs 30 local repetitions of
Bend ordinary checking, C emission, separate Clang compilation, and the
resulting executable. It also runs SEMAPRAX checking, its combined native
build command, and the resulting executable. Raw command streams and generated
artifacts are retained with digests. Each native output must match the fixed
two-value witness. The profile has no cache isolation or formal proof route,
and it cannot split SEMAPRAX build's internal check/codegen/compile work.

The Boolean agent campaign has a separate preregistered Claude Haiku profile:
ten matched pairs, fresh no-tool sessions, a requested per-call budget, and
independent Bend verdict/SEMAPRAX Z3 candidate and attack replay. The v2
profile retains failures as trial outcomes and reports provider token and
monetary events from sanitized streams. Its completed 20-trial capsule has one
failed Bend candidate and nine fully accepted pairs. The earlier v1 profile
stopped after the CLI exceeded its requested per-call budget; its observations
are not pooled with v2.

The five checked-`u32` cells have a digest-bound input/output corpus under
`benchmarks/bend2-law-v1/fixtures/`. It is language-neutral because the
reviewed SEMAPRAX scalar profile does not admit `u32`; replacing it with its
smaller `i32` profile would make the comparison unequal. Pinned Bend `U32`
source and verdict observations are retained, but they have no equal
SEMAPRAX checked-`u32` route. These are explicit unsupported matched cells,
not successful cross-language evidence.

Issue acceptance requires reproducible evidence for admitted benchmark
configurations and an explicit disposition for every original manifest cell.
This follows #392's instruction to state unsupported cells rather than
substitute a weaker task. Unsupported cells retain their original identities,
numeric domains, laws and attacks; they contribute no timing result,
successful task outcome or superiority claim. Completing the benchmark does
not admit checked-`u32` source syntax. Supplemental profiles remain separately
identified and do not replace original manifest cells.
The supplemental matched Boolean Project edit records three-module Bend and
four-source SEMAPRAX check routes, an unchanged-law provider-body edit, a
signature attack rejected on both sides, and thirty ordinary-check samples
per lane/state. It does not establish incremental cache reuse, a large-project
result, or admission of the original checked-`u32` Project cell.

Available configurations must satisfy the required repetition, phase
separation, provenance, control and agent-trial gates, including thirty timed
repetitions per microbenchmark configuration and ten independent fixed-budget
trials per admitted task/language/model with token/cost provenance. Retained
ordinary/check and proof/verdict guest capsules now authenticate thirty
cold/warm file-page-cache pairs per admitted route. A separate host capture
records thirty source-to-SMT process samples. Guest file-page-cache
observations retain their exact scope; host and Rosetta cache state are
unobserved. SEMAPRAX's combined native build metric does not establish
isolated compilation time. Runtime throughput and GPU scaling remain a
separate family.

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

## Guest file-page-cache profile v1

`law16_guest_cache.py` adds an explicitly separate Linux x86_64/Rosetta
profile for the fixed Boolean-negation ordinary Bend and SEMAPRAX `check`
commands. It pins the official SEMAPRAX 0.7.0 Linux executable at source
`eec951eb1cce83e5e0f42edf97cbb5b8f3cffa2c`, Bun 1.2.5, the existing Bend
source pin, and the existing RI13 Linux image by digest. Both tools use the
same extracted Ubuntu glibc 2.39 loader/library path because the release
compiler does not run with the image's Debian glibc 2.36. Exact distribution
URLs, archive digests, and source associations are retained in
[`law16-guest-cache-provisioning-v1.json`](../benchmarks/bend2-law-v1/evidence/law16-guest-cache-provisioning-v1.json).
Provisioning happens before timing and runs no Cargo command.

The default Apple Container `/proc/sys` mount is read-only. This profile
explicitly starts its disposable guest with `--read-only-path NONE`, then
uses guest-local `sync` and `drop_caches=3`. Each pair first reads every
inventoried executable, input, Bend source, and extracted runtime file and
requires `mincore` to observe all of their pages resident. After the reset,
every inventoried page must be nonresident before timing can begin. The
immediately following warm command has the same argv/input bytes, and its
executable and input must have resident pages. `mincore` uses `PROT_NONE`
mappings, so observation does not read the file contents. Reset duration,
guest provisioning duration, checking durations, and raw output are separate.

This establishes **guest file-page-cache** state for the inventoried files.
It does not establish cold macOS storage caches, Rosetta translation caches,
hardware caches, or every shared system-library page. Live Python and system
libraries remain outside the direct residency inventory. Bun's runtime
transpiler disk cache is disabled for both states; `BEND_NO_TELEMETRY=1` is
fixed. The profile does not measure Bend verdict, SMT, Lean, proof synthesis,
or native/GPU throughput. Its observations cannot be combined with historical
macOS executables or relabelled as native Linux hardware measurements.

Capture requires exactly one pilot pair or thirty pairs per lane. Thirty pairs
are refused unless an earlier pilot authenticates under the same profile.
Tool digests, source pins, version output, canonical input bytes, per-file page
counts, warm residency, exact commands, raw output, and container cleanup are
checked offline. Failed commands, missing files, nonzero cold residency,
unobserved warm residency, or widened cache claims fail closed. The original
read-only capability probe remains valid for its original flags; it does not
contradict this explicitly different guest configuration.

The local pilot and thirty-pair capture are retained under
`benchmarks/bend2-law-v1/evidence/law16-guest-cache-{pilot,thirty}-v1/`.
A preceding missing-Bend-effect-file preflight remains a separate nonresult
with zero benchmark samples. These ordinary-check observations alone did not
close AC5; the separate proof/verdict guest and host source-to-SMT capsules
provide the later phase evidence.

Method references: [Apple Container command reference](https://github.com/apple/container/blob/main/docs/command-reference.md),
[Linux drop_caches](https://kernel.org/doc/html/v6.15/admin-guide/sysctl/vm.html#drop-caches),
[mincore](https://man7.org/linux/man-pages/man2/mincore.2.html), and
[Bun runtime transpiler cache](https://bun.sh/docs/runtime/environment-variables).
