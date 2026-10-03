# Source live CLI v1

Audience: private host operators and reviewers of the checked source execution route.

Status: **private host implementation with local recorded-transport integration
tests.** This CLI is a host adapter for the existing checked
[source driver and journal](SOURCE-LIVE-JOURNAL-V2.md). It does not add a second
replay engine, provider fallback, candidate publication path, or model-created
authority.

## Exact operator route

Only the unpublished `semaprax-full` binary admits:

```
semaprax-full source-live run CONFIG CHECKPOINT --opencode ABS --scratch EMPTY_ABS
semaprax-full source-live resume CONFIG CHECKPOINT --opencode ABS --scratch EMPTY_ABS
semaprax-full source-live migrate OLD_CONFIG OLD_CHECKPOINT NEW_CONFIG NEW_CHECKPOINT FUNCTION STEPS --opencode ABS --scratch EMPTY_ABS
semaprax-full source-live offline-repair
semaprax-full source-live repair run REPAIR_CONFIG REPAIR_CHECKPOINT
semaprax-full source-live repair resume REPAIR_CONFIG REPAIR_CHECKPOINT
semaprax-full source-live repair run REPAIR_CONFIG REPAIR_CHECKPOINT --opencode ABS --scratch EMPTY_ABS [--pause-after-settled]
semaprax-full source-live repair resume REPAIR_CONFIG REPAIR_CHECKPOINT --opencode ABS --scratch EMPTY_ABS [--pause-after-settled]
semaprax-full source-live repair-tested run REPAIR_CONFIG REPAIR_CHECKPOINT --opencode ABS --scratch EMPTY_ABS [--pause-after-settled]
semaprax-full source-live repair-tested resume REPAIR_CONFIG REPAIR_CHECKPOINT --opencode ABS --scratch EMPTY_ABS [--pause-after-settled]
```

All operands are absolute except the stable migration function identity and
positive checked-evaluator step limit. `run` requires a new, private checkpoint
directory. `resume` requires its existing latest journal. `migrate` accepts a
committed unpriced v2 Suspend or a committed priced v4 Suspend from the
predecessor directory, and writes a fresh or same-claim destination journal.
The private CLI performs one handoff from predecessor to destination, not a
general migration chain. This CLI version rejects v3 predecessors. The checked
embedding migration API has its own separate A→B→C gate. The executable is the one explicitly chosen OpenCode binary, and
every process attempt uses the fixed
`opencode/muse-spark-1.3-contributor-free` profile without a paid fallback.
Scratch must be a new empty absolute directory for each CLI invocation.

`offline-repair` is a separate fixed, credential-free private demonstration.
It accepts no operands and authenticates only the bundled
`examples/offline-repair-project` Project. It uses the checked Direct Runtime
v2 source loop with two scripted streaming attempts: the first creates a
malformed ephemeral candidate and the second must carry the checked diagnostic
feedback before it can create the bounded `fixture.repair.value` replacement preview. Its
single JSON report contains the candidate digest, source review, semantic
delta, impact summary, model/effect counters and the in-memory source journal.
The command neither writes source nor persists a checkpoint, publishes a
candidate, selects a network provider, accepts a target/path/model operand, or
claims physical recovery. It is local demonstration evidence for the checked
repair path, not a general offline repair interface.

`offline-repair-model-wait` accepts no operands and selects the separate bundled
`examples/offline-repair-model-wait-project`. It uses the same checked repair
runner, grants, candidate validation and feedback guard through the opt-in
interpreter [Source Model Wait v1](SOURCE-MODEL-WAIT-V1.md) route. Its
`semaprax.private-offline-repair-model-wait-demo.v1` report additionally retains
the exact canonical wait evidence bytes, their parsed view and contracted root,
wrapper binding, engine, fuel and v7 source journal. The fixture supplies an
in-memory key/store; this command does not claim physically persisted recovery
or complete owned Agent state suspension. The original `offline-repair`
command and report remain unchanged.

