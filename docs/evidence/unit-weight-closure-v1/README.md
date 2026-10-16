# Reuse the complete unit-weight closure

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

Hosted STD-08 job
[116690924082](https://github.com/wavect/semaprax/actions/runs/38812864014/job/116690924082)
advanced past both previously failing testing consumers, then hit the unchanged
64 MiB builder bound in five JSON cases. One refused reservation was the
`<usize, u64>` node used to rebuild a closed weighted map in
`canonical_shared::successors`. The newer
[2691824df job](https://github.com/wavect/semaprax/actions/runs/38816116869/job/116701836061)
confirms the same five JSON budget failures and preserves those testing passes.

For exactly one input `(node, 1)`, that producer inserts each unique key from
the node's complete immutable closure with `routes * 1 + 0`. The result is the
existing map, including zero and `u64::MAX` route counts. The receiver now keeps
that already-paid Rc instead of constructing another map. It preserves every
old route-addition work call, in order and with the same phase, then runs the
entire old successor grouping, checked weight accumulation and vector producer.
All other input shapes retain the full general producer, including duplicate
roots, non-unit weights, overflow and empty input.

Both new Option constructions and the borrowed receiving view are prepaid.
The unit receiver separately pays its Rc parameter and complete Result carrier,
then authenticates the live payer even when no backing is allocated. It moves
the existing Rc without cloning its contents or adding an Rc allocation. No
ceiling, work allowance, cleanup-plan order or authority changes.

The complete previous module is retained as `canonical_shared.before.rs.txt`,
SHA-256 `a3ce7638daaf633d17e121f51028875b733d77e8c38464699c83700588003ca6`.
`before_unit.rs` independently compiles the whole previous successor method;
only its name and visibility change. A private scoped test switch runs that
method through the complete unchanged canonical receiver. `inverse.json`
restores the entire current module byte for byte. Existing full-old receivers,
algebra, depth, physical-budget and allocation tests remain intact. Both source
audit inventories include every new child module.

Local bounded probes execute the actual complete old/new successor methods,
new receiver, bounded-output ledger, Diagnostic, node prices and allocation
observer. Synthetic immutable closure/NFA/Label inputs and a physical-budget
adapter isolate this component. Cases compare complete outputs, every work
request and supplier lookup, unit/general/absent/duplicate/zero/max/overflow
inputs, exact/short/zero work, reduced requested backing, exact/short/zero
physical limits and final Drop. Rust 1.88 and 1.97 passed with sampled aggregate
compiler peaks of 179,840 and 149,120 KiB under a 200 MiB child-tree stop guard.
The actual whole-source inverse Rust test also passed. These results do not
qualify the full HIR, memo ownership or hosted JSON gate.

Additional real-HIR tests compare complete receiver ids, signatures, states,
work and cycle errors with the previous producer, and check whole-lifetime
allocation and exact physical-budget boundaries. Those tests require CI. Fresh
hosted evidence owns any graph-budget pin update; no bill or digest is guessed.
