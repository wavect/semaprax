# Single-successor path vector reuse

Status: private component evidence; full HIR and hosted qualification pending.

Audience: compiler maintainers and allocation reviewers.

[STD-08 run 38890385964, job 116938849885](https://github.com/wavect/semaprax/actions/runs/38890385964/job/116938849885)
retained all three JSON fixtures and the 67,108,864-byte builder bound. Its
current receipts include a 114,688-byte sequencing-vector reservation with
57,707 bytes left, and later observation/path acquisitions in the other cases.
These are cumulative construction bills, not process RAM measurements.

`sequence_vector` admits one borrowed suffix and an owned prefix vector whose
capacity is exactly the old output's power-of-two capacity. The removed vector
backing must cover the complete new receiving frame. Legacy, selected and oracle
payers retain the original route. The pure admission probe reads headers only.
The caller pays the receiving layout before moving the vector into its receiver.

The receiver replaces one slot at a time using safe mutable iteration. An empty
path owns no backing. Every original clone/transfer, suffix extension, owned
source and continuation calculation remains, followed by the same work debit,
materialization and zero-byte sticky reservation as the old preallocated push.
The vector retains its pointer, capacity and later geometric growth targets.
A failed call drops its owned metadata and publishes no result. Metadata backing
release timing may change; canonical cleanup plans and runtime finalizers do not.
No budget allowance, refund or new source/backend admission is introduced.

`function.json` pins every byte of the complete previous receiver, independently
compiled as `sequence_vector/before.rs`. Source controls bind that whole body
and the single new guarded call in the live function. Original fallback code,
old minimum-array controls and all earlier prefix receivers remain compiled.
Both authority inventories join the complete helper, predecessor and controls.

Authored controls compare the full old/current receiver over short, wide,
empty, branching and stopped path sets; every path field, order, capacity, work
and materialization; exact pointer reuse; complete backing removal and following
growth; zero/exact/short accounts; sticky refusal and complete paid Drop. A
separate full-HIR control retains every real function in the existing fixture.
Expression-path comparisons destructure every field explicitly; the production
carrier has no equality/debug derives, and adding a field must break the
exhaustive comparison until that field is checked too. Component probes retain
that same absence of derives.
Authored controls do not qualify current-main execution. Hosted owning tests
and all full quality gates remain required.

Local component execution retained the complete live sequencing function,
receiver, original predecessor and four actual control bodies. The additional
control preserves the original growth route for excess input capacity. Static standard
library runs observed allocation and complete Drop, including a vector element
layout of 112 bytes and a Diagnostic layout of 136 bytes. Explicit program,
HIR, Work/Budget and flat-DAG adapters remain; these runs do not qualify the
complete compiler or application. Current Rust compilation peaked at 187,712
KiB; the complete finite/future-growth body also passed on pinned Rust 1.88
at 162,032 KiB. Current Clippy checked the complete production helper. The
200 MiB child-tree limit remained intact and sampled system memory availability
stayed at least 75 percent during these probes. The real-HIR control and every
owning hosted gate remain required.