`repair` is the durable, host-selected candidate-preview route for issue #116.
Its canonical JSON configuration selects the retained Project, source Agent,
target declaration, bounded typed effect contract and task; it never supplies
a provider endpoint, credential, source-edit path, test command, publication
grant, or Git authority. Version 1 (`semaprax.source-live-cli.repair-config.v1`)
also contains exactly two bounded scripted fixture documents and accepts no
OpenCode operands. It exists as the credential-free test seam: the second
fixture turn can require the actual first rejection's checked effect-feedback
bytes, proving that the correction did not proceed blind. Version 2
(`semaprax.source-live-cli.repair-config.v2`) has the same fields except
`turns`, and requires the explicit `--opencode ABS --scratch EMPTY_ABS`
operands. `ABS` names the operator-selected executable; `EMPTY_ABS` is a
host-selected, non-symlink scratch directory that is empty or contains only the
exact deny-all policy and private session directory left by an interrupted
run-before-export boundary, or the exact post-settlement pause marker described
below. The configured executable is retained as an exact
bounded byte snapshot, copied to a create-new staged file for each attempt, and
re-authenticated before spawn. Host construction removes a crash-left staged
file only when its held inode and bytes match that snapshot, then clears the
exact owned policy/private state; a completed dispatch removes all of it eagerly
and refuses while preserving any foreign entry the provider leaves. This is
binding inside the documented host boundary, not isolation from a hostile
concurrent process running as the same operating-system principal. The only v2
provider profile is the fixed free `opencode/muse-spark-1.3-contributor-free`
profile. There is no fallback model and no provider selection in source or
configuration. The selected source Agent deployment must already admit that
exact provider/model row; a V2 host binding against a source deployment that
only admits another model fails before checkpoint or provider work.

### Operator interruption point for repair smoke (#323)

`--pause-after-settled` is a one-shot, private operator control for a V2
`repair` or `repair-tested` smoke. It is an operand outside `REPAIR_CONFIG`,
so it changes neither the authenticated repair configuration nor the bound
executable/provider identity. It is admitted only after explicit
`--opencode ABS --scratch EMPTY_ABS`; there is no environment-variable control.

After the physical checkpoint host has synced, renamed, and acknowledged a
generation whose final causal entry is `attempt_settled`, the wrapper writes
one canonical marker through that already selected scratch host:

```
EMPTY_ABS/.semaprax-repair-post-settled-pause.json
```

Its schema is `semaprax.source-live-cli.repair-post-settled-pause.v1`; it
contains the authenticated journal `invocation`, `checkpoint_generation`, `turn`,
`attempt`, and retained `response_digest`. The marker is synced before the process parks. It carries
no raw response, credentials, candidate, effect result, capability, or new
authority. A marker write or acknowledgement failure stops the invocation
before decode, authorization, an effect, or another stage.

For the local interruption/recovery smoke, an operator runs the explicit V2
command with this flag, observes that exact marker in the configured scratch
directory, and sends `SIGKILL` while the process is parked. The controller
then resumes with the same `REPAIR_CONFIG`, checkpoint directory, executable,
and scratch directory, but omits `--pause-after-settled`. The repair resume
first rebinds and validates the retained journal; only then does the OpenCode
scratch host remove a marker whose canonical bytes exactly match that recovered
invocation, generation, turn, attempt, and response digest. A foreign,
malformed, or mismatched marker remains in place and the resume refuses.
Normal source-journal recovery then replays the settled response without
redispatching that provider attempt. A following effect or later provider
attempt is performed only by the resumed checked execution.

The marker is a timing observation tied to the held local scratch directory.
It is not evidence of provider delivery, exactly-once behavior, physical
power-loss recovery, hosted support, or production readiness.

The ordinary V2 CLI still has **no candidate-test authority**. An embedding host
may instead call the public `source_live_cli::run_repair_with_candidate_test`
entry with one opaque candidate-test capability together
with a bounded observer. The observer receives the immutable exact
`ProjectCandidate` (including its retained candidate source material) together
with the candidate revision, base Project revision, source revision, and
host-selected capability identity; it receives no candidate mutator, command,
environment, process handle, publication grant, or Git authority. It returns one canonical
`semaprax.source-live-cli.candidate-test-observation.v1` data document with a
bounded `passed`, `failed`, or `refused` outcome. The host rejects malformed,
oversized, foreign-candidate, stale-base, stale-source, or wrong-capability
documents. There is deliberately no JSON operand that selects a test runner.

