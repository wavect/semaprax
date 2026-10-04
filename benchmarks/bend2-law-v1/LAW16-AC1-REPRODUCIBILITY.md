# LAW16 AC1 reproducibility sequence

`law16_replay.py` is the entrypoint for retained evidence and the seven
currently executable non-agent routes. Retained review is offline: it
reauthenticates capsules and receipts, but runs no benchmark command and
reproduces no timing. Fresh capture invokes pinned local tools and writes a
new capsule; its results are new physical observations.

## 1. Reauthenticate retained evidence

From the repository root, choose a new output directory:

```sh
python3 benchmarks/bend2-law-v1/law16_replay.py \
  --verify-retained \
  --output-dir /absolute/new/law16-retained-review
```

This renders the current report, which independently reviews the retained
Boolean check, process, proof/verdict, RSS, agent, refactor, native-phase,
guest-cache, project-incremental, guarded-i64, Bend proof, and bounded-balance
capsules. It also records a digest inventory of the raw evidence directories.
`retained_evidence_verified` means those retained bytes passed their current
reviewers; it does not mean a tool was rerun. The report remains incomplete
while checked-u32 admission and other stated issue requirements remain open.

## 2. Physically rerun the seven pinned non-agent routes

Copy [`law16-replay-pins.example.json`](law16-replay-pins.example.json) to a
private local file and fill every executable path and SHA-256 digest. Pin the
exact clean Bend source tree at the committed revision and declare the
SEMAPRAX build source commit. The latter is caller-supplied metadata, not a
reproducible-build attestation. The tool file also requires Lean, its prebuilt
test binary, and the cached BendTT kernel even when a route does not use each
one. The runner makes no Cargo build.

```sh
python3 benchmarks/bend2-law-v1/law16_replay.py \
  --execute --pins /absolute/private/law16-replay-pins.json \
  --output-dir /absolute/new/law16-seven-route-capture
```

This physically runs, in order: Boolean ordinary checking; Boolean
verdict/Z3 candidate and attack checking; Boolean peak RSS; guarded-i64
balance/sort controls; guarded-i64 balance source proof; Bend universal U32
sort source proof; and the supplemental LAW15 Lean list theorem. The
bounded-balance agent capsule is reauthenticated offline in the same
invocation. The emitted `replay-status.json` lists `fresh_capture_routes`
separately from `offline_replay_routes`; only the former rerun tools.

The retained `law16-unified-fresh-v1` capsule predates the guarded-i64 balance
source-proof step and contains six fresh routes. A separate retained
`law16-unified-fresh-guarded-i64-v1` capsule records that seventh physical
route. `--verify-fresh` authenticates the older aggregate capsule offline; it
does not rerun it:

```sh
python3 benchmarks/bend2-law-v1/law16_replay.py \
  --verify-fresh benchmarks/bend2-law-v1/evidence/law16-unified-fresh-v1 \
  --output-dir /absolute/new/law16-six-route-review
```

Review the standalone seventh route with its route-specific verifier:

```sh
python3 benchmarks/bend2-law-v1/law16_guarded_i64_balance_smt.py \
  --review benchmarks/bend2-law-v1/evidence/law16-unified-fresh-guarded-i64-v1
```

## 3. Reproduce specialized physical cells

These cells have distinct instrumentation or prerequisites and are not part
of the seven-route command above. Run each into a new output directory. Their
existing evidence is reviewed by step 1; replaying the capsule does not run
these commands.

**Native phases.** Requires the exact Bend source pin plus pinned Bun,
SEMAPRAX, and Clang executables. Thirty repetitions capture the seven timed
phases listed in its receipt:

```sh
python3 benchmarks/bend2-law-v1/law16_boolean_native_phases.py \
  --output /absolute/new/law16-native-phases \
  --semaprax /absolute/pinned/semaprax --bun /absolute/pinned/bun \
  --bend /absolute/pinned/bend --clang /absolute/pinned/clang \
  --repetitions 30
```

