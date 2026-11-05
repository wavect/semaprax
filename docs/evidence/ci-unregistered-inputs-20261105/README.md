# Complete historical ingress for eight unregistered sources

Status: Local repair in progress; current original CI remains authoritative.
Audience: Compiler maintainers and CI reviewers.

Eight sources outside the immutable canonical membership changed after older
whole-file readers were sealed. Their original request tuples, lengths, digests,
full predecessor files, downstream inverses and hostile controls remain intact.
The new test-only ingress recognizes only the eight declared source/digest
pairs. It authenticates the complete compiled input, a fresh regular physical
file and its independent current archive before applying the exact inverse.
Every predecessor byte is compared with its independent archive. Unknown names,
digests, lengths and foreign, truncated, duplicate or changed current bodies
cannot select this ingress. Already projected inner stages retain their existing
route. The original fourteen-entry CI input table is unchanged.

The byte-operation predecessor is the complete module from 2d87a3b892, before
the three borrowed read kernels. Their current implementation and backend
contracts remain present and independently tested. The line-reader predecessor
retains its entire ordinary while/Option control; the current bounded kernel
also retains ordinary execution and the closed selected-purity check. The three
public wire-test predecessors retain every original literal and assertion.

This fixture grants no compiler, ownership, budget, publication or reuse authority.
It does not change a production source, source inventory membership or limit.

```sh
python3 docs/evidence/ci-unregistered-inputs-20261105/verify.py
```

The complete ordered receiver predecessor also retains its original field access
spelling, while the current owner uses the shared immutable accessors. Both whole
files are independently archived; the original complete comparison still runs.
