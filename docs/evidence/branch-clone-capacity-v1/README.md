# Branch clone capacity

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

Hosted [STD-08 job 116714584223](https://github.com/wavect/semaprax/actions/runs/38819916075/job/116714584223)
retains the testing consumer passes but refuses five JSON consumers at the
unchanged 64 MiB builder limit. Two refusals occur at the observation push
immediately following a branch-history clone.

The paid clone now reserves the exact capacity that the next push previously
acquired. It retains the complete clone census, nested payload and oracle work,
then pays the additional slots before allocation. The next push uses that
backing. Contents, capacity after the push, subsequent growth, path order,
work and materialization counts are unchanged. The removed intermediate backing
is exactly the old history length times its element size. Unobserved legacy
execution keeps the original helper. No ceiling or diagnostic is weakened.

The three archived functions preserve the complete original enumerator, clone
and push bodies. A source-custody test pins each archive hash and matches the
whole current function, allowing only the one clone-call substitution. A private
scoped switch runs the entire original enumerator. Both source audit inventories
include the new production and test modules.

Sequential local component probes passed on Rust 1.88 and 1.97, with sampled
aggregate compiler peaks of 163,888 and 135,728 KiB under a 200 MiB stop guard.
They compile the actual complete clone/push helpers, new constructor, payload
pricing, physical growth, bounded-output ledger, diagnostics and allocation
observer with a thin budget adapter. Cases cover widths 0 through 64, one and
three pushes, legacy and paid routes, exact contents and capacity, work and
materialization equality, exact allocation savings, zero/exact/short physical
limits, work limits and complete Drop. Source hashes and whole-function
matching also passed locally.

The real-HIR enumeration and allocation tests are authored for CI. The local
adapter does not qualify full HIR, clone-cursor or oracle integration; hosted
JSON and installed-journey qualification remain pending.
