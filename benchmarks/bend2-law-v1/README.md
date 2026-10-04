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

The reviewed SEMAPRAX scalar profile does not support the checked-`u32` cells.
They remain explicitly `unsupported`, so the current mixed plan is
`partially_preregistered` and exits nonzero. It cannot be relabeled as a
successful matched trial or use an `i32` substitute.

## Pinned Bend Boolean smoke fixture

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
`semaprax.bend2-law-benchmark.agent-telemetry-export.v1` document bound to the
exact preregistration digest. Each trial supplies its transcript digest, the
existing `token_usage` and `cost_usage` telemetry events (with distinct event
identities), completed wall-time observations for proof synthesis, law-kernel
checking, and compilation or runtime, plus digest-bound observations for every
success witness and seeded attack.

The capture command validates that structure against the fixed plan:

```sh
python3 benchmarks/bend2-law-v1/agent_trial_capture.py \
  --plan /secure/local/law16-agent-plan.json \
  --raw-export /secure/local/law16-agent-raw-export.json \
  --output /secure/local/law16-agent-capture.json
```

It emits `completed` only when the raw export covers every preregistered trial
for both languages. A partial export remains `partial` and exits nonzero;
accepted attacks, omitted phase timings, reused telemetry events, unknown
trial IDs, and a changed plan digest are refused. The output retains raw-export
and transcript digests plus the telemetry values and separate phase durations.
It does not authenticate an external telemetry provider, execute an agent, or
state a comparison result.

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