`repair-tested` is the one separate private CLI startup profile that supplies
that embedding boundary itself. It accepts the same V2 OpenCode operands as
`repair`, fixes capability identity to `semaprax.source-live-cli.repair-tested.v1`,
and fixes the reference-interpreter policy to 100,000 steps, 65,536 execution
bytes, and 262,144 report bytes. It calls `ProjectCandidate::execute_tests`
against the immutable candidate and records only the bound pass/fail/refused
observation plus the canonical candidate-test report digest. It accepts neither
a test command nor policy/configuration overrides. Scripted V1 repair is
refused before checkpoint creation, so this profile cannot turn fixture input
into live test authority.

When an embedding observes a candidate, it converts the canonical outcome to a
deterministic typed `i64` feedback code bound to the complete observation. The
existing typed-effect/journal boundary settles that result before any later
proposal request. A failed candidate test therefore supplies real bounded
feedback, not a fixture-only annotation. The capability identity is also
bound into the V2 model/journal binding: resuming with a different selected
capability refuses before provider or test-observer dispatch. A terminal replay
dispatches neither and does not fabricate a fresh observation. `refused` is an
honest host observation, not a pass or an authorization to retry outside the
checked loop. The callback returns fixed-size bounded observation storage; it
cannot hand validation an unbounded allocation.

V2 reads `deadline_millis` as Unix-epoch milliseconds, so the deadline survives
restarts. At host construction, the process timeout is the smaller of 30 seconds
and the time remaining. The runtime checks the same absolute deadline around
every attempt and settlement. V1 retains its fixed-zero fixture clock to keep
committed credential-free examples deterministic.

V2 adapts that same bounded `ProcessOpenCodeRunner` through the Provider Adapter
SDK boundary used by Direct Runtime v2. The OpenCode process receives the
runtime's compiler-derived canonical source-proposal request and schema, runs
with the deny-all tool policy, and must pass the existing run/export receipt
validation before its raw text reaches the compiler proposal decoder. A fresh
adapter instance is created only after the checked journal acknowledges an
attempt intent. Both V1 and V2 resume first rebind and recover their journal;
terminal recovery occurs before V1 fixture-only target lookup or diagnostic
derivation, and before V2 host, candidate, or typed-effect construction. A
terminal checkpoint dispatches neither provider nor typed effect again, a
changed source or executable/scratch host binding refuses, and an unresolved acknowledged delivery remains
uncertain rather than being sent again. These facts make interruption/resume
observable in the journal, not an exactly-once claim about an external service.
The V1 scripted fixture is a deterministic terminal-receipt seam only; this
document does not claim nonterminal V1 provider replay or exactly-once fixture
work after a crash. V2 recovery retains the journal's explicit
uncertainty/idempotency outcome for an acknowledged but unsettled provider
attempt.

Candidate-test observation has the same fail-closed crash boundary. The host
observer runs only after a durable effect intent exists and before its
`EffectObserved` settlement. A returned observer error is settled as a refused
handler result; if the process stops before either settlement, the journal
remains uncertain. Resume does not redispatch the observer in either case. The
observer's bounded `detail` field is included in the fresh public CLI receipt;
embedding hosts must therefore return only review-safe, non-secret detail.

