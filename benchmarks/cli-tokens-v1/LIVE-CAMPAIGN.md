# LogLens round 3 live campaign

`live_campaign.py` is the launch and collection path for matched LogLens trials.
It defaults to five trials per arm and pins Claude Code to `claude-sonnet-5-5`
at medium effort. Each trial uses a non-cone sparse detached worktree that
contains only `SPEC.md` and `sample.log`; the oracle, goldens, other candidates,
and repository instructions are absent from the trial filesystem. The runner
copies candidate source and generated outputs to the external artifact folder,
records their hashes, then removes the disposable worktree if every change was
inside `candidate/`. A worktree with unexpected changes is retained for review.
Failed, timed-out, non-compiling, and non-accepting trials remain in the
denominator.

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
candidate archive, prompt, transcript, stderr, acceptance result, and the
updated `results.json`. The runner does not inspect or serialize credentials.
Claude Code must already be authenticated for the selected account, and that
account must be entitled to the pinned model.

Each trial is an independent Claude Code print session, not a nested subagent
inside a longer parent session. CLI help confirms `--permission-prompts none`
is supported. Trials use restricted mode, strict MCP configuration, and the
built-in Bash, Read, Edit, Write, Glob, and Grep tools so the session can build
and check its candidate without loading user-installed MCP tools.

Input, cache-write, cache-read, and output usage values are copied from the
stream transcript when present. Missing provider fields remain `null`; the
collector deduplicates repeated assistant message updates, records any mismatch
against final result usage, and prefers final result totals when available.
First-turn usage includes the task and provider-managed context, so the fixed
inherited context is explicitly marked unmeasured. Output counts are provider
reported output tokens; visible text bytes are retained separately and are not
treated as authored tokens. No compatible local tokenizer was found in the
bounded `/tmp` search. The rate-card amount is a list-price estimate pinned to
the date and prices in `webapp-tokens-v2/cost.mjs`; it is not provider-billed
cost. Actual billed cost stays unknown until a matching provider receipt is
supplied. The summary includes all attempts, failures included, in its
estimated cost per accepted task and reports per-trial and aggregate model
session wall time.

The acceptance runner invokes the arm's `candidate/build.sh` once, then its
`candidate/run.sh` against the independent oracle over empty and malformed
inputs, ties, 1xx responses, blank lines, LF/CRLF/CR files, half-up percentage
rounding, option order, default/top boundaries, and invalid/missing top values.
Fixtures are staged inside the candidate working directory and every input
path is relative to that directory, matching the language filesystem profile.
Build time and logs are stored separately from the model transcript. The exact
compiler binary is supplied to every trial through `SEMAPRAX_BIN` and its
digest is recorded; TypeScript uses the same Node installation on `PATH` in
every trial.
