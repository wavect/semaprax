# LogLens matcher integration reference

This directory records the matcher boundary for a future private LogLens
adapter. It is not an application, does not alter the qualified benchmark, and
does not claim that matcher fixtures cover the CLI's 49 acceptance checks.

## Pinned acceptance sources

The trusted references are `benchmarks/cli-tokens-v1/SPEC.md`,
`benchmarks/cli-tokens-v1/oracle.py`,
`benchmarks/cli-tokens-v1/qualification.py`, and
`benchmarks/cli-tokens-v1/boundary-audit-v1/corpus.json`. The boundary corpus
pins the SPEC digest
`51bf564cb9f7eadd4b3bd12fac53b7857befa09b3ef757d9c34400a8509fdeae`.
`qualification.py` composes the unchanged historical 33-check result with
the 16 independent boundary invocations (eight corpus inputs × text and JSON).
The checked-in corpus and qualification runner remain the source of truth;
this reference does not copy their expected outputs.

The matcher-level boundary needed by a future app is a full-line request
recognizer that returns byte spans for IP, hour, method, path, status, and
byte-count fields. `request-pattern.txt` is a proposed pattern for that narrow
job. It deliberately leaves hour-range and byte-token semantics to the app:
the app must check hour `00`–`23`, and accept byte count `-` or ASCII digits.
The matcher itself does not split LF/CRLF/CR, read files, parse command-line
arguments, aggregate numbers, rank paths, or render the reports.

## Binding needed for a real private application

To make this a qualification driver rather than a pattern reference, an
application adapter must compose the existing matcher source with the private
LogLens source and call `compile`/`full_match` for each already-split line. It
must use capture start/end offsets as byte offsets into the original line,
then hand captured fields into the app's existing validation and aggregation
path. The adapter must not normalize or narrow any input required by the
frozen SPEC or corpus.

The driver still needs to execute the existing historical qualification and
the independent 16 boundary invocations against the same private app. It must
retain the existing statuses, stderr, stdout bytes, and report comparisons.
Until that app binding exists and those checks run, this directory establishes
only the matcher interface and fixture provenance; it is not a substitute for
33+16 application qualification.

## Limits of the evidence

No matcher or application was executed while preparing this source reference.
Long inputs still need ordinary source-fuel validation in the eventual app;
the matcher's work-limit/refusal status is separate from its semantic
no-match result. In particular, this artifact makes no claim that the
65,536-byte corpus case fits default interpreter fuel or that a refusal is a
valid CLI result.