The repair receipt is always source-immutable and publication-authority-free.
V1 retains the frozen `semaprax.source-live-cli.repair-receipt.v1` projection:
candidate digest, source review, semantic delta, impact summary, rejected-
candidate count, checkpoint generation and dispatch counters. V2 emits the
additive `semaprax.source-live-cli.repair-receipt.v2` projection. On the
invocation that produces a candidate it additionally carries `journal_binding`
(invocation, chain, generation) and `analysis.coverage` / `analysis.blind_spots`
for those review artifacts. It also prints `selected_profile` with the exact
config schema, provider/model, adapter identity/version and provider profile,
plus `checked_prerequisites` with the ProgramRoot, checked source revision,
compiler-derived proposal-schema digest and deployment binding. These are the
same identities used to bind the durable invocation, not independently authored
receipt labels; terminal replay must reproduce them exactly. They are review
evidence only and mint no provider, filesystem, test or publication authority.
`model_attempts` is the bounded, replayable per-attempt projection of that same
validated, binding-checked retained source journal: it reports intent and settlement stages, request /
response digests and byte counts, decode/refusal outcome, and provider-reported
usage only when the adapter actually recorded it. Missing usage remains `null`;
the receipt does not manufacture zero tokens, cost, timing or delivery. Terminal
replay recomputes the identical projection without starting the provider.
The hash chain supplies integrity and causal shape, not freshness or external
authentication; a storage controller can replay an older same-binding journal,
so consumers must not treat this receipt as proof that it is the newest state.
V2's `candidate_test_execution.status: "not_run"` is deliberate when the
ordinary CLI route supplies no capability. A
capability-bearing embedding can instead report the bounded canonical observation
and its feedback code in the fresh receipt. A terminal replay carries the
previously settled status and typed feedback with `replayed: true`; the full
host observation document is not retained in the current journal, and no new
observation is fabricated. Neither receipt is a cost proof,
provider-delivery proof, source/Git mutation, or approval to publish the
candidate.

Priced migration requires both predecessor and destination config v2 pricing
with exactly matching work unit, currency, minor-unit exponent and integer
rate. The destination money ceiling may narrow but cannot fall below carried
reservations. The migrated v4 journal retains unknown exposure, observed
charges, overage and the global money ordinal; it never reconstructs them from
the CLI receipt. Unpriced-to-priced and priced-to-unpriced conversion are
refused rather than silently shedding exposure or treating it as zero-priced.

A crash after fresh directory creation but before its first journal ACK can
leave an empty directory. Both `run` (existing directory) and `resume` (no
latest journal) refuse it. An operator may use a different new directory
only after independently establishing that no journal or provider work began;
the CLI does not infer that fact from an empty directory.

`CONFIG` is one canonical JSON object, at most 8192 bytes. Version 1 has exactly these
keys: `schema` (`semaprax.source-live-cli.config.v1`), `manifest`,
`source_path`, `agent_id`, `step_id`, `task_path`, `task_budget`, `read_path`,
`deadline_millis`, `ceiling`, `reservation_units`, `max_iterations`,
`max_stages`, `max_steps_per_stage`, `max_total_steps`, and `response_limit`.
Unknown, duplicate, alternate-encoding, and negative or over-capacity fields
are rejected before journal or provider work. JSON must use exact compact
sorted-key serialization; a single final line feed is accepted. `manifest`,
`task_path`, and `read_path` are absolute host selections; `source_path` is a
relative Project `.spx` selector without `..`. The task and observation files
are bounded to 65,536 bytes each. The observation file is an explicit fixed
read snapshot, returned by the one injected `AgentReadOperation`; it is not a
shell, test runner, candidate editor, or semantic validation tool.

Version 2 is an additive priced route. It uses schema
`semaprax.source-live-cli.config.v2`, retains every v1 key, and requires one
additional exact `pricing` object with `currency` (three uppercase ASCII
letters), `minor_unit_exponent` (`0..=9`), positive integer
`price_per_work_unit_minor`, and nonnegative integer `money_ceiling_minor`.
These are an operator quote in bound integer minor units for the fixed source
work unit, never a provider price lookup, currency conversion, float-cost
parser, or invoice. A v1 document with `pricing`, or a v2 document with a
missing, extra, malformed, zero-price, negative, or noncanonical pricing
field, is refused before checkpoint or provider activity; it cannot downgrade
to the unpriced route.

