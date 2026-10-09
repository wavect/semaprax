# ShiftSim token benchmark v1

ShiftSim measures the effort to build a deterministic discrete-event
scheduling engine. It exercises event ordering, stable priority selection,
multiple workers, exact integer metrics, validation, and a pure request/report
API. It is deliberately a different application from the LogLens text analyzer
and TeamDesk browser application.

`SPEC.md` freezes the behavior. Both language arms receive the same task prompt
and the same specification. `acceptance/corpus.json` is the shared test corpus;
`oracle.py` generates its expected reports. It currently contains 11 valid and
4 invalid cases (15 total). The interrupted first live campaign and its
normalized per-attempt evidence are documented in
[`round1-report.md`](round1-report.md) and [`results-live.json`](results-live.json).
That campaign did not complete its matched sample and supports no comparative
headline.

The adapter can prepare a matched `--round 2` rerun with these same frozen
inputs after the compiler OPT batch. Round 2 records its dated price card and
stops subsequent sessions on a structured provider quota refusal. Preparation
does not launch paid sessions or reuse historical compiler qualification as
current evidence; see `LIVE-CAMPAIGN.md`.

The campaign defaults to preflight-only reporting. A scored run requires a
reviewed qualification-evidence JSON file that binds the exact SPEC and
acceptance corpus hashes, compiler source commit and binary hash, native
streaming Project v2 route, and a hashed per-case acceptance report with every
corpus case passing. The SEMAPRAX command returns `i64` status 0 through 255
under Project v24 / `language-command-io.stream.v2`; stdin remains
`argv-utf8+stdin-stream.v1`. The corpus includes both 65,537 leading whitespace bytes
and a maximum-cardinality request with legally escaped JSON keys and
identifiers; the latter is over 65,536 bytes without whitespace. Compact
maximum-cardinality input remains under the old byte boundary, and 9-server and
257-patient requests must fail with status 2, empty stdout, and exactly one
diagnostic line.
Passing this evidence gate only allows a scored campaign; it does not close
issue 611 or assert that it has been closed. See
[`LIVE-CAMPAIGN.md`](LIVE-CAMPAIGN.md) for the evidence format and commands.
Earlier Project v23 / `language-command-io.stream.v1` preflights and evidence
remain historical and cannot qualify a v2 scored campaign. Qualification
evidence uses a distinct v2 schema, so a prior v1 route or report is refused.
The frozen native-v2 candidate qualification is recorded in [`qualification/native-v2-20261007/README.md`](qualification/native-v2-20261007/README.md), with its per-case report and portable evidence envelope. It only gates a future matched campaign; it is not a comparative result.
The fresh round-2 qualification for compiler `94fadd14c` is recorded in
[`qualification/native-v2-20261008/README.md`](qualification/native-v2-20261008/README.md).
Its unchanged archived candidate passed all 15 acceptance cases with the new
frozen binary; the matched campaign remains unlaunched.

Round 3 is an additive authoring route for Project v27. It must be selected
explicitly with `--round 3 --authoring-profile
semaprax-project-v27-stream-data-v1`; neither flag reinterprets a round-1 or
round-2 record. Its qualification envelope uses
`semaprax.event-sim-qualification-evidence.v3` and binds the reviewed candidate
source inventory plus the exact `semaprax.toml`, native binary, compiler, and
per-case report through a hashed build receipt. The manifest must select
`language-command-io.stream-data.v1`, the existing stdin-stream input, the
closed process capability list, and a single `fn() -> i64` command/export root.
The frozen SPEC, corpus, oracle, and functional acceptance remain unchanged.
The [fresh source 398 native-v3 qualification](qualification/native-v3-source398-20261009/README.md)
records unpaid session 71437 exiting 0 with all 15 cases passing. It binds the
archived compiler, retained candidate inventory/manifest, fresh native binary,
and exact acceptance report. Independent Cargo/test work overlapped its wall
time. Its publication preserves the original monitor validation false negative
and the corrected read-only receipt; no application rerun or paid campaign is
claimed. Historical native-v2 pins and paid measurements remain unchanged.
For every v27 SEMAPRAX attempt the harness invokes the pinned compiler directly,
builds a fresh native executable outside the candidate, and runs hidden
acceptance against that executable. Candidate build and test scripts remain
supplemental. Both round-3 arms must keep their closed authored inventory
unchanged through scripts, acceptance, measurement, and archive.
The explicit arm selects the route: only the SEMAPRAX arm may contain the v27
manifest or use the harness native binary. The TypeScript arm keeps the frozen
candidate `run.sh` interface and is refused if it contains `semaprax.toml`.

The corpus is invoked through a command adapter that reads one request from
stdin and writes one report to stdout. Example after an arm has been authored:

```sh
python3 benchmarks/event-sim-tokens-v1/acceptance/run.py \
  --command-json '["node","dist/cli.mjs"]'
```

Replace the command array with the executable for the arm under test. The
runner also checks all four invalid-request cases and requires status 2, empty
stdout, and exactly one diagnostic line on stderr. It does not compile either
arm or call a model.

`codex_campaign.py` is the separate Codex adapter for the same frozen task and
qualification gate. Its plan records `gpt-6.1-sol` at medium effort, the selected
CLI binary/version, and a dated conditional API price card. Task-owned rollout
records must reconcile the final CLI usage before acceptance. Calibration is
reported separately and never subtracted from trials. SPEC changes and writes
outside `candidate/` invalidate acceptance before archive or cleanup, and the
worktree is retained for review. Provider/process failures stop subsequent
attempts while retaining the original denominator and unlaunched order. An
ordinary model timeout is retained as a paid failed attempt and the matched
order continues after the candidate is archived and the worktree is safely
removed.
`--max-budget-usd` is refused because this CLI adapter cannot enforce a strict
monetary cap. Offline tests use synthetic model events and real minimal Git
worktrees; they are adapter evidence, not a live matched campaign.

Once a run has produced `results.json`, regenerate its read-only Codex report
with `python3 codex_report.py /path/to/results.json`. It derives model-request
usage and conditional cost only from reconciled rollout traces; missing traces
and incomplete arms remain explicit in the JSON report.

The fresh source398 campaign dated 2026-10-09 completed all ten planned attempts
(five per arm); all ten were accepted through the independent 15-case gate. See
the [campaign report](reports/codex-current-source398b051e6-base398b051e6-20261009.md)
and [trace-backed recount](reports/codex-current-source398b051e6-base398b051e6-20261009-recount.json).
This is a separate fresh run; historical reports remain unchanged, and no
language-advantage or savings claim is made.

Future matched runs can opt into the pinned dependency-only TypeScript setup
with `--typescript-bootstrap-receipt`. See [LIVE-CAMPAIGN.md](LIVE-CAMPAIGN.md)
for setup instructions and the unknown-context reporting limit.
