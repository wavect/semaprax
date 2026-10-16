# Paid boxed ownership path states

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

The production STD-08 job
[116660444540](https://github.com/wavect/semaprax/actions/runs/38803872053/job/116660444540)
refused a 4,520-byte incoming ownership-state tree-node reservation during final
HIR validation. The builder limit remains 67,108,864 bytes.

The path frontier and its control groups now hold `Box<PathState>` keys. Their
ordering and equality are the complete state values. New states prepay both the
unchanged conservative pointer-key node upper bound and `size_of::<PathState>()`
before either allocation. Regrouping transfers the existing paid box; it pays a
new node without cloning, refunding, or charging another box. Nested owned
backing retains its existing entry/branch/clone payments. Duplicate insertion
also authenticates the live payer before returning.

The traversal, complete sorted-prefix join checks, branch normalization,
transition/exit replay, and semantic work charges remain in place. This changes
the physical builder debit and therefore requires fresh hosted browser graph
captures; no new bill or graph digest is guessed here.

`path_join.before.rs.txt` is the complete original module, SHA-256
`bcabe39f9158261ce682392252b437ea2e8d23cec3c68cd379a2d18ce50aad69`.
The compiled test-only `before.rs` removes only three module registrations.
`inverse.json` restores the entire current module, byte for byte, to that
original. The owning Rust test verifies the hash, compiled body and inverse.
Both receivers use the same unchanged branch and storage dependencies.

The actual source inventory adds just this new test module to the existing H006
diagnostic paths. `catalog-receipt.json` records the authenticated old/new
catalog and installed bundle digests; `bundle.before.json` retains every prior
byte. The source test reconstructs that entire prior bundle after restoring
only its catalog identity and envelope digest. The code count remains 1,513.

Local isolated probes run the actual generic constructors, actual physical node
pricing, actual bounded-output ledger and actual allocation observer on Rust
1.88 and 1.97. They cover exact/short/zero/sticky refusals, non-Clone pointer
transfers, deduplication, 127 keys with tree splits and every ordered prefix,
and last Drop, including a refused held-box transfer. Latest sampled aggregate
compiler peaks were 179,664 and 181,616 KiB,
under a 200 MiB child-tree stop guard. These are component results. The new
whole-HIR old/current replay, hostile duplicate-initialization, full physical
budget, and boxed history-prefix tests require CI; they were not executed in a
local full compiler build. Existing production budget fixtures remain intact.
