# Bend 2 law benchmark (v1)

This is the reproducible benchmark harness for issue #392. It records a
comparison only when a local command file pins both checked-out source trees,
their exact commits, tools, and commands. The committed manifest pins Bend 2
at `947db722640c86247849343657bf2f7ef01cb7f1` and forces
`BEND_NO_TELEMETRY=1` for every execution.

Run an admitted 30-repetition trial on a quiet host with a local command file:

```sh
python3 benchmarks/bend2-law-v1/run.py \
  --commands /secure/local/bend2-commands.json \
  --output /secure/local/bend2-result.json
```

The command file follows [`commands.example.json`](commands.example.json). It
is deliberately not committed because executable paths, the exact SEMAPRAX
commit, hardware/OS, toolchain and dependency identities, backend, flags, and
fixture digests are host inputs. The runner refuses a missing or drifted Git
identity, records every raw warm sample plus output digests, labels smaller
runs as pilots, and never emits a winner field.

For each of the six cells, it separately executes `bend_normal`,
`bend_verdict`, `semaprax_smt`, `semaprax_lean`, and `semaprax_runtime`.
Every path must reject the cell's law-gaming attack before its timing is
accepted. Thus an empty sort cannot pass a sorting cell and a no-op transfer
cannot pass a balance cell. Numeric domains are declared per cell; a command
file must not substitute Bend `Nat` semantics for the declared checked `u32`
domain.

The scalar-contract cell is a matched microcell: it uses total Boolean
negation, where both languages have the same two-value domain. It remains
unexecuted until a pinned Bend toolchain and local fixture drivers are supplied.
Every command receives its canonical fixture path through `{fixture}` and the
requested `success` or attack case through `{case}`. This prevents a local
driver from timing an unbound substitute. The other five cells retain their
checked-`u32` domain and remain unavailable until SEMAPRAX has an equal `u32`
surface; they cannot be replaced with an `i32` benchmark.

The local command file also declares the exact numeric domains each execution
path can run. The runner records that declaration separately for ordinary
Bend, Bend `--verdict`, SEMAPRAX SMT, external Lean, and SEMAPRAX runtime.
It refuses a cell before invoking its command when that path lacks the cell's
exact domain: an `i32 checked` declaration cannot run a `u32 checked` cell.
The example keeps the unprovisioned SMT and Lean routes empty and only marks
the SEMAPRAX runtime route as Boolean-capable, so these declarations are not
claims that an external tool executed.

`fixtures/` is the committed language-neutral source-input corpus: every cell
has one accepted witness and one rejected attack witness, and their digests
are bound into the result. The current SEMAPRAX scalar surface has no `u32`
type (it admits `i32`, `i64`, and `u8`), while Bend uses `U32`; an `i32`
substitute would narrow the domain and is therefore not presented as an equal
source fixture. The pinned Bend source used for the Boolean smoke route does
not supply matched checked-`u32` source/proof files or a runnable six-cell
command configuration; those remain explicitly unavailable.

Before executing any path, the runner validates the canonical equal-spec
controls in the fixture corpus. The scalar cell requires exact Boolean
negation; balance requires both conservation and the requested debit/credit;
sorting requires both sortedness and permutation with multiplicity; refactoring
requires the same observed result; the agent edit retains its declared law; and
the incremental cell records rechecking `core` while reusing only `api`.
The corresponding mutants are rejected as weakened postconditions, no-op
transfer, empty sort, dropped refactor law, removed agent law, and stale cache
reuse. These are fixture checks, not tool execution evidence. All five
checked-`u32` cells remain unavailable until both languages have matched
executable source and proof routes; this validation never substitutes `i32`.

The harness records local evidence only. It does not provision tools, clone
repositories, generate source fixtures, publish results, or make a
superiority claim. Unimplemented fixture/tool combinations remain
`unavailable` in the result rather than a favorable score.

## Transparent report

