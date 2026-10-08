# LogLens round 3 live campaign

## Future round 6 qualification

The next Codex plan selects round 6. Its frozen SPEC/sample and both arm prompts
remain identical to round 5. Accepted now requires the historical build,
candidate tests and all 33 historical checks **and** the 16 independent
[SPEC boundary comparisons](boundary-audit-v1/README.md). The historical oracle
and its hashes remain unchanged; existing rounds and recounts retain their
original gates and statuses.

Both arms receive the same boundary inputs and exact expected bytes. Plans bind
the independent corpus, facts generator, qualification adapter and SPEC hashes.
Checks run after the paid attempt and before archival or workspace cleanup;
full comparison stdout/stderr, fixtures, expected bytes and execution mode are
saved under the new campaign's `qualification/<arm>-<number>/`. Each boundary
process has a 120-second process-group timeout; any timeout or mismatch refuses
acceptance. Native and interpreter routes remain eligible. All paid failures
remain in the attempted-task and cost denominators; calibration stays separate.
No expanded acceptance is assigned retroactively to an older result.

Offline gates (no provider calls):

```sh
cd benchmarks/cli-tokens-v1
python3 -m unittest -v test_qualification test_codex_campaign test_codex_report
```

The Claude adapter can explicitly select the same round-6 qualification with
`--round 6`; its legacy default remains round 3. Preparing this code authorizes
no new paid campaign.

`live_campaign.py` is the launch and collection path for matched LogLens trials.
It defaults to five trials per arm and pins Claude Code to `claude-sonnet-5-5`
at medium effort. Each trial uses a detached worktree from a fresh one-commit
seed Git repository containing only `SPEC.md` and `sample.log`, exported from
the pinned compiler commit's exact file bytes.
The seed records the source commit, file hashes, and fresh seed commit, and has
no original repository objects. The oracle, goldens, other candidates, and
repository instructions are absent from both the trial filesystem and its Git
history. The runner
copies candidate source and generated outputs to the external artifact folder,
records their hashes, then removes the disposable worktree if every change was
inside `candidate/`. A worktree with unexpected changes is retained for review.
Failed, timed-out, non-compiling, and non-accepting trials remain in the
denominator.

The CLI does not expose a turn limit. The harness applies a wall-clock timeout
to each process and can pass Claude Code's `--max-budget-usd` cap when set.
It launches trials sequentially in alternating arm order. No fallback model is
configured. Each session must report one model identifier, and every trial's
identifier must equal calibration's observed identifier. The CLI request is
pinned to one model setting; an alias may resolve to a dated identifier, which
is recorded in campaign metadata.

Prepare and review a campaign without launching a model:

```sh
python3 benchmarks/cli-tokens-v1/live_campaign.py plan \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/to/loglens-round3 \
  --tokenizer-dir /tmp/semaprax-opt-tokenizer
```

After the compiler and task are ready, launch it with the same arguments:

```sh
python3 benchmarks/cli-tokens-v1/live_campaign.py run \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/to/loglens-round3 \
  --semaprax-bin /absolute/path/to/verified/semaprax \
  --tokenizer-dir /tmp/semaprax-opt-tokenizer \
  --timeout-seconds 1800
```

The `run` command requires an empty, new artifact directory. It preserves each
candidate archive, prompt, transcript, stderr, acceptance result, and the
updated `results.json`. Candidate archives preserve files and record their
SHA-256 hashes while omitting only `node_modules`, `.cache`, `__pycache__`, and
`.pytest_cache`; each manifest lists the actual omitted paths. Build/run
scripts and lockfiles are retained. Dependencies and build outputs must be
reinstalled or regenerated as needed before using an archive; it is not
guaranteed runnable as archived.
The runner does not inspect or serialize credentials.

Recompute accounting later from the saved transcripts without invoking Claude,
the compiler, or candidate acceptance:

```sh
python3 benchmarks/cli-tokens-v1/live_campaign.py recount \
  --artifacts /absolute/path/to/loglens-round3
```

