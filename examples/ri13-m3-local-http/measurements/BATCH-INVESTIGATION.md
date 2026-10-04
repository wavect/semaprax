# RI-13 M3 batch regression investigation

The 64-operation batch receipt at the reviewed local checkout recorded 245.80
generated operations per second and 4,890.84 handwritten-adapter operations
per second. Its 0.0503 generated-to-handwritten ratio is below the recorded
0.90 investigation threshold. This is a local measured regression, not a
support or throughput claim.

Render a bounded investigation from an actual receipt and the exact measured
source file:

```sh
python3 examples/ri13-m3-local-http/measurements/investigate_batch.py \
  --receipt /path/to/ri13-combined-real.json \
  --measure-source examples/ri13-m3-local-http/src/bin/measure.rs \
  --output /path/to/ri13-batch-investigation.json
```

The renderer checks the exact 64-operation, two-byte workload and all three
route names, binds the receipt and source SHA-256 values, and derives only
throughput ratio and allocator-request differences per operation. It also
checks the current source shape: the batch loop calls `execute` per operation,
and the generated branch performs a one-shot `register` plus consuming
`call_typed` per operation. `call_typed` prepares a checked source future for
that invocation, so the present API does not amortize setup across the 64
operations.

The evidence does not measure how much time is spent in registration,
checked-prefix evaluation, program cloning, callback creation, or future
resume individually. The source observation explains the repeated setup shape;
it does not allocate an exact portion of the measured gap to those operations.
Changing the one-shot callback/registration contract requires a separate
semantic design and measured rerun, because it protects each invocation's
revision and callback binding.
