# Supplemental receiving cursor growth

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

The hosted STD-08 job `116607589314` on `6dec6f011` still refuses 24,672 bytes
with 6,103 left in its unchanged 64 MiB builder ledger. Caller propagation
identifies `ordinary_inventory/storage.rs:10`, whose fixed 1,028-element array
is requested by `supplemental/receiving.rs::Scratch::new`. Its Frame contains
one expression reference, one index and a variant tag. This is the actual
receiving allocation, distinct from the earlier same-sized structural call
cursor. Snapshot testing also refuses a separate 4,520-byte path-join tree
request; wrapper caller propagation now exposes that next requesting receiver.

Scratch now pays its inline owner plus two initial frames. Each deeper pending
stack reserves a complete exact replacement and two vector/frame controls while
old backing is still live. Moving live continuations consumes the same work
ledger. Growth is bounded independently by the original 1,028-frame maximum.
The complete original receiver and constructor remain independently compiled;
the source control reconstructs every original byte after only declared
storage substitutions. Parameter resolution, type/drop checks, shape and flag
derivation, storage identities, receive order and both traversal errors remain
unchanged. Shallow root walks retain the existing zero-allocation reuse test.
No program, quota, diagnostic, fixture or cleanup vector is weakened.

Local focused probes used the actual new constructor/growth helper, complete
original constructor, actual production ReplayBudget methods, physical ledger,
bounded-output payer and allocation observer with synthetic HIR carriers.
These carriers do not qualify HIR classification or the full receiving walk.
Rust 1.88 and 1.97 passed all eleven growth frontiers, live-frame preservation,
exact full funding, every one-short frontier and sticky reuse, shallow
reduction, complete source custody, original constructor cost, relocation-work
refusal and the independent 1,028/1,029 storage boundary. Observed backing is
3,074 Frame slots cumulatively, peak overlap 2,052 slots and zero final live
backing. Exact nonempty relocation work is 2,046 moved frames. The first draft
probe had only scratch-harness visibility/missing-helper errors; correcting
those made both compilers pass. Aggregate compiler peaks were 179,920 KiB and
191,904 KiB. Isolated Clippy with -D warnings passed at 200,864 KiB, using the
repository's existing result_large_err allowance and scratch-only unused
code/import/macro allowances. All compilers ran sequentially below the
200 MiB sampled cutoff; no full root build ran.

The authored real-HIR gate compares every received slot, full type, liveness
shape and flag count with the complete original receiver over repeated
requires/body/ensures sequences. It and the complete STD-08, installed journey,
backend, MSRV and unit gates remain pending hosted qualification. Deep walks
reserve more cumulative bytes; no full application-fit claim follows from the
small storage probes. Changed successful graph bills require actual hosted
captures and complete historical reconstruction before any pin refresh.
