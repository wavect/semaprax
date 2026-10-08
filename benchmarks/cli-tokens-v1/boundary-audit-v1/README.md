# Independent LogLens boundary qualification v1

This additive, post-round audit preserves the frozen SPEC, paid campaign,
33-check historical statuses, transcripts, candidate archives, and historical
oracle. It imports none of the live harness or oracle. Expected output derives
from authored valid request facts and explicitly malformed controls, using
arbitrary-precision Python integers, integer half-up rounding, and UTF-8 byte
ordering.

Eight fixtures run as both exact text and exact one-line numeric JSON:

- U+E000 and U+10000 tie ordering (UTF-8 differs from UTF-16).
- Decimal operands above JavaScript's safe integer range.
- A sum beyond i64, and a longer decimal digit token. The SPEC imposes no
  numeric ceiling on byte counts; no narrower language profile replaces it.
- A valid file of exactly 65,536 bytes with an unterminated final request;
  truncating its last byte changes the expected byte count.
- Literal-plus timezone grammar. Negative offsets are malformed under the
  SPEC's `+ZZZZ`, although the historical oracle accepts either sign.
- Exact request/status/hour/method/digit/separator grammar and the explicit
  quote/backslash path exclusions.
- Half-up 6.25% → 6.3%, LF/CRLF/CR, empty lines, and final-line processing.

No additional calendar, timezone-range, larger-file, or success-stderr
requirements are inferred. The input generator uses ordinary October dates
and valid minutes/seconds. Integer expected reports never pass through float.

Run offline authoring checks in this directory:

```sh
python3 -m unittest -v test_audit
```

Only after the matched round has all ten archived attempts:

```sh
python3 audit.py /absolute/completed-campaign/results.json \
  --output /absolute/new-external-audit-directory \
  --build-timeout 180 --case-timeout 120
```

The runner refuses incomplete rounds, verifies the frozen SPEC/compiler and
full archive inventories, copies each candidate to a new external directory,
and builds and executes only the copies. Every build/case has a process-group
wall-clock timeout; a timeout is recorded as a qualification failure. Full
stdout/stderr and expected/actual hashes remain in the new audit directory.
Builds inherit an offline npm setting and the campaign's exact compiler.

Execution mode is recorded from each copied entry wrapper and built executable
magic. SEMAPRAX interpreter execution is allowed by the original compile-or-
validate prompt; it is not disqualified because another attempt built native
code. Mode and timing are descriptive evidence, not a new language rule.

Expanded acceptance requires both the saved historical acceptance and all 16
new boundary comparisons. Historical accepted statuses remain separate.
All paid attempts, including new boundary failures, stay in the cost total and
attempt denominator. Costs are the saved conditional API-equivalent estimates,
not independently recounted provider receipts. Calibration remains separate.
A zero expanded-accepted denominator produces no cost-per-accepted estimate.
These finite probes are an executable qualification gate, not a proof over all
possible inputs or a change to historical acceptance.
