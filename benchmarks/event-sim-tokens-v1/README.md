# ShiftSim token benchmark v1

ShiftSim measures the effort to build a deterministic discrete-event
scheduling engine. It exercises event ordering, stable priority selection,
multiple workers, exact integer metrics, validation, and a pure request/report
API. It is deliberately a different application from the LogLens text analyzer
and TeamDesk browser application.

`SPEC.md` freezes the behavior. Both language arms receive the same task prompt
and the same specification. `acceptance/corpus.json` is the shared test corpus;
`oracle.py` generates its expected reports. Live results belong in
`results-live.json` after matched runs are collected.

The corpus is invoked through a command adapter that reads one request from
stdin and writes one report to stdout. Example after an arm has been authored:

```sh
python3 benchmarks/event-sim-tokens-v1/acceptance/run.py \
  --command-json '["node","dist/cli.mjs"]'
```

Replace the command array with the executable for the arm under test. The
runner also checks the two invalid-request cases and requires status 2 with no
stdout. It does not compile either arm or call a model.