Version 3 is an additive priced-I/O route. It uses schema
`semaprax.source-live-cli.config.v3`, retains every v2 key and requires one
additional exact `io_limits` object with nonnegative integer
`max_request_bytes`, `max_total_request_bytes`, and
`max_total_response_bytes`. The per-attempt request field is capped at 65,536
bytes and may be zero; the cumulative fields are `u64` ceilings. V3 does not
widen v1/v2 keys, and a missing, extra, malformed, negative or noncanonical
I/O field is refused before a checkpoint or provider call. Its exact
reservation, recovery and migration semantics are in
[Source Live I/O v5](SOURCE-LIVE-IO-V5.md).

The host authenticates the retained Project, selects and checks its Agent
role closure, derives its actual `ProgramRoot`, and derives the proposal
grammar from the compiled source. The read snapshot bytes and fixed model are
hashed into the deployment binding; changing the snapshot on resume changes
the invocation identity and fails journal recovery. The task bytes, budget,
source revision, ProgramRoot, schema, fixed charge, bounds, clock and deadline
are bound by the existing `SourceInvocationBinding`. Neither submitted model
text nor the checkpoint document supplies those host facts. The unit is
`fixed_model_attempt_units.v1`, charged once per acknowledged attempt intent;
provider-reported counters remain optional observations, never billing proof.
For v2, the paired price reservation is also acknowledged before dispatch.
Current OpenCode cost JSON has no bound currency/minor-unit representation, so
the host records explicit `Unknown` charge evidence instead of converting a
float or manufacturing zero cost.

## Latest store, clock and migration claim

The Unix host holds the checkpoint directory by file descriptor and a
nonblocking exclusive advisory lock. It refuses symlinked/nonphysical path
components and non-private directories. A read preflights regular-file type
and byte limit on the opened descriptor; a FIFO or replaced symlink cannot
turn the bounded read into an unbounded wait. Each canonical journal
generation is written to a new file, synced, renamed over the latest document
through the held directory, then the directory is synced before the store
ACK. A failed or ambiguous commit poisons the writer. Recovery loads the
latest authoritative document under the same exclusive lock, validates the
exact independently derived source binding, and restores the existing
cumulative ledger. A store replacement by its owner or a valid rollback of
the latest document cannot be authenticated by hashes alone.

The CLI uses Unix epoch milliseconds as one restart-stable clock domain,
with origin zero for a fresh v2 run and an absolute `deadline_millis` supplied
in CONFIG. A v3 migration's origin is the authenticated predecessor latest
checkpoint's last checked clock floor; repeating the same handoff derives the
same origin from that predecessor terminal. Recovery does not reset that
deadline. A regressed or expired continuation
refuses; an already committed terminal is a read-only receipt and can be
retrieved after expiry with zero model/effect dispatches. The OpenCode
process timeout is no greater than 30 seconds or the invocation time
remaining when this CLI traversal begins. The source clock is checked again
before each attempt and after each settlement; a later child may cross the
absolute deadline, in which case its result is withheld and its committed
reservation remains charged.

Before a v2→v3 migration can evaluate or run the destination, the predecessor
store persists a single handoff claim under its held lock. The claim binds the
checked handoff digest, destination directory and new invocation. The same
destination/handoff may reopen its latest journal; another destination is
refused. A crash after the claim but before destination settlement can leave
the handoff unavailable pending explicit operator reconciliation. This is a
cooperating-CLI single-destination rule, not a distributed transaction or
proof against hostile owner rollback. The destination uses
`prepare_source_live_migration` and the same source journal/driver; checked
migration fuel is acknowledged before the pure evaluator, and the migrated
State is schema-checked before first Observe. No Initialize is repeated.

## Output and scope

