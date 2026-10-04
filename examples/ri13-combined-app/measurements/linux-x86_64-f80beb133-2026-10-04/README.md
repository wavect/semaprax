# RI-13 Linux x86_64 combined application evidence

This is a locked, offline Linux x86_64 GNU guest run from clean detached
source commit `f80beb1336d226aceba19070229223cdc2407507`. Apple Container
ran the pinned Rust 1.98.0 image at digest
`sha256:c9024b5897124ae3a7f124a41dbe4d301c7319daf4e9aed82b23424648eb311e`
with Rosetta, 6 GiB memory, one Cargo job, and no guest network. It is an
executed Linux guest result, not a native physical x86_64 performance claim.

The combined receipt records 9/0/0 cold stages, 9/0/0 warm stages, and 4/0/0
route measurements (22 passed, 0 failed, 0 skipped). The separate linked
consumer also completed and emitted `ri13-linked-project-ok`. `receipt.json`
binds 53 retained files by SHA-256 through `output-digests.json`, including
all 44 raw command streams (36,153 bytes). The private build cache and
container were removed after the receipt passed. `friction-ledger.json`
counts authored adapters and escape hatches; the investigation files disclose
adverse throughput rather than claim a performance pass.

Verify the raw command streams from any checkout with:

```sh
python3 examples/ri13-combined-app/measure.py \
  --verify-raw-artifacts examples/ri13-combined-app/measurements/linux-x86_64-f80beb133-2026-10-04/combined-receipt.json \
  --raw-artifact-dir "$(pwd)/examples/ri13-combined-app/measurements/linux-x86_64-f80beb133-2026-10-04/combined-raw"
```

The verifier checks exact command coverage, names, sizes, and SHA-256 values.
The final Linux receipt additionally binds the individual logs and environment
file. The original `/evidence` paths in the receipts describe the guest's
capture location; the explicit override reads the committed copies.
