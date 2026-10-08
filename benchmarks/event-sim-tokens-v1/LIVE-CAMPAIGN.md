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
v2 stream route, and every per-case result. The selected SEMAPRAX command is
`fn() -> i64` and returns a portable process status from 0 through 255. Both
over-64-KiB valid cases must
pass: 65,537 leading spaces and maximum cardinality with escaped JSON keys and
identifiers. The compact maximum-cardinality control must remain at or below
65,536 bytes. Both invalid capacity cases (9 servers and 257 patients) must
exit 2, emit no stdout, and write exactly one diagnostic line to stderr. The
gate is campaign evidence; it does not close issue 611 or assert that issue has
been closed. Prior Project v23 / stream-v1 qualification evidence is historical
and cannot qualify this v2 route. For stream-v2 SEMAPRAX candidates, ordinary
invalid input must return application status 2 from the `i64` command result;
do not produce it by triggering a contract, read, or runtime failure.

The optional `--max-budget-usd` sets the CLI's per-session budget cap, so it
applies separately to the calibration and each trial. `--timeout-seconds`
bounds each session. An optional offline `--tokenizer-dir` records final-source
counts using the explicitly identified legacy Claude tokenizer proxy. Raw
provider usage, the historical net-input convention, list-price estimates, and
provider-reported API-equivalent costs remain separate in result files.

`--round 2` prepares the matched rerun after the compiler OPT batch. It keeps
the exact round-1 SPEC, all 15 corpus cases, oracle, prompt, seed inventory,
Sonnet 5.5 model, medium effort, and five-trial minimum. The plan records all
three frozen input hashes and rejects a different SPEC, corpus, or oracle.
Round 1 remains the default for historical tooling. Round 2 records the
2026-10-08 list-price card with cache reads at $0.1 per million tokens; round 1
retains its dated $0.2 card. Calibration and attempts use their campaign's
recorded card. Historical result files and their existing cost estimates are
not rewritten.

Round 3 preserves those frozen task bytes and uses the successor native-only
authoring route. It is selected only by the paired arguments `--round 3
--authoring-profile semaprax-project-v27-stream-data-v1`. The campaign refuses
a missing or mismatched profile rather than inferring a new meaning for an old
round. Its Project route is `semaprax.project.v27` /
`language-command-io.stream-data.v1` with
`argv-utf8+stdin-stream.v1`; the external command remains `fn() -> i64`.
Private helpers may borrow `Vec<T>` for the profile's Copy scalar set, while
the command and export roots stay closed. Both arms start with no `candidate/`
leaf so each agent creates its own implementation root.

A round-2 provider result explicitly reporting an error with status 429 or
`usage_limit_reached` stops further paid sessions. Results retain the failed
attempt and the ordered unlaunched arms, with status
`interrupted_provider_quota`; unlaunched tasks are not fabricated as attempts.
This is an incomplete sample and supports no comparative headline. Inspect
account quota before launch, and run one campaign at a time. `plan` dispatches
no paid session. A new compiler source/binary still requires a new reviewed
15-case qualification report; historical evidence cannot be relabeled.

To author the native candidate before it can qualify a scored comparison, run
one SEMAPRAX-only preflight. It uses the campaign's pinned prompt and model,
performs the usual one-turn calibration, builds/tests the candidate, checks the
independent corpus, and archives the candidate. The recorded campaign kind and
qualification status make the result explicitly single-arm and unscored:

```sh
python3 benchmarks/event-sim-tokens-v1/campaign.py preflight \
  --arm semaprax \
  --base-ref <verified-compiler-commit> \
  --artifacts /absolute/path/outside/repository/shiftsim-native-preflight \
  --semaprax-bin /absolute/path/to/semaprax \
  --timeout-seconds 1800 \
  --max-budget-usd <approved-per-session-cap> \
  --tokenizer-dir /absolute/path/to/offline-tokenizer
```

