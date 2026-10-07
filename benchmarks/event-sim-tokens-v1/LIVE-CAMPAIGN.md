# ShiftSim live campaign adapter

`campaign.py` provides the same Claude stream accounting, minimal seed
repository, candidate archive, and process-launch helpers as the LogLens
campaign. The seed repository contains only the pinned `SPEC.md`; it does not
contain the independent acceptance corpus or oracle. The acceptance runner
uses those hidden files outside each agent worktree after the candidate has
finished.

The adapter pins `claude-sonnet-5-5` at medium effort, alternates SEMAPRAX and
TypeScript trials, runs one matched empty-task calibration, and defaults to
five trials per arm. Without `--qualification-evidence`, every result remains
preflight-only and must not be presented as a scored comparison. A supplied
evidence file enables scored trials only after the campaign verifies the exact
SPEC and corpus hashes, compiler source commit and binary hash, native Project
stream route, and every per-case result. The oversized-whitespace case must
pass with 65,537 leading spaces. The gate is campaign evidence; it does not
close issue 611 or assert that issue has been closed.

The optional `--max-budget-usd` sets the CLI's per-session budget cap, so it
applies separately to the calibration and each trial. `--timeout-seconds`
bounds each session. An optional offline `--tokenizer-dir` records final-source
counts using the explicitly identified legacy Claude tokenizer proxy. Raw
provider usage, the historical net-input convention, list-price estimates, and
provider-reported API-equivalent costs remain separate in result files.

To prepare qualification evidence, run the independent acceptance adapter
against a native Project candidate using the compiler binary that will be
pinned in the campaign. It writes a JSON report with one status row per corpus
case, including input byte counts and output hashes:

```sh
python3 benchmarks/event-sim-tokens-v1/acceptance/run.py \
  --command-json '["/bin/sh","/absolute/path/to/native-candidate/run.sh"]' \
  --report-json /absolute/path/outside-repository/shiftsim-qualification-report.json
```

Review that report, then create an evidence JSON file with this shape. Use the
full compiler commit and SHA-256 hashes from the exact compiler binary and
report; `acceptance_corpus_sha256` and `spec_sha256` must match the selected
`--base-ref` commit.

```json
{
  "schema": "semaprax.event-sim-qualification-evidence.v1",
  "spec_sha256": "<sha256 of SPEC.md at compiler source commit>",
  "acceptance_corpus_sha256": "<sha256 of acceptance/corpus.json at compiler source commit>",
  "compiler_source_commit": "<full compiler source commit>",
  "compiler_binary_sha256": "<sha256 of the compiler binary>",
  "native_project_route": {
    "project_profile": "language-command-io.stream.v1",
    "input_route": "argv-utf8+stdin-stream.v1"
  },
  "acceptance_report": {
    "path": "/absolute/path/outside-repository/shiftsim-qualification-report.json",
    "sha256": "<sha256 of the reviewed report file>"
  }
}
```

The campaign checks the report against the pinned corpus, verifies all 11
per-case pass results and the actual oversized request length, and rechecks the
compiler binary hash before it dispatches any model session. Evidence is copied
into the external campaign artifacts for later review.

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

To plan an evidence-gated scored campaign, also supply the exact binary and
evidence paths. A plan without evidence remains preflight-only:

```sh
python3 benchmarks/event-sim-tokens-v1/campaign.py plan \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/outside/repository/shiftsim-round1 \
  --semaprax-bin /absolute/path/to/semaprax \
  --qualification-evidence /absolute/path/outside/repository/shiftsim-qualification.json \
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
  --qualification-evidence /absolute/path/outside/repository/shiftsim-qualification.json \
  --trials-per-arm 5 \
  --timeout-seconds 1800 \
  --max-budget-usd <approved-per-session-cap> \
  --tokenizer-dir /absolute/path/to/offline-tokenizer
```

`plan` and the offline tests make no model calls. `run` dispatches the
calibration and trial sessions; retain its campaign, results, transcripts,
candidate archives, and seed hashes together for review.
