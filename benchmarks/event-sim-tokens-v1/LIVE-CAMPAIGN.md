# ShiftSim live campaign adapter

`campaign.py` provides the same Claude stream accounting, minimal seed
repository, candidate archive, and process-launch helpers as the LogLens
campaign. The seed repository contains only the pinned `SPEC.md`; it does not
contain the independent acceptance corpus or oracle. The acceptance runner
uses those hidden files outside each agent worktree after the candidate has
finished.

The adapter pins `claude-sonnet-5-5` at medium effort, alternates SEMAPRAX and
TypeScript trials, runs one matched empty-task calibration, and defaults to
five trials per arm. The optional `--max-budget-usd` sets the CLI's per-session
budget cap; `--timeout-seconds` bounds calibration and each trial. An optional
offline `--tokenizer-dir` records final-source counts using the explicitly
identified legacy Claude tokenizer proxy. Raw provider usage and the historical
net-input convention remain separate in the result files.

Issue #611 remains open: the
specification's stdin contract has no reconciled maximum raw request size, while
the native Project input envelope is bounded. Therefore every result from this
adapter is labeled `preflight_not_qualified`; passing the current corpus is
only a preflight outcome and must not be presented as a qualified score or
completed contract coverage. Resolve and freeze the transport boundary before
using a campaign for a scored comparison.

From the compiler checkout, first inspect the frozen inputs and campaign
settings with `plan`:

```sh
python3 benchmarks/event-sim-tokens-v1/campaign.py plan \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/outside/repository/shiftsim-round1 \
  --trials-per-arm 5 \
  --timeout-seconds 1800 \
  --max-budget-usd <approved-per-session-cap> \
  --tokenizer-dir /absolute/path/to/offline-tokenizer
```

Once the native compiler binary and compiler commit are verified, `run` uses
that exact binary in every native-arm prompt and acceptance environment:

```sh
python3 benchmarks/event-sim-tokens-v1/campaign.py run \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/outside/repository/shiftsim-round1 \
  --semaprax-bin /absolute/path/to/semaprax \
  --trials-per-arm 5 \
  --timeout-seconds 1800 \
  --max-budget-usd <approved-per-session-cap> \
  --tokenizer-dir /absolute/path/to/offline-tokenizer
```

`plan` and the offline tests make no model calls. `run` dispatches the
calibration and trial sessions; retain its campaign, results, transcripts,
candidate archives, and seed hashes together for review.
