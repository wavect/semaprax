# LogLens matcher integration reference

`matcher-adapted/` is a private source-only adaptation of a closed, accepted
LogLens SEMAPRAX candidate. `qualified-source/` preserves its application
source unchanged. Neither directory is a live trial seed, and the historical
campaign files were not modified. `provenance.json` binds the copied baseline
to its accepted results row, whole candidate inventory, and file hashes.

The reference candidate is `semaprax-02` from the closed
`loglens-current-source398b051e6-base393432ccb-20261009` run. Its retained row
records accepted status, all 33 historical checks and all 16 independent
boundary invocations passing. The candidate inventory was re-hashed before
copying; compiled binary and generated acceptance fixtures were omitted.

## Matcher adaptation

The adapted project copies the private ASCII matcher source into its source
set and imports only its byte matcher and capture observers. The report loop
compiles the request pattern once, then calls `full_match` on a borrowed byte
view of each already-split line. Captures provide byte spans for IP, hour,
method, path, status, and byte count; their fields feed the existing
aggregation path directly. It keeps the accepted app's token/path/hour/decimal
checks after matching. A matcher refusal or invalid compiled pattern takes a
separate terminal error path instead of silently counting the line as
malformed.

`request-pattern.json` records the adapted expression and capture order. The
pattern leaves month/day/minute/second/timezone-range semantics broad, as the
frozen contract requires, and the app retains the existing token/path checks.
Aggregation, exact decimal arithmetic, report formatting, command-line
handling, file limits, and line-ending handling are copied unchanged from the
qualified source. `tests.mjs` and the original build/run scripts are retained
as source references only; none were executed.

## Qualification boundary

The trusted acceptance sources are `benchmarks/cli-tokens-v1/SPEC.md`,
`benchmarks/cli-tokens-v1/oracle.py`, `benchmarks/cli-tokens-v1/qualification.py`,
and `benchmarks/cli-tokens-v1/boundary-audit-v1/corpus.json`. The boundary
corpus pins SPEC digest
`51bf564cb9f7eadd4b3bd12fac53b7857befa09b3ef757d9c34400a8509fdeae`.
`qualification.py` composes the historical 33 checks with eight corpus cases
in text and JSON forms, for 16 independent boundary invocations.

The copied app's accepted result is provenance for this private starting point,
not evidence for the adapted source. No adapted app or matcher was executed;
the retained 33+16 result does not transfer to this modification. The next
qualification step must run the existing acceptance flow on `matcher-adapted/`
and compare all output bytes and statuses. Long-line work-limit behavior and
ordinary source-fuel behavior are also unverified here. A semantic no-match and
a matcher resource refusal remain distinct outcomes.
