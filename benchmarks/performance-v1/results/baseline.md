# Performance macrobenchmark baseline

- Status: no measurement recorded
- Audience: compiler contributors and performance operators

No baseline wall times are committed for this suite.

The file this document renders,
[`baseline.json`](baseline.json), holds an empty scenario list. It is not a
measurement, and nothing in this repository should be read as one. The previous
version of these two files carried a host string, a `rustc` version and a commit
id with zero scenarios behind them; that is provenance without evidence, so it
was removed rather than filled in with numbers from a loaded shared machine.

## Recording a baseline

Record on an idle host, from a clean checkout, and commit both files together:

```sh
python3 benchmarks/performance-v1/run.py \
  --output benchmarks/performance-v1/results/baseline.json \
  --markdown benchmarks/performance-v1/results/baseline.md
```

The runner records the host it actually ran on, the selected binary and its
digest, the commit and whether the tree was dirty, the load average at the
start, and every scenario's authenticated input digests. Read
[`../docs/METHODOLOGY.md`](../docs/METHODOLOGY.md) before treating the result as
comparable evidence: wall times are advisory local evidence for one host and one
build, never a hosted, release or cross-platform claim.