The preflight uses only the public SPEC in the agent seed. The acceptance
corpus and oracle remain in the benchmark checkout, outside its worktree. Its
source-only archive is at `candidates/semaprax-01`; review it and rebuild it
with the same compiler binary before generating the detailed qualification
report below. The preflight's campaign metadata records the full source commit
and compiler binary hash. Use that exact commit and binary for evidence and the
later scored campaign. The preflight does not create qualification evidence or
count as one arm of a matched comparison.

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
  "schema": "semaprax.event-sim-qualification-evidence.v2",
  "spec_sha256": "<sha256 of SPEC.md at compiler source commit>",
  "acceptance_corpus_sha256": "<sha256 of acceptance/corpus.json at compiler source commit>",
  "compiler_source_commit": "<full compiler source commit>",
  "compiler_binary_sha256": "<sha256 of the compiler binary>",
  "native_project_route": {
    "project_schema": "semaprax.project.v24",
    "project_profile": "language-command-io.stream.v2",
    "input_route": "argv-utf8+stdin-stream.v1",
    "command_result_type": "i64",
    "process_status_range": [0, 255]
  },
  "acceptance_report": {
    "path": "/absolute/path/outside-repository/shiftsim-qualification-report.json",
    "sha256": "<sha256 of the reviewed report file>"
  }
}
```

The campaign checks the report against the pinned corpus, verifies every
per-case pass result and both actual oversized request lengths, and rechecks the
compiler binary hash before it dispatches any model session. Evidence is copied
into the external campaign artifacts for later review.

Round-3 evidence uses schema
`semaprax.event-sim-qualification-evidence.v3`, the v27 native route above,
and adds these fields to the v2 envelope:

Create that closed evidence set with the harness-owned builder. It invokes the
pinned compiler directly, writes a fresh native binary outside the candidate,
runs the unchanged hidden acceptance runner against that binary, and emits the
inventory, copied manifest, report, receipt, binary, and evidence envelope:

```sh
python3 benchmarks/event-sim-tokens-v1/campaign.py qualify-v3 \
  --compiler-source-ref <verified-compiler-commit> \
  --candidate /absolute/path/to/reviewed-v27-candidate \
  --semaprax-bin /absolute/path/to/semaprax \
  --output /absolute/path/outside-repository/shiftsim-v27-qualification
```

```json
"candidate_source": {
  "inventory": {"path": "/absolute/path/candidate-source-inventory.json", "sha256": "<sha256>"},
  "manifest": {"path": "/absolute/path/candidate/semaprax.toml", "sha256": "<sha256>"}
},
"qualification_subject": {
  "compiler_source_commit": "<full compiler commit>",
  "compiler_binary_sha256": "<compiler sha256>",
  "closed_authored_inventory_sha256": "<canonical inventory rows sha256>",
  "candidate_manifest_sha256": "<manifest sha256>",
  "native_binary_sha256": "<accepted native binary sha256>"
},
"qualification_build_receipt": {
  "path": "/absolute/path/qualification-build-receipt.json",
  "sha256": "<sha256>"
},
"qualified_native_binary": {
  "path": "/absolute/path/qualified-native-binary",
  "sha256": "<sha256>"
}
```

The inventory uses `semaprax.closed-authored-inventory.v1`, contains sorted
unique regular-file rows and its canonical closed digest, and must contain
exactly one `semaprax.toml` row with the same hash.
The manifest must use `semaprax.manifest.v1`, profile
`language-command-io.stream-data.v1`, input
`argv-utf8+stdin-stream.v1`, one command function exported through
`exports.web`, and exactly the sorted capabilities `process.args.read`,
`process.stderr.write`, `process.stdin.read`, and `process.stdout.write`.
The evidence, report, inventory, manifest, receipt, and qualified binary are copied into the external
artifact closure. The build receipt uses schema
`semaprax.event-sim-qualification-build-receipt.v1`, repeats the exact
`qualification_subject`, and records the acceptance-report SHA-256. This binds
the reviewed compiler, closed authored inventory, manifest, native binary, and
per-case report as one subject. Every artifact hash is checked before and after
copy, before calibration. V2 evidence cannot qualify round 3, and V3 evidence
cannot qualify a historical round.

An ordinary model timeout remains a paid failed attempt. After its candidate
archive and safe worktree cleanup complete, the adapter continues the planned
matched order. Runner errors, resource contamination, retained workspaces,
non-timeout process failures, and invalid non-timeout telemetry still stop
later paid attempts.

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
