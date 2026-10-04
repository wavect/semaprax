# RI-13 M1 local batch investigation

This directory records one local Darwin arm64 measurement after the bounded
batch route began reusing one closed carrier context. It remains an
investigation because the generated throughput is below the 0.90 trigger for
at least one matched route. It is not a performance pass, portability claim,
or measure of varying foreign input.

[`darwin-arm64-93c9e2599-2026-10-04.csv`](darwin-arm64-93c9e2599-2026-10-04.csv)
is the exact stdout of the `measure` binary at checkout `93c9e2599`. Its
SHA-256 is
`e41d1c66120711d3147a5cd6ec2e0a2c937486c7af3866654c087d67868a666a`.
The run has five timed batches per route after one warmup, rotating direct
Rust, handwritten adapter, and generated Semaprax. Every batch repeats 4,096
copies of the same authenticated scalar export and reports 114,688 borrowed
fixture bytes. Each row records zero adapter copies and zero post-run owner,
view, and string counts. Regex and Url allocation or copy work inside the
foreign implementations remains unavailable.

Run the offline verifier from the repository root:

```sh
python3 examples/ri13-m1-regex-url/measurements/verify.py
```

It binds the raw digest, CSV columns, route/order/sample inventory, the
fixed workload and borrow accounting, zero generated adapter copies and
cleanup state, then recomputes the medians and throughput ratios.

| Task | Route | Median batch latency | Median allocation calls | Median allocated bytes |
| --- | --- | ---: | ---: | ---: |
| Regex scan | Direct Rust | 44.555750 ms | 270,336 | 27,537,408 |
| Regex scan | Handwritten adapter | 45.012834 ms | 270,336 | 27,537,408 |
| Regex scan | Generated Semaprax | 50.007458 ms | 270,337 | 27,537,568 |
| Url parse/view | Direct Rust | 8.402708 ms | 4,096 | 114,688 |
| Url parse/view | Handwritten adapter | 8.475917 ms | 4,096 | 114,688 |
| Url parse/view | Generated Semaprax | 13.174167 ms | 4,097 | 115,264 |

The generated/direct throughput ratio is 0.8910 for Regex scan and 0.6378
for Url parse/view. Generated/handwritten is 0.9001 and 0.6434 respectively.
Both generated routes now have only one additional carrier allocation per
whole batch: 160 bytes for Regex and 576 bytes for Url. The earlier
per-invocation context allocation is therefore removed.

The remaining time is the generated ownership carrier itself. Regex makes
checked C-to-Rust constructor, shared-receiver matcher, and owner-drop calls.
Url additionally makes view creation, view-length, and view-end calls. Each
call validates the private context, generation and lifetime state; direct and
handwritten code call the foreign API without those generated boundary checks.
Removing those crossings would require a new whole-export lowering that
specializes the selected signatures and their cleanup graph. This batch API is
intentionally a held scalar-export repetition seam, so that larger design is
outside this bounded allocation fix.