**Linux/Rosetta guest file-page cache.** Requires the clean pinned Bend
checkout, the provisioned guest tool tree matching the committed release
manifest, the pinned local container image, and at least 2 GiB free space.
Thirty pairs require the authenticated one-pair pilot. This measures guest
file-page residency for Bend ordinary checking and SEMAPRAX `check`; it does
not observe host or Rosetta caches:

```sh
python3 benchmarks/bend2-law-v1/law16_guest_cache.py \
  --tools /absolute/pinned/guest-tools --bend /absolute/pinned/bend \
  --repetitions 30 \
  --pilot benchmarks/bend2-law-v1/evidence/law16-guest-cache-pilot-v1 \
  --output /absolute/new/law16-guest-cache
```

**Project incremental compiler cache.** Requires a prebuilt test binary and
the exact source checkout revision associated with it. Obtain the current
full source revision with `git rev-parse HEAD`; the runner rejects a different
HEAD. The test-binary association is recorded locally, not build-attested.
This runs two exact tests for the body-edit success and signature-edit
negative control; it is not a matched Bend route:

```sh
python3 benchmarks/bend2-law-v1/law16_project_incremental_cell.py \
  --test-binary /absolute/pinned/semaprax-test-binary \
  --source-commit FULL_40_CHARACTER_SOURCE_SHA \
  --output /absolute/new/law16-project-incremental
```

**Matched Boolean refactor.** Requires the committed Bend revision and the
runner's fixed executable digests for Bun, SEMAPRAX, and Z3. It performs one
ordinary/verdict/check/Z3 candidate and attack set; the separate checked-u32
refactor remains unsupported:

```sh
python3 benchmarks/bend2-law-v1/law16_boolean_refactor_cell.py capture \
  --bend-root /absolute/pinned/bend --bun /absolute/pinned/bun \
  --semaprax /absolute/pinned/semaprax --z3 /absolute/pinned/z3 \
  --output /absolute/new/law16-boolean-refactor
```

**Live Boolean agent continuation (explicit opt-in).** Add a SHA-pinned Codex
executable to the local replay pin file, then run the seven physical routes
and nine new matched agent pairs with the same entrypoint:

```sh
python3 benchmarks/bend2-law-v1/law16_replay.py \
  --execute --pins /absolute/private/law16-replay-pins.json \
  --include-agent-campaign \
  --output-dir /absolute/new/law16-seven-routes-and-agent-campaign
```

The fixed plan bounds each agent turn to 20,000 tokens and 600 seconds, and
declares a $0.00 cost ceiling. This runs nine pairs (ordinals 2–10); the
retained ordinal-1 pilot supplies the tenth historical pair. It is a new
Codex-backed run, so do not use it for offline review. The retained Codex
events have no monetary charge data, so this declared ceiling is not a
measured provider-cost result.

**Costed Claude Boolean campaign.** The completed 20-trial capsule uses the
separate v2 preregistration, with ten fresh matched pairs. Reproduction needs
the pinned Claude CLI and provider access, and incurs provider charges. The
plan specifies a requested $0.06 cap per trial and $1.20 total; provider
charges can exceed a requested per-trial cap and are retained as failures.

```sh
python3 benchmarks/bend2-law-v1/law16_claude_boolean_campaign.py \
  --plan benchmarks/bend2-law-v1/fixtures/law16-claude-boolean-campaign-plan-v2.json \
  --claude /absolute/pinned/claude --bend-root /absolute/pinned/bend \
  --bun /absolute/pinned/bun --semaprax /absolute/pinned/semaprax \
  --z3 /absolute/pinned/z3 --first 1 --last 10 \
  --max-cost-usd 0.06 --output /absolute/new/law16-claude-campaign
```

## Scope

The checked-u32 non-admission, remaining-u32 cells, and guarded-i64
supplemental routes remain separate facts. No i32 substitute, independent
LAW15 theorem, or Bend-only U32 theorem is promoted to a matched LAW16 result.
No sequence here claims a quiet host, isolated system caches, a winner, or
issue closure. Each fresh capture preserves its tool and source identities,
raw output, command receipts, and explicit nonclaims.
