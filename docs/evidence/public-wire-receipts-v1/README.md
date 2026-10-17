# Public report wire receipts

Status: test-only diagnostic instrumentation; no pin update or hosted promotion.

Audience: compiler contributors and CI maintainers.

The completed Linux integration-3 job [117423478628](https://github.com/wavect/semaprax/actions/runs/39041964674/job/117423478628)
on `99f1f06d361e16bd937f4a6eda19703e78f0e525` reported ten exact public report
KAT/count failures. The public graph getter observed 868472 builder bytes rather
than the pinned 1428092. Reconstructing reports from the previous complete wires
using only that count, output-size fixed points and dependent digest bindings
failed all eight observed analysis hashes. Partial receipts therefore do not
justify replacing the pins.

The existing graph/analysis capture hook and three test hashing helpers now
observe complete fixture wires in GitHub Actions. Receipts carry a stable label,
byte length, full SHA-256 and hex payload. The decoder must independently verify
both length and SHA before treating a receipt as byte evidence. The observation
returns the original digest unchanged. Original fixture setup, report generation,
API/CLI comparison, read-only/locking checks and every literal/assertion remain.

Only JSON-shaped inputs up to 64 KiB and bounded single-line labels produce a
receipt. Larger or non-JSON inputs retain their original digest and test behavior
without capture. This bounds diagnostics, not compiler admission or report size.
Diagnostic output errors do not replace a test verdict. No artifact is published
and no pin, quota, schema, expected result or production implementation changes.

Owning component tests preserve raw framing including NUL, non-UTF-8 and line
break bytes, reject label injection and over-bound capture, authenticate the
complete payload, and preserve original SHA values. Complete actual hashing and
graph capture helpers run against real SHA2/DigestHex with an explicit namespace
adapter on Rust 1.88 and 1.98. Source evidence in `sources.json` binds six complete
predecessors, preserving all older source controllers and assertion bodies.
Full current-head compiler execution and receipt collection remain hosted gates.

The local component suite passed six tests on each compiler. All sixteen emitted
receipts decoded to their original bytes and SHA values. Three compiled mutants
(changed digest, corrupted payload, removed size bound) were rejected. All 29
original owning report test bodies remain byte-for-byte unchanged. These are
local component controls, not actual current-head public report captures.