This writes `accounted-results.json` alongside the original results. It records
the accounting code revision and transcript hashes, refreshes provider usage,
TTL-aware rate-card estimates, and the aggregate summary, and preserves trial
acceptance, wall time, and candidate archive fields. The source `results.json`
is left unchanged. It also adds a post-run diagnostic separating the checks
defined by the frozen specification from additional robustness checks. The
original full-corpus acceptance result remains unchanged; this diagnostic is
not used to claim a comparative win. CR and CRLF input support should become an
explicit specification requirement before it is scored in a future matched
campaign.
Claude Code must already be authenticated for the selected account, and that
account must be entitled to the pinned model.

Before the ten benchmark sessions, `run` makes one matched calibration session
with the same model, effort, tool set, restricted mode, and minimal seed checkout. Its
fixed prompt asks for `READY` without reading files or using tools. Calibration
usage and its list-price estimate are recorded separately and included in the
combined campaign cost. Its first-turn provider input-plus-cache total, fixed
prompt count from the legacy tokenizer, and their difference are retained as a
one-turn context diagnostic. This is not an exact model-input measurement:
the tokenizer may differ from the active model, and the calibration differs
from a trial in its user prompt and subsequent history. No calibration value is
subtracted from trial totals.

Each trial is an independent Claude Code print session, not a nested subagent
inside a longer parent session. CLI help confirms `--permission-prompts none`
is supported. Trials use restricted mode, strict MCP configuration, and the
built-in Bash, Read, Edit, Write, Glob, and Grep tools so the session can build
and check its candidate without loading user-installed MCP tools.

Input, cache-write, cache-read, and output usage values are copied from the
stream transcript when present. Missing provider fields remain `null`; the
collector deduplicates repeated assistant message updates, records any mismatch
against final result usage, and prefers final result totals when available.
Raw per-trial input/cache counters remain authoritative. These counters include
repeated system and tool context on every turn, along with the benchmark task
and accumulated tool history. For comparison with earlier benchmark reports,
the runner also emits `legacy_net_input_tokens`, computed as the sum of
deduplicated per-turn input, cache-write, and cache-read counts minus the
first-turn input-plus-cache total multiplied by the number of turns. It reports
the first-turn baseline and subtracted subtotal separately; if any required
per-turn field is missing, the legacy metric is `null`. The first turn includes
the task prompt and harness context, so this operational convention is not
task-only model input and may be negative. It does not alter raw usage or
list-price estimates. The calibration diagnostic is separate and is never
subtracted here. Output counts are provider-reported output
tokens (including thinking when the provider includes it in that counter);
visible text bytes are retained separately. The optional offline authored
source count is a snapshot of final candidate files and uses
`@anthropic-ai/tokenizer@0.0.4`'s bundled Claude BPE, with its
`tiktoken` dependency versions, Node version, and package fingerprint recorded.
It counts candidate `.spx`, `.ts`, `.tsx`, scripts, and text manifests, while
excluding compiler-generated C, binaries, `dist`, `node_modules`, and staged
acceptance fixtures. This final-source snapshot excludes rewritten or deleted
text, so it is not cumulative authored generation or provider output. Treat it
as a legacy-Claude tokenizer proxy, not exact current-model or billing tokens.
Install it outside the repository with:

```sh
npm install --prefix /tmp/semaprax-opt-tokenizer --ignore-scripts --no-audit --no-fund @anthropic-ai/tokenizer@0.0.4
```

This is a measurement dependency only; it is not part of either implementation
or the compiler build. The rate-card amount is a list-price estimate from the
official [Sonnet 5.5 pricing page](https://platform.claude.com/docs/en/models/sonnet-5-5/overview),
dated 2026-10-07: $2/M input, $2.50/M 5-minute cache writes, $4/M 1-hour cache
writes, $0.20/M cache reads, and $10/M output. If cache-write TTL buckets are
absent, the estimate prices all reported cache-creation tokens at the 5-minute
rate and records that assumption. If TTL buckets are present, the estimator
uses the split and records its basis. Provider `result.total_cost_usd`, when
present, is retained as a provider-reported API-equivalent amount; it is not a
receipt or a confirmed account-billed charge. The summary includes all
attempts, failures included, in its
estimated cost per accepted task, reports the shared calibration cost
separately and in the combined campaign estimate, and reports per-trial and
aggregate model-session wall time.

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
