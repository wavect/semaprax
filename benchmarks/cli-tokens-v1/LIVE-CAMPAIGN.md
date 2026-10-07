# LogLens round 3 live campaign

`live_campaign.py` is the launch and collection path for matched LogLens trials.
It defaults to five trials per arm, pins Claude Code to `claude-sonnet-5-5` at
medium effort, creates a fresh detached Git worktree per trial, saves the raw
stream-JSON transcript, and independently checks the resulting program against
the checked-in oracle outputs. Failed, timed-out, non-compiling, and
non-accepting trials remain in the denominator.

The CLI does not expose a turn limit. The harness applies a wall-clock timeout
to each process and can pass Claude Code's `--max-budget-usd` cap when set.
It launches trials sequentially in alternating arm order. No fallback model is
configured; the transcript's observed model must match the pinned model.

Prepare and review a campaign without launching a model:

```sh
python3 benchmarks/cli-tokens-v1/live_campaign.py plan \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/to/loglens-round3
```

After the compiler and task are ready, launch it with the same arguments:

```sh
python3 benchmarks/cli-tokens-v1/live_campaign.py run \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/to/loglens-round3 \
  --semaprax-bin /absolute/path/to/verified/semaprax \
  --timeout-seconds 1800
```

The `run` command requires an empty, new artifact directory. It preserves each
trial's workspace, transcript, stderr, build log, acceptance log, and the
updated `results.json`. The runner does not inspect credentials. Claude Code
must already be authenticated for the selected account, and that account must
be entitled to the pinned model.

Input, cache-write, cache-read, and output usage values are copied from the
stream transcript when present. The first turn's input is reported separately
as initial context; it is not silently subtracted from the gross total. The
rate-card amount is a list-price estimate pinned to the date and prices in
`webapp-tokens-v2/cost.mjs`. It is not provider-billed cost. Actual billed cost
stays unknown until a matching provider receipt is supplied.

The acceptance runner invokes the arm's `candidate/build.sh` once, then its
`candidate/run.sh` against every golden and usage-error case. Build time and
logs are stored separately from the model transcript. The exact compiler
binary is supplied to every trial through `SEMAPRAX_BIN` and its digest is
recorded; TypeScript uses the same Node installation on `PATH` in every
trial.