Render a reviewable report from a completed, failed, or unavailable benchmark
receipt with:

```sh
python3 benchmarks/bend2-law-v1/report.py \
  --benchmark-result /secure/local/bend2-result.json \
  --output /secure/local/bend2-report.json
```

The report binds the exact input receipt, manifest, command, and fixture
digests; retains the raw warm samples and their per-path p50, p95, mean,
range, and population standard deviation; carries every law-gaming result;
and lists the local execution trust boundary. It never merges the ordinary
Bend checker, Bend verdict kernel, SEMAPRAX SMT, external Lean, or SEMAPRAX
runtime into a score. Failed, timed-out, and unavailable routes remain visible
nonresults, and the report cannot emit a winner or superiority claim.

The deterministic no-tool input receipt is useful for review before a local
toolchain run:

```sh
python3 benchmarks/bend2-law-v1/fixture_receipt.py \
  --output /tmp/bend2-fixture-receipt.json
```

## Agent-trial preregistration

Before a live agent experiment, create a local configuration from
[`agent-trial-config.example.json`](agent-trial-config.example.json), replacing
its placeholder configuration digest with the SHA-256 of the reviewed model,
tool policy, and prompt configuration. The plan requires a fixed token/cost
budget and at least ten trials per language and cell:

```sh
python3 benchmarks/bend2-law-v1/agent_trial_plan.py \
  --config /secure/local/law16-agent-config.json \
  --output /secure/local/law16-agent-plan.json
```

This writes a preregistration only; it never invokes an agent or reports
token/cost observations. It binds the manifest and every fixture digest, the
model/tool/budget configuration, laws, and seeded law-gaming attacks. The exact
Boolean scalar cell preregisters 10 independent trials for each language. Each
trial requires acceptance of its success witness, rejection of every seeded
attack, existing telemetry token/cost events, and separate proof-synthesis,
law-kernel-check, and compile-or-runtime observations. These remain required
observations, not invented measurements.

The admitted local runner is `codex_agent_trial.py`. Its configuration fixes
the `openai-codex-cli` model name and reviewed configuration digest, a token
and wall-time budget, and `codex exec --json` with `--ephemeral`,
`--ignore-user-config`, and the `read-only` sandbox. Every run starts in a new
empty directory and has no repository workspace or additional writable
directory. It retains the raw JSONL event stream, stderr, and Codex version
bytes under a new operator-selected evidence directory:

```sh
python3 benchmarks/bend2-law-v1/codex_agent_trial.py \
  --plan /secure/local/law16-agent-plan.json \
  --trial scalar-contract-bug-v1:bend2:1 \
  --evidence-dir /secure/local/law16-codex-trial-001 \
  --output /secure/local/law16-codex-trial-001.json
```

