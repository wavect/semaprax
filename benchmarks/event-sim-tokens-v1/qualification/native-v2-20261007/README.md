# ShiftSim native v2 qualification evidence

This record qualifies one SEMAPRAX candidate for the campaign's native streaming acceptance gate. It is a single-arm candidate qualification, not a matched comparison or a scored performance result. Comparative outcomes remain pending in the separately scored matched campaign.

The candidate archive from the single-arm preflight was copied to scratch. Its original, unmodified `test.sh` ran with the frozen compiler below, rebuilt `dist/shiftsim`, ran five named Project tests and the candidate fixtures, and exited 0. The rebuilt native executable SHA-256 is `8e9feeaf5db85d91166a585aed4b272b9e2351d912470862fca2b8857b7d30da`. `candidate-test.log` preserves the command output, `candidate-test-run.json` records the result, and `candidate-source-inventory.json` records hashes for 69 candidate files excluding the generated `dist` binary.

The independent acceptance runner then invoked the rebuilt candidate's `run.sh`. All 15 pinned cases passed: 11 valid and 4 invalid. This includes the 65,537-leading-whitespace request and the over-64-KiB escaped maximum-cardinality request, the compact maximum-cardinality control, and both invalid capacity boundaries. The complete per-case report is `acceptance-report.json`; the v2 evidence envelope binds that report and the exact compiler route and input hashes in `qualification-evidence-v2.json`.

Frozen identities:

- Compiler source commit: `1e0e988218b51ad626881b87c83c553fc7cddf37`
- Compiler binary SHA-256: `1e5b4b0e5bd1c3e27cd09e72c47e8853ac39d035732c8a087c20ec4d663c183e`
- SPEC SHA-256: `5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00`
- Acceptance corpus SHA-256: `3c285999cfcf6a905e885d636ac55ba0ccb0f5999ef5b20ac3a8c17a2e023587`
- Native route: Project v24, `language-command-io.stream.v2`, `argv-utf8+stdin-stream.v1`, command result `i64` with process status 0–255
- Acceptance report SHA-256: `6d2edb5c32701c57d8f4b8c2c065c2e0bbd2951c1db88598c2dcda62c88dc1af`

The campaign's offline evidence gate accepted this package for five trials per arm at `claude-sonnet-5-5`, medium effort. That gate only permits a matched campaign; it says nothing about comparative outcomes. Keep the eventual calibration, trials, and raw usage in their own campaign artifacts.
