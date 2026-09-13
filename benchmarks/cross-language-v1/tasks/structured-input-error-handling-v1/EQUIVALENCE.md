# Task equivalence: `structured-input-error-handling-v1`

This validation task is a separate `validation` split in
`benchmarks/cross-language-v1/tasks.json`. It measures deterministic error
classification for a small versioned record envelope; it does not measure
parsing, I/O, allocation, or wall-clock performance.

## Problem and oracle

`validate(kind, version, payload_len)` returns the first applicable error code:

- `0`: kind `7`, version `1`, and payload length in the inclusive range `1..64`;
- `1`: unknown kind (checked first);
- `2`: unsupported version (checked after kind);
- `3`: payload length below `1` or above `64` (checked last).

The hidden overlay replaces only the language entry/test module and imports the
copied public candidate. Its vectors include inputs with more than one invalid
field, making the error precedence independently observable: a candidate that
checks version before kind, or accepts an out-of-range length, fails hidden
assertions.

## Inputs and outputs

Each vector supplies exactly three signed 64-bit scalar fields representing a
record envelope: kind, version, and payload length. The output is one signed
64-bit error code. Inputs are literals in each language's test source; no file,
stdin, environment, network, or external data is used.

## Boundary and toolchains

The measured region is the pure `validate` function, called by each language's
own test runner. Compiler and process startup are outside the measured region.
The adapters and success signals are `semaprax run .` through this task's
additive `semaprax-project` adapter, bare `rustc --test`, and `tsc --strict`
entries in `adapters.json`. The Project route lets hidden test modules import
the unchanged public SEMAPRAX candidate.

## Split and independent negative control

This task is in the `validation` split and shares no fixture or task family
with the development sequence digest or held-out counter repair task. The
public vectors cover valid envelopes and single-field failures. The hidden
overlay adds compound-invalid envelopes and boundary values while preserving
the copied public candidate. The executable
self-test
`structured_input_hidden_oracle_rejects_version_first_candidate_and_visible_test_tampering`
in `tests/documentation/cross_language_benchmark_suite.rs` is the independent
negative control: a deliberately wrong mutation of the committed public
candidate returns version errors before kind errors, passes the public vectors,
and fails the hidden compound-invalid vector. Deleting visible tests still
leaves the hidden overlay importing that same candidate, so hidden acceptance
remains authoritative.

The harness copies the hidden overlay only into the separate hidden phase and
checks that hidden-only files never enter the public build tree. No benchmark
result is claimed by adding this fixture; execution requires the existing
`run.py` with a provisioned adapter.