The runner embeds the Boolean law, accepted witness, seeded law-gaming source,
and required JSON response schema in the prompt. It asks the model to return a
complete repaired source (including Bend's proof body) and a claim that the
embedded attack is rejected. It retains the exact JSON response, final-source
bytes, and attack claim alongside raw Codex events. The record derives only the
input, cached-input, and output token counters in `turn.completed` and rejects
an absent or over-budget counter. Codex JSON events do not provide a monetary
charge, and the turn neither independently executes a proof kernel nor a
runtime. `edit_artifacts_captured` is therefore raw edit provenance, never a
completed LAW-16 repair or a cost observation.

The reviewed SEMAPRAX scalar profile does not support the checked-`u32` cells.
They remain explicitly `unsupported`, so the current mixed plan is
`partially_preregistered` and exits nonzero. It cannot be relabeled as a
successful matched trial or use an `i32` substitute.

## Pinned Bend Boolean smoke fixture

## Bounded balance-transfer preflight

`bounded_balance_preflight.py` establishes the next feasible matched witness
without relabelling it as the unavailable checked-`u32` cell. It fixes debit,
credit, and amount to `20`, `30`, and `7`; all intermediate and result values
are in `0..100`, where Bend `U32` and SEMAPRAX `i64` agree. Bend checks and
verifies its three literal laws, while the project-bound installed-Z3 route
separately discharges the scalar debit, credit, and total postconditions. The
seeded no-debit source must fail Bend on both routes and Z3 for debit and total;
its unchanged credit clause is expected to remain provable.

```sh
python3 benchmarks/bend2-law-v1/bounded_balance_preflight.py \
  --bend-root /tmp/bend2-law-source --bun /absolute/path/to/bun \
  --semaprax /absolute/path/to/semaprax --z3 /absolute/path/to/z3 \
  --artifacts /secure/local/law16-bounded-balance-artifacts \
  --output /secure/local/law16-bounded-balance.json
```

The project-proof command refuses projects below `/tmp`; choose a real local
directory such as `/secure/local` for the retained artifacts. This is a fixed
bounded witness with local source proof. It does not establish general transfer
semantics, checked-`u32` support, lowering or execution proof, an agent trial,
or a language comparison.

## Bounded balance agent-trial observation

[`evidence/law16-bounded-balance-v2/`](evidence/law16-bounded-balance-v2/)
retains ten fixed-budget, read-only Luna pairs and their raw events, returned
sources, Bend ordinary/verdict replays, installed-Z3 project proof receipts,
and the earlier v1 prompt failure. The v2 aggregate is an observation of ten
independent completed pairs: nine Bend candidates passed both routes and one
eligible Bend candidate failed its proof syntax; all ten SEMAPRAX candidates
had debit, credit, and total scalar postconditions discharged by local pinned
Z3. The fixed no-debit attack was rejected for debit and total in every Z3
replay; its unchanged credit clause remained provable. Every Bend attack was
rejected on ordinary and verdict routes.

Re-authenticate the bounded files and derived counts without invoking a tool:

```sh
python3 benchmarks/bend2-law-v1/law16_bounded_balance_capsule.py \
  --capsule benchmarks/bend2-law-v1/evidence/law16-bounded-balance-v2 \
  --output /tmp/law16-bounded-balance-review.json
```

The v1 Bend pilot is ineligible because its prompt permitted a computed record
projection that the pinned parser refuses. The v2 ordinal-4 syntax failure is
an eligible failed trial and remains in the denominator. These are local pinned
tool observations: source proof does not prove lowering or execution, retained
Bend verdict output is not an independent proof system, and Codex JSON has no
monetary billing event. This cell is a fixed `0..100` witness, so it does not
admit the wider checked-`u32`, list, refactor, or incremental LAW16 cells, and
it does not establish a general transfer theorem or complete LAW16.

The following local-only provisioning sequence fetches the pinned Bend source
without building it or placing it in this repository:

```sh
git clone --filter=blob:none --no-checkout https://github.com/bendlang/bend.git /tmp/bend2-law-source
git -C /tmp/bend2-law-source fetch --depth=1 origin 947db722640c86247849343657bf2f7ef01cb7f1
git -C /tmp/bend2-law-source checkout --detach 947db722640c86247849343657bf2f7ef01cb7f1
```

`fixtures/bend-two-value-boolean-v1.bend` matches Bend `False{}` and `True{}`
and prints `0` then `1`. Its driver separately invokes ordinary Bend checking
and `--verdict` with `BEND_NO_TELEMETRY=1`:

```sh
python3 benchmarks/bend2-law-v1/bend_boolean_driver.py \
  --bend-root /tmp/bend2-law-source \
  --bun /absolute/path/to/bun \
  --output /tmp/bend-two-value-boolean.json
```

This is a pinned upstream Boolean smoke fixture. It is outside the six
checked-`u32` cells and records no comparison, timing result, or winner.

## Agent-trial raw telemetry capture

After an agent run, retain its existing telemetry export and its raw case
transcripts outside the repository. Do not add a Boolean value such as
`passed` to make the capture succeed. Instead, create a machine-readable
`semaprax.bend2-law-benchmark.agent-telemetry-export.v2` document bound to the
exact preregistration digest. Each trial supplies its transcript digest, the
existing `token_usage` and `cost_usage` telemetry events (with distinct event
identities), completed wall-time observations for proof synthesis, law-kernel
checking, and compilation or runtime, plus a distinct SHA-256 for each raw
phase-measurement artifact. The capture retains each phase's wall time and
measurement digest separately; a phase cannot reuse another phase's raw
artifact. It also retains digest-bound observations for every success witness
and seeded attack.

The capture command validates that structure against the fixed plan:

```sh
python3 benchmarks/bend2-law-v1/agent_trial_capture.py \
  --plan /secure/local/law16-agent-plan.json \
  --raw-export /secure/local/law16-agent-raw-export.json \
  --artifact-root /secure/local/law16-agent-raw-artifacts \
  --output /secure/local/law16-agent-capture.json
```

It emits `completed` only when the raw export covers every preregistered trial
for both languages. A partial export remains `partial` and exits nonzero;
accepted attacks, omitted phase timings, reused telemetry events, unknown
trial IDs, reused phase-measurement artifacts, and a changed plan digest are
refused. The output retains raw-export
and transcript digests plus the telemetry values and separate phase durations.
It does not authenticate an external telemetry provider, execute an agent, or
state a comparison result.

The v2 export replaces every digest-only reference with a `{path, sha256,
bytes}` reference beneath `--artifact-root`. Capture reads each referenced
regular file, checks the byte count and SHA-256 itself, enforces a 32 MiB
per-artifact and 8 MiB export bound, and rejects absolute paths, `.`/`..`, and
any symbolic link component. A v1 export with bare digest strings is refused:
a claimed digest without its retained raw file is not evidence.

## Boolean final-source pair review

`boolean_pair_acceptance.py` reviews one preregistered Boolean ordinal after a
trial operator retains its final source and verification files. Its
`semaprax.bend2-law-benchmark.boolean-final-artifacts.v2` input binds the exact
plan digest, both lane trial IDs, each final source, and a raw verification
file by relative path, byte count, and SHA-256. The evaluator re-hashes those
bounded regular files below a supplied artifact root and rejects links or
source bytes that differ from the committed Boolean success fixtures.

The Bend side requires a retained verdict stream containing `ALL PROOFS CHECK`.
The evaluator also binds the retained model response to the exact final source,
the exact seeded attack source, and the model's `reject` claim. It labels that
as static control distinction rather than tool rejection. The SEMAPRAX side requires only its exact runtime witness (`0`), then labels
its formal proof phase unavailable because this evaluator does not execute the
separate project-bound installed-Z3 proof route. Cost is likewise unavailable
unless a separately retained provider billing record is introduced. The result
is named `source_pair_authenticated`, never `accepted` or a successful law
repair: it does not execute a command, interpret an exit code, establish agent
authorship, or turn the runtime witness into a proof.

## Ordinal-2 SEMAPRAX two-input runtime evidence

`semaprax_boolean_runtime_acceptance.py` reviews one SEMAPRAX ordinal whose
agent output differs from the canonical fixture bytes but retains the pinned
module name, `app.negate` and `app.main` identities, and
`ensures result == !value`. It receives raw candidate and exact seeded-attack
source files, the bound model response, and a separate retained CLI receipt,
stdout, and stderr for each source. Each receipt binds the source digest,
three-element `semaprax run` command, executable digest, and pinned SEMAPRAX
commit. The candidate must have exit zero with exactly `0`; the exact seeded
attack must have a nonzero exit, empty stdout, and a `language status`
diagnostic. The evaluator re-hashes every regular artifact and refuses links;
it does not run a compiler itself.

```sh
python3 benchmarks/bend2-law-v1/semaprax_boolean_runtime_acceptance.py \
  --plan /secure/local/law16-agent-plan.json \
  --evidence /secure/local/law16-two-input-evidence.json \
  --artifact-root /secure/local/law16-two-input-artifacts \
  --expected-semaprax-commit <pinned-commit> \
  --output /secure/local/law16-two-input-acceptance.json
```

On 4 October 2026, ordinal 2 of the preregistered Boolean SEMAPRAX lane was
evaluated against `/tmp/semaprax-9a9db7a81`, attributed to
`9a9db7a8117ac8d292b24ffd5671ec3333272290` with executable SHA-256
`cc9dd3ca99a74dd973cbfb904621e27b801d8ddea6065873d24c14de9dee1d89`.
The candidate source digest was
`8e0323e63a4eb7fb0207766d5855ab88a1b7e979c0866b91795ed15b77546b82`;
the exact seeded attack digest was
`9f6aab36ed9d03ede8e3cb1f79407e77f8fe5395434a6b8579acc59e7270ba2b`.
The retained candidate route exited zero and emitted `0`; the attack route
exited one and emitted the contract `language status` diagnostic. The receipt
is `/tmp/law16-codex-edit-pilot.CBbG5U/ordinal-2-runtime-evidence/acceptance.json`
with status `two_input_runtime_authenticated`.

This authenticates only this local compiler/runtime two-input observation.
It does not establish a full law repair, a formal proof, a provider cost, an
agent-authorship result, a matched Bend result, current-head evidence, or a
checked-`u32` result. The formal proof and cost fields in the receipt are both
`unavailable`.

The corresponding Bend lane uses `bend_boolean_runtime_acceptance.py`. It
requires retained ordinary-check and `--verdict` receipts for the candidate,
and rejection receipts with `SOME PROOFS FAIL` for the exact seeded attack on
both routes. Its `two_input_bend_runtime_authenticated` result records an
observed raw verdict marker, not independently replayed proof. It also labels
the cell `bool_exact_only`: the Boolean microcell does not supply checked-`u32`
evidence.

On 4 October 2026, Bend ordinal 2 used source commit
`947db722640c86247849343657bf2f7ef01cb7f1` and Bun SHA-256
`abe991b29c5151ab11b5344be65dee3a675b0a4a55b8fc493e3cfbe256e61781`.
The model source digest was
`205c46ac3bcb0620e0c11d55732614c83026d74de9d023050e4cc45b9d46da46`.
Raw candidate and attack receipts are under
`/tmp/law16-codex-edit-pilot.CBbG5U/ordinal-2-bend-runtime-evidence/`; its
`acceptance.json` has status `two_input_bend_runtime_authenticated`.

All ten ordinals of the fixed 20,000-token Boolean plan were executed
sequentially on 4 October 2026. The local machine-readable summary is
[`law16-ten-boolean-local-summary.json`](law16-ten-boolean-local-summary.json):
it lists the SHA-256 and local path of all 20 retained agent records and all
20 language-specific evaluator records. Each ordinal has one
`two_input_runtime_authenticated` SEMAPRAX receipt and one
`two_input_bend_runtime_authenticated` Bend receipt. The summary is a local,
unhosted evidence capsule and carries no authority by itself.

These ten Boolean pairs do not close #392. The five checked-`u32` cells remain
unsupported by the reviewed SEMAPRAX scalar profile; Codex JSON events provide
no monetary cost observation; and the retained tool identities are local
pinned observations, not current-head evidence. The retained Bend verdict
markers have not been independently replayed by a separate proof system. The
batch therefore supplies no full LAW-16 repair, checked-`u32`, agent-authorship,
timing, superiority, or cross-language comparison result.

## Independent replay and committed raw capsule

[`evidence/law16-boolean-v1/`](evidence/law16-boolean-v1/) contains the
bounded raw evidence for all 20 read-only agent turns: JSONL events, model
responses, returned sources, attack claims, Codex versions, stderr, and a
SHA-256/byte-count manifest. It also contains a fresh independent replay of
every retained Bend candidate and exact attack through ordinary Bend checking
and `--verdict`, and every SEMAPRAX candidate and exact attack through
`semaprax check --json`. It also retains the project wrappers, exact command
receipts, stdout, stderr, and solver/tool hashes from a project-bound installed
Z3 replay for all ten SEMAPRAX candidates and exact attacks. The capsule is
copied without transforming the raw content after a credential-marker scan; it
is explicitly local and unhosted.

Review the capsule without invoking a provider, Cargo, Bend, or SEMAPRAX:

```sh
python3 benchmarks/bend2-law-v1/law16_boolean_capsule.py \
  --capsule benchmarks/bend2-law-v1/evidence/law16-boolean-v1 \
  --output /tmp/law16-boolean-capsule-review.json
```

The independent replay confirms ten Bend candidate successes and ten exact
attack rejections on both Bend routes. It also shows the available SEMAPRAX
`check` route exits zero for all ten candidates **and** all ten exact attacks.
Consequently, `check` is retained as a compiler semantic-check observation and
is never presented as proof. Separately, the retained
`project-proof-check --tool z3` receipts discharge `app.negate`'s sole
postcondition for all ten candidates and reject all ten exact attacks. That
route is an installed-Z3 source proof for the exact retained project revision:
it proves neither lowering nor execution, and its call-free SMT subset does not
cover `app.main`, which invokes `app.negate`. An attack's nonzero route exit is
recorded as rejection, not as a separately retained solver counterexample.

The next scalar campaign must specify a call-free contract in an identical Bend
numeric domain and state overflow semantics explicitly. The five checked-`u32`
cells remain unavailable until SEMAPRAX admits that same domain; an `i32`
substitution would not be a matched task. Provider monetary cost remains a
separate unavailable observation and cannot be inferred from token counts.

The local pinned Bend Boolean smoke route was executed on 4 October 2026 with
Bun 1.2.5 and `BEND_NO_TELEMETRY=1`. Its receipt is
`/tmp/bend-two-value-boolean.json`: the checked-out source was
`947db722640c86247849343657bf2f7ef01cb7f1`; ordinary checking emitted `0`,
`1`, and the separate verdict invocation emitted `ALL PROOFS CHECK`. This is
upstream tool evidence only. It is not an agent trial, a matched six-cell
benchmark, or a Bend-versus-SEMAPRAX result.

## Executed Boolean law-gaming controls

The Boolean fixture has executable mutant controls that retain the claimed law
while changing the implementation. The Bend mutant maps both constructors to
zero but keeps its equality proof; the SEMAPRAX mutant always returns `false`
while retaining `ensures result == !value`. Run them with pinned source roots
and write stdout/stderr artifacts outside the repository:

```sh
python3 benchmarks/bend2-law-v1/boolean_negative_controls.py \
  --bend-root /tmp/bend2-law-source \
  --bun /absolute/path/to/bun \
  --semaprax-root /path/to/pinned/semaprax \
  --semaprax /path/to/pinned/semaprax-binary \
  --semaprax-commit "$(git -C /path/to/pinned/semaprax rev-parse HEAD)" \
  --artifact-dir /secure/local/law16-boolean-negative-artifacts \
  --output /secure/local/law16-boolean-negative.json
```

The driver requires the pinned Bend head and supplied SEMAPRAX head, stores raw
stdout and stderr by path and SHA-256, and classifies the four paths separately:
ordinary Bend mutation rejection, Bend `--verdict` mutation rejection,
SEMAPRAX runtime success witness acceptance, and SEMAPRAX runtime mutant
rejection. Any unexpected zero exit, missing Bend kernel failure marker, or
wrong success output fails the receipt.

A local execution on 4 October 2026 completed this control using the pinned
Bend revision and a clean SEMAPRAX binary attributed to
`ac68e72595514acc717bb1825c3add9d6cac990c`. Both Bend paths rejected the
mutant with `SOME PROOFS FAIL`; the SEMAPRAX witness printed `0`, and its
mutant returned a contract language failure. Raw receipt and stream artifacts
are at `/tmp/law16-boolean-negative.json` and
`/tmp/law16-boolean-negative-artifacts/`. This is Boolean-only local execution
evidence, not current-head evidence, an agent trial, a timing result, or a
Bend-versus-SEMAPRAX comparison.

## Boolean cold/warm measurement preparation

`boolean_measure.py` prepares the available Boolean routes for a 30-repetition
local run. It pins both source heads and executable hashes; takes one cold
sample and at least 30 warm samples for ordinary Bend checking, Bend
`--verdict`, and SEMAPRAX runtime separately; retains every elapsed-nanosecond
sample and each command's stdout/stderr in an artifact directory; and derives
p50, p95, mean, and per-command peak RSS from those raw samples.

On Darwin, `/usr/bin/time -l` supplies a per-command peak resident-set-size
observation in bytes. On GNU/Linux the runner uses `/usr/bin/time -f %M` and
normalizes KiB to bytes. An absent or malformed wrapper remains explicitly
`unavailable`; the runner never substitutes cumulative parent-process memory.
The current bounded runner records SMT and external Lean Boolean paths as
unavailable because it has no compiler-owned command for either one.

```sh
python3 benchmarks/bend2-law-v1/boolean_measure.py \
  --bend-root /tmp/bend2-law-source \
  --bun /absolute/path/to/bun \
  --semaprax-root /path/to/current/semaprax \
  --semaprax /path/to/current/semaprax-binary \
  --semaprax-commit "$(git -C /path/to/current/semaprax rev-parse HEAD)" \
  --artifact-dir /secure/local/law16-boolean-measure-artifacts \
  --output /secure/local/law16-boolean-measure.json
```

This command is prepared but not run against a current-head SEMAPRAX binary.
Its timing values must remain separate route observations; it emits no ratio,
winner, GPU result, or claim about the unavailable checked-`u32` cells.

`law16_cold_warm_cell.py` is a lower-level 30-sample process-state cell for
already pinned commands and artifacts. It records both command streams and
artifact digests, but always marks OS-cache coldness unavailable: fresh
processes cannot isolate Darwin page, executable, solver, or tool caches.

Use it only for a pair of already reproduced paths: a fresh copy of the same
project or fixture and a repeat path with the same source bytes. Supply the
two source paths, the pinned executable, and its retained version receipt as
`--artifact` inputs. It writes all 120 child streams (stdout and stderr for 30
fresh-path plus 30 repeat-path children) beneath a new raw-artifact directory,
and the JSON receipt binds each stream's relative path, byte count, digest,
argv, and command digest. The process-state plan is committed as
[`evidence/law16-process-state-plan-v1.json`](evidence/law16-process-state-plan-v1.json).

```sh
python3 benchmarks/bend2-law-v1/law16_cold_warm_cell.py \
  --fresh-command '["/absolute/path/to/semaprax", "check", "/secure/local/law16-fresh/src/app.spx", "--json"]' \
  --repeat-command '["/absolute/path/to/semaprax", "check", "/secure/local/law16-repeat/src/app.spx", "--json"]' \
  --fresh-input /secure/local/law16-fresh/src/app.spx \
  --repeat-input /secure/local/law16-repeat/src/app.spx \
  --artifact /absolute/path/to/semaprax \
  --artifact /secure/local/semaprax-version-receipt.txt \
  --timeout-seconds 120 \
  --raw-artifact-dir /secure/local/law16-process-state-streams \
  --output /secure/local/law16-process-state.json
```

This is process provisioning evidence, not a cold-cache result. Preserve the
fresh and repeat source byte digests in the receipt; if they differ, the
result compares different programs and must be rejected during review.
