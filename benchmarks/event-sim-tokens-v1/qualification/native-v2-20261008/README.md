# ShiftSim round 2 native qualification

This is a single-candidate qualification for a future matched campaign, not a comparative performance result. No paid round 2 sessions were launched.

The accepted `semaprax-02` candidate from the round 1 archive was copied unchanged to external scratch, excluding generated `dist` and Git metadata. Its original `test.sh` rebuilt the native executable once with the frozen compiler below, passed six named Project tests and its authored fixtures, and exited 0 in 32.875 seconds. `candidate-source-inventory.json` records SHA-256 hashes for 64 copied source files; `candidate-test.log` and `candidate-test-run.json` retain the test outcome. Candidate sources and generated binaries remain in `/Users/kevin/.codex/benchmark-runs/shiftsim-round2-qualification-20261008/candidate`.

The independent acceptance runner passed all 15 frozen cases: 11 valid and four invalid. This includes both requests over 64 KiB, the compact maximum-cardinality control, and both invalid capacity boundaries. `acceptance-report.json` retains each input size/hash, output hash, status, and invalid-input diagnostic shape. The campaign's offline qualification gate accepted the report and exact compiler identities.

- Compiler source: `94fadd14cf27d045d22b23a79def640c95131c31`
- Compiler SHA-256: `da3d978118e233acaef18e0b0e48cd8c4741da51d7636d84fefacb19703ba51c`
- Native candidate SHA-256: `be382207268a340a575a0d79555a4c1893133ab5e9a5dffa1d6ab2090d7e84f4`
- SPEC SHA-256: `5a8631fc59f55d145bfabb62c8edd3f86164114e3d27b69422031b664b529e00`
- Acceptance corpus SHA-256: `3c285999cfcf6a905e885d636ac55ba0ccb0f5999ef5b20ac3a8c17a2e023587`
- Acceptance report SHA-256: `6d2edb5c32701c57d8f4b8c2c065c2e0bbd2951c1db88598c2dcda62c88dc1af`
- Route: Project v24, `language-command-io.stream.v2`, `argv-utf8+stdin-stream.v1`, `i64` command result and process status 0–255

Round 2 uses the unchanged SPEC, corpus, oracle, prompt, SPEC-only seed, Sonnet 5.5 medium effort, and five trials per arm. Its dated 2026-10-08 list-price card is separate from historical round 1 estimates. Provider quota currently blocks paid Sonnet sessions; this evidence only qualifies the compiler route and does not establish campaign outcomes.

Inspect an unlaunched round 2 plan from the compiler checkout:

```sh
python3 benchmarks/event-sim-tokens-v1/campaign.py plan \
  --round 2 \
  --base-ref 94fadd14cf27d045d22b23a79def640c95131c31 \
  --artifacts /Users/kevin/.codex/benchmark-runs/shiftsim-round2-20261008-r1 \
  --semaprax-bin /Users/kevin/.codex/benchmark-binaries/semaprax-94fadd14c \
  --qualification-evidence benchmarks/event-sim-tokens-v1/qualification/native-v2-20261008/qualification-evidence-v2.json \
  --trials-per-arm 5
```

`plan` makes no provider call and creates no campaign artifact directory. The campaign remains unlaunched while provider quota is unavailable. A structured quota error during round 2 stops remaining sessions and preserves the attempted and unlaunched trial order separately.
