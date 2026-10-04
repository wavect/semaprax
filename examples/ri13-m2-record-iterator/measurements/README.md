# Local M2 record and callback samples

The adjacent CSV is the raw stdout of the `measure` binary at code commit
`1a03f2801`, SHA-256
`6b5f552954a4b7944cfc8d29be723c7312621cb9f176ab86766f2faf1a3d87bf`.
It contains exactly five timed samples per route and task, plus one header.
Each sample runs 32 operations after one warmup. The route order rotates.

Environment: local macOS 26.5.1, Darwin arm64, 18 GiB physical RAM,
Homebrew rustc 1.98.0 (88d9e12ae 2026-08-18), Cargo 1.98.0
(797e8a9bc 2026-08-05). The generated Rust module was 11,384 bytes and
generated C module 4,208 bytes. Build elapsed was 4m05s for a fresh private
debug target with `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0` and
`CARGO_PROFILE_DEV_DEBUG=0`; another host build ran concurrently, so that
elapsed time is environmental context, not a controlled cold-build
comparison. The measurement binary itself was rerun after commit to capture
the raw CSV without rebuilding.

| Task | Route | Median batch latency | Allocation calls per batch | Allocated bytes per batch |
| --- | --- | ---: | ---: | ---: |
| Generic record | Direct Rust | 88,917 ns | 128 | 8,384 |
| Generic record | Handwritten adapter | 89,750 ns | 128 | 8,384 |
| Generic record | Generated Semaprax | 92,875 ns | 192 | 8,576 |
| Stateful callback | Direct Rust | 1,250 ns | 0 | 0 |
| Stateful callback | Handwritten adapter | 4,209 ns | 32 | 512 |
| Stateful callback | Generated Semaprax | 23,625 ns | 128 | 5,888 |

These are exploratory five-sample debug-build timings for tiny operations.
The direct and handwritten routes implement the same successful values but
do not authenticate a Semaprax source revision or enforce its lifecycle;
the generated callback does. Rust allocator counts exclude any foreign
allocator. The zero fixture-owned adapter buffer copy count refers only to
the scalar callback bridge; it is not a zero-copy JSON claim. No throughput
threshold or broad crate support conclusion follows from these samples.
