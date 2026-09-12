# Cross-language Agent benchmark laboratory (v1)

This suite is the **laboratory**, not a result: the harness, the task and
adapter inventories, the provenance binding, and the comparison/regression
logic that issue #211 asks for. It intentionally ships with **no committed
timing measurement**. See [Non-claims](#non-claims) below for why, and
[`docs/METHODOLOGY.md`](docs/METHODOLOGY.md) for the full equivalence
contract, provenance model, and what a future quiet-host run must do to
produce real numbers.

## Layout

| Path | Role |
| --- | --- |
| `run.py` | The harness: resolves tasks/adapters, builds and tests each task/language pair, records provenance, scores comparisons |
| `tasks.json` | Task inventory (schema `benchmark.cross_language.tasks.v1`). Each task declares a `split` — `development` (the frozen original pilot) or `held_out` (issue #106's contamination-protected extension; see that task's `EQUIVALENCE.md`) |
| `adapters.json` | Per-language adapter inventory (schema `benchmark.cross_language.adapters.v1`): official toolchain invocation, version probe, success signal |
| `tasks/<task-id>/EQUIVALENCE.md` | That task's fairness contract: inputs, outputs, measured boundary, allowed optimizations |
| `tasks/<task-id>/public/<language>/` | The source tree a solver (human or Agent) would author against |
| `tasks/<task-id>/hidden/<language>/` | Files that overlay (same relative path replaces, new paths add) the public tree for scoring only; never copied into the public build step |
| `results/` | Where a real run's output JSON goes; empty in this commit (see Non-claims) |

## Quick start

```sh
# Plan only: resolve every task/language pair, run nothing.
python3 benchmarks/cross-language-v1/run.py --dry-run --output /tmp/plan.json

# Score every implemented adapter against the committed pilot task.
python3 benchmarks/cross-language-v1/run.py \
  --semaprax target/debug/semaprax \
  --output /tmp/result.json

# Restrict to one task or one language.
python3 benchmarks/cross-language-v1/run.py --semaprax target/debug/semaprax \
  --only sequence-digest-v1 --language rust --output /tmp/rust-only.json

# Compare a run against a prior recorded result (pass/fail regression only;
# there is no timing field to compare in this schema version).
python3 benchmarks/cross-language-v1/run.py --semaprax target/debug/semaprax \
  --output /tmp/local.json --compare benchmarks/cross-language-v1/results/prior.json
```

## Languages

Nine languages are on the roster in `adapters.json`, matching issue #211's
initial list: SEMAPRAX, Zero, NTNT, Aver, Vera, Hale, MoonBit, Rust, and
TypeScript. Three are wired (`"implemented": true`) with a real, working
official-toolchain adapter today: **SEMAPRAX**, **Rust**, and **TypeScript**.
The other six are declared with an honest `blocked_reason` and never scored
as a pass or a fail — the harness reports them as `blocked`, distinctly from
`ok` or `failed`. Wiring one is a scoped, mechanical follow-up once its
official toolchain is available in a pinned, network-free form (see
`adapters.json` and `docs/METHODOLOGY.md`).

## Non-claims

- **No timing was measured to produce this suite, and none is committed.**
  This host runs many concurrent build lanes at once; a wall-clock number
  measured here would record contention, not the compiler or the language
  runtime. The result schema (`benchmark.cross_language.v1`) has no field to
  receive one by accident — see `docs/METHODOLOGY.md`. Issues #85, #130, and
  #131 own adding a timing metric once an exclusive quiet host is available.
- The original pilot task (`sequence-digest-v1`, `split: development`) is
  real, small, and deliberately narrow (see its `EQUIVALENCE.md`). It is
  evidence that the harness works end to end for three real languages, not a
  claim that SEMAPRAX outperforms or underperforms Rust or TypeScript at
  anything. It stays frozen; issue #106's held-out extension
  (`bounded-counter-repair-v1`, `split: held_out`) is added alongside it, not
  in place of it — see `tasks/bounded-counter-repair-v1/EQUIVALENCE.md`'s
  "Held-out discipline" section for what that split declaration commits to.
- This is local, single-host evidence for whichever toolchain versions
  happen to be installed on the run host, recorded, not pinned by a lockfile
  or a container image. Containerized/pinned environments are in issue
  #211's scope and are not built here (see `docs/METHODOLOGY.md`'s
  "What remains for a quiet-host run" section).
