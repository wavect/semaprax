# ShiftSim token benchmark v1

ShiftSim measures the effort to build a deterministic discrete-event
scheduling engine. It exercises event ordering, stable priority selection,
multiple workers, exact integer metrics, validation, and a pure request/report
API. It is deliberately a different application from the LogLens text analyzer
and TeamDesk browser application.

`SPEC.md` freezes the behavior. Both language arms receive the same task prompt
and the same specification. `acceptance/corpus.json` is the shared test corpus;
`oracle.py` generates its expected reports. It currently contains 11 valid and
4 invalid cases (15 total). Live results belong in `results-live.json` after
matched runs are collected.

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
