# RI-13 Darwin arm64 combined application evidence

This is a local macOS 26.5.1 arm64 run from clean source commit
`575768070ce451c786f304a7e1e0623444271d86`, using Cargo 1.98.0,
`/usr/bin/clang`, a new private target directory, locked offline dependencies,
and one cold plus one warm pass. The target cache was removed after capture.

The combined receipt records 9/0/0 cold stages, 9/0/0 warm stages, and 4/0/0
route measurements (22 passed, 0 failed, 0 skipped). `combined-raw/` retains
all 44 command stdout/stderr streams (36,136 bytes) with their receipt hashes.
The generated-code inventory and copy/transfer ledgers are in the receipt;
`friction-ledger.json` counts authored adapters and escape hatches. The two
investigation JSON files retain adverse batch-throughput findings and their
source-bound observations. They do not claim a performance pass or foreign
library internal copy counts.

Verify the retained bytes from any checkout with:

```sh
python3 examples/ri13-combined-app/measure.py \
  --verify-raw-artifacts examples/ri13-combined-app/measurements/darwin-arm64-575768070-2026-10-04/combined-receipt.json \
  --raw-artifact-dir "$(pwd)/examples/ri13-combined-app/measurements/darwin-arm64-575768070-2026-10-04/combined-raw"
```

The receipt also preserves the original absolute capture location. The
explicit override changes only where the verifier reads raw files; it still
requires exact names, command coverage, byte counts, and SHA-256 digests.
