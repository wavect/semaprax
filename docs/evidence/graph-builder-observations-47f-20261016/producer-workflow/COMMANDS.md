# Explicit frozen-head workflow; no capture performed by derivation

Frozen head: `47f80b0f7a06fde9b2388439a9c0bfe998a05af1`. The generator requires the actual clean ROOT and actual
registered round. It never supplies a future bill, invokes a producer, or
applies the readonly plan. Managed published BEFORE remains actual35cc1209652;
364d, 2d4b and 8bb are retained observed histories, not applied pin values.

Run only after all FIVE actual build commands PASS and the parent schedules each
producer serially. The original observed-phase/RSS/timeout/settlement guards,
role-fixed producer and same-completed-build core/fullCLI artifact checks remain.

```sh
FLOW='/Volumes/Semaprax/provenance/paid-route-pin-workflow-47f-72-20261016'
ROUND='/Volumes/Semaprax/lane-prompts/final-47f-72-qualification-20261016'
BUILD_SHA=$(shasum -a 256 "$ROUND/owning-build-result.json" | cut -d ' ' -f 1)
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 "$FLOW/check-ready.py" --round "$ROUND" --expected-build-receipt-sha256 "$BUILD_SHA" --out "$FLOW/actual-completed-build-artifact-readiness.json"
READY_SHA=$(shasum -a 256 "$FLOW/actual-completed-build-artifact-readiness.json" | cut -d ' ' -f 1)
```

The parent must inspect the actual two SHA operands and readiness PASS before
continuing. Set three NEW external output paths; preserve every older capture.

```sh
: "${CAPTURE:?set a new external managed capture directory}"
: "${BROWSER:?set a new external browser capture directory}"
: "${PLAN:?set a new external readonly plan directory}"
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 "$FLOW/capture-managed.py" --out "$CAPTURE" --expected-build-receipt-sha256 "$BUILD_SHA" --expected-readiness-sha256 "$READY_SHA"
# Wait until the preceding phase is terminal before the browser phase.
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 "$FLOW/capture-browser.py" --out "$BROWSER" --expected-build-receipt-sha256 "$BUILD_SHA" --expected-readiness-sha256 "$READY_SHA"
# Only after actual managed producer exit0; inspect its actual receipt SHA.
PRODUCER_SHA=$(shasum -a 256 "$ROUND/prelude-pin-producers-result.json" | cut -d ' ' -f 1)
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 "$FLOW/prepare-after-actual-capture.py" --capture "$CAPTURE" --round "$ROUND" --expected-build-receipt-sha256 "$BUILD_SHA" --expected-producer-receipt-sha256 "$PRODUCER_SHA" --out "$PLAN"
```

Do not use `--apply`. Full original107 source predicates and exact11-literal
managed plan remain strict. Browser checks require independent complete
next→Main2600(3296148), observed8bb(3534564), observed2d4b(3558886),
observed364d(3472870), own35cc(4032239),17104(3930276),841(1214409) byte restorations after only
actual billing and the bound selfdigest. Any other field/byte difference fails
closed. Capture success is not owner-test PASS. Applying a reviewed actual plan,
source-front joins and affected consumer execution require a later parent step.

All five actual command groups must succeed/settle. Identical repeated full-CLI Cargo rows require complete profile, manifest/source path, source bytes and executable bindings to agree. Ten independent364d managed wires and their actual1103165 ordinary bill remain archived; the readonly plan produces complete next→364d wire inverses without applying pins.