A completed unpriced run returns the v1 bounded JSON receipt with terminal status,
invocation, generation, chain, acknowledged model units and stage fuel, and
this traversal's model/effect dispatch counts. Every receipt version also
carries `iterative_evidence`: the compiled reducer's own
`semaprax.agent-iterative-evidence.v2` document (policy, invocation digest,
status, iteration/effect counts, per-stage role/function/outcome/step rows,
authorization bindings, and a terminal-value digest) when this traversal
dispatched fresh work, or `null` on a pure terminal-checkpoint replay that
redispatched nothing. It does not include raw model text, credentials,
provider stderr, or a publication grant. A failure reports its selected
status and last acknowledged counters; the journal remains the reviewable
causal artifact. The CLI never rewrites authoritative `.spx` source or Git
state. The separate `source-live repair run|resume` route described above now
provides the host-selected failed-check observation seam, semantic candidate
preview (source diff, semantic impact and blind spots), checked repair feedback,
and durable journal binding for issue #116. It still provides no approval-bound
publication. The general `run`/`resume`/`migrate` route remains a domain-agnostic
Agent-lifecycle host adapter and is not itself a repair command. No
operator-approved live repair run, hosted CI, durable power-loss, or exactly-once
physical delivery claim follows from the local injected tests.

A completed priced run returns
`semaprax.source-live-cli.receipt.v2` with the same top-level status,
invocation, generation, chain, `committed_model_units`,
`committed_stage_fuel`, `model_dispatches`, and `effect_dispatches`, plus a
`money` object containing exactly `currency`, `minor_unit_exponent`,
`reserved_minor`, `observed_charge_minor`,
`unknown_charge_reservation_minor`, `observed_over_reservation_minor`, and
`remaining_admission_minor`. These values are replay-derived bound
reservations and provider observations. They are neither a reconciled invoice
nor a refund or payment authorization. A priced failure reports the same
acknowledged monetary counters in its error detail. Terminal recovery returns
the bound receipt without another provider call.

The focused `source_live_cli` tests exercise local recorded execution. The
retained-Project fixture sends a recorded OpenCode run/export through the
actual source adapter, then checks terminal resume and changed read, task,
or policy refusal with zero further calls. A second fixture executes checked
Suspend, pure StateB migration, destination completion, terminal recovery,
and competing-destination claim refusal. Store tests cover exclusive locks,
held-directory rename, poisoned writes, symlink/FIFO input refusal, and an
empty fresh directory that neither mode silently resumes. These are local
fixture results, not a live provider or power-loss test. The repair-specific
local fixture additionally injects a bounded candidate-test observer: it proves
failed-test feedback reaches a later recorded provider request,
malformed/oversized/withheld observation data is refused, terminal resume does
not redispatch either provider or observer, and capability-binding drift fails
closed. Its V2 hostile-recovery corpus additionally covers malformed settled
response hex, unknown settled-response fields, a settled-response sequence
mismatch, an unknown envelope field, stale checked source, and changed task
binding. The V1 terminal-replay corpus separately replaces the configured
candidate target with a valid-but-missing declaration and alters the fixture
diagnostic shape; recovery still returns its retained receipt because neither
fixture-only action is constructed. These mutations have exact local stable
refusal or replay assertions before a provider, effect, or candidate-test
handler can be constructed. The implementation also maps its
other closed journal recovery classes (clock, capacity, uncertain delivery, or
unavailable store) to diagnostics, but this corpus does not claim a hostile
fixture for each of them. The clean terminal V2 replay remains an exact
positive control, so rejection alone cannot satisfy the corpus. These labels
are diagnostic-only and do not authenticate freshness, grant provider
authority, or approve a candidate. This is local injected-host evidence only;
it is neither a real
test-command execution claim nor the operator-approved live-provider smoke
required by issue #116. The #323 interruption regression uses the same
credential-free recorded OpenCode runner and actual physical checkpoint host:
it observes the persisted scratch marker while the latest journal ends at
`attempt_settled`, verifies that no effect ran, then resumes without the pause
operand and verifies that only the later provider attempt dispatches. It is
local timing and recovery evidence for the host seam, not real-provider
evidence.

## Additive native Claude repair profile

[Claude print repair v1](CLAUDE-PRINT-REPAIR-V1.md) defines config/receipt V3
and the explicit `--claude` operand. It preserves the frozen V1/V2 meanings
and shares the existing checked candidate, journal and replay routes.
