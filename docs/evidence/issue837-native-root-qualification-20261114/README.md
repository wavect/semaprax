# Application starter qualification for #837

Audience: compiler contributors and reviewers.

This is acceptance coverage and local evidence, not a new installer, a general
completion claim, or a measured efficiency improvement. The existing compiler-owned
starter already composes checked codec plans and command profiles.

## Exact scope

Base: `154fddcf3a51003bbcd6690ced570958075d313c` (current main when this
qualification branch was created). Linux x86_64, Rust/Cargo 1.98.1, Ubuntu Clang
18.1.3, Node 22.23.1. No full suite is required by the issue-specific maintainer
instruction: https://github.com/wavect/semaprax/issues/837#issuecomment-6864956129.

The selected-file acceptance previously asserted only native-test process success.
It now requests JSON and requires the declared LogLens main and decimal test roots,
the native schema/target, and successful per-root outcomes. Empty, missing or
substituted cases must not count as declared-test evidence. The independent CLI49
receiver, source fixtures and production native-test implementation are unchanged.

## Reproduction

Use a private target under this checkout, the same physically resolved Cargo and
Rustup homes for every command, one build job and serial tests:

```sh
export CARGO_HOME="$(realpath "${CARGO_HOME:-$HOME/.cargo}")"
export RUSTUP_HOME="$(realpath "${RUSTUP_HOME:-$HOME/.rustup}")"
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target/private"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_BUILD_JOBS=1
cargo test --offline --locked -p semaprax-toolchain --test cli_new_project_v1 \
  application_starter:: -- --test-threads=1
cargo clippy --offline --locked -p semaprax-toolchain \
  --test cli_new_project_v1 --no-deps -- -D warnings
```

Populate the locked dependency cache separately when needed; the actual checks
remain offline. Clang and Node are prerequisites, not installed by these commands.

## What the journeys prove

- Typed ShiftSim: fresh installation, unchanged authored source, deterministic
  complete inventory/receipt and canonical roundtrip, project check, explicit
  collection-record pure native tests, native build, all 15 original valid/invalid
  inputs with exact output/exit checks, and retained legacy native refusal.
- Selected-file LogLens: fresh install, canonical/receipt roundtrip, project check,
  exact declared native roots, native build, unchanged CLI49 receiver. CLI49 names
  33 historical plus 16 SPEC bodies; its receiver executes 207 internal checks.
- Three other starter checks cover selected relative inputs/no-follow, incomplete
  options, and the explicit tiny collection-record pure route.

Successful subprocess stdout is not individually printed by the integration test.
A passing test plus its mandatory assertions is the evidence; this report does not
invent separate check/build/application transcripts. Selected-file installation of
an additional Row codec does not prove its runtime consumption by LogLens.

## Limits

These focused checks do not qualify every backend or every #837 hostile-publication
and codec-replay gate. Earlier 37-test qualification at `f9849aaf` and five journey
reruns at `645747d57` are historical evidence, not automatically results for this
base. No full-suite or hosted-CI status is claimed. The three separate internal
old/new/allocation unit gates were not run as part of this focused contribution.
No compiler-only token, time or cost saving has been measured.

The earlier dependency-including Clippy run at `f9849aaf` failed on an unchanged
core `manual_slice_fill` lint at
`src/cleanup_plan/replay/expression_facts/arena/invocation.rs:193` with Rust 1.98.
A passing test-package `--no-deps` run is narrower evidence and does not clear
that historical core finding or claim a workspace-wide clean lint run.
This contribution changes no generated pins; no pin regeneration is required.

## Matched setup/repair trial requirements

Before collecting outcomes, freeze both workflow revisions, identical application
requirements and immutable acceptance receivers, machine/toolchain, the same model
and version/quantization/settings/context, tool permissions, prompts, token/turn
budgets, stopping rules and permitted repair steps. Keep a strong idiomatic
TypeScript baseline. Setup and repair are separate conditions. A changed scaffold,
prompt or profile is a distinct treatment, not a compiler-only causal comparison.

Pre-register repeated-run count, order/randomization and failure handling. Each run
needs an ID, arm, immutable revision/model/settings and receiver hashes, task,
input/output tokens with accounting source, tool turns, commands, retries, elapsed
time, completion/timeout/error, acceptance result, and redacted transcript/log.
Keep failed runs and interventions, not only successes. Report paired raw outcomes,
distributions and failure rates. Reduced help bytes or command count alone is not
reduced accepted-task cost. No savings claim until matched repeated runs complete.

## Verification results

At the base above plus this test-only assertion change, the five `application_starter::`
tests passed (0 failed, 31 filtered out) on 2026-11-14 at 09:58 UTC. Test time
413.04 seconds, Cargo exit 0. These are five distinct focused tests, not a claim
that the entire harness or all previously run tests were rerun.

| Selected test | Result |
| --- | --- |
| `application_starter_relative_selected_inputs_keep_exact_bytes_and_no_follow` | pass |
| `application_starter_selected_file_declared_native_tests_and_unchanged_cli49` | pass |
| `application_starter_typed_shiftsim_fresh_check_native_tests_and_original_fifteen` | pass |
| `incomplete_starter_options_select_exact_usage_before_inputs_or_publication` | pass |
| `tiny_collection_record_selector_executes_typed_native_roots_and_keeps_legacy_refusal` | pass |

Assertion-sensitivity experiment: temporarily remove only the decimal test from
the copied authored test module, leaving its main runnable. A first raw truncation
was rejected earlier as noncanonical (`SPX-G170`), so it was not evidence for the
new assertion. Repeating with the mutated source canonically formatted let native
testing return success but the new case-count assertion failed: `left: 1`,
`right: 2`, `both declared LogLens roots must execute` (Cargo exit 101).
The temporary mutation was fully removed before the five green tests above.
This is a deliberate coverage mutation, not a discovered installer/native defect.

Scoped test-harness Clippy (`--no-deps`, `-D warnings`) passed with exit 0
at 10:00 UTC (2m18s). `rustfmt --check` on the changed test file and
`git diff --check` passed. The original ShiftSim corpus and LogLens receiver,
launcher and unit fixture hashes are unchanged. No generated pins changed.
