# Sequencing capacity from the first active prefix

Status: implementation and authored controls; hosted qualification pending.

Audience: compiler maintainers and allocation reviewers.

[Current STD-08 job 117013997542](https://github.com/wavect/semaprax/actions/runs/38912995873/job/117013997542)
failed three unchanged JSON fixtures under the 67,108,864-byte builder bound.
HIR final validation refused a 320-byte ordinary observation suffix with 221
bytes left, a 57,344-byte expression-path vector with 29,562 bytes left, and a
229,376-byte expression-path vector with 226,497 bytes left. These cumulative
reservation receipts describe construction backing, not process RAM.

The original sequencing loop emits one path for each stopped prefix and one
path per suffix for each active prefix. With P prefixes and S nonempty suffixes,
an active first prefix therefore proves at least P + S - 1 output paths. This
bound uses the same failed/residual flags as the unchanged loop and reads no
other prefix. It does not inspect or reinterpret cleanup observations.

The old initial minimum has capacity C = next_power_of_two(P). The additional
bound may select D = next_power_of_two(P + S - 1) only when D exceeds C. Every
successful original execution reaches D. Acquiring D directly skips C and any
intermediate geometric replacements, preserving the final capacity, future
growth, field order, clones, work debits and materializations. Exactly D - C
allocated element slots disappear. The skipped backing alone must cover the
complete additional typed receiving/probe/result layout; the original minimum
fee remains paid and supplies no credit.

Small, unpaid, selected, oracle, stopped-first, overflow, nonempty-destination
and no-saving cases retain the original minimum. No bound, reservation, work
allowance, refund, source fixture or verification route is weakened. Only
compiler metadata allocation timing changes; canonical cleanup plans and
runtime finalizers are unchanged.

The complete previous sequencing receiver has fixed SHA-256
`fd54e2c120585d772809e8d0cc8f1c9ea4fda1aff898239c27cd05a9556ccae7`
and remains independently compiled in `path_array/sequence/before.rs`.
The new source control permits only the reviewed minimum-call replacement in
that whole function. [source-controls.json](source-controls.json) retains both
complete earlier source controls, their fixed original hashes and exact
additive compositions; all earlier subjects and assertions remain checked.
Both authority inventories join the complete new helper, predecessor and tests.

Authored controls compare every expression-path field, order, capacity, work,
materialization, future growth and complete Drop against the full predecessor.
Finite controls retain zero, exact, one-short and sticky-refusal accounts and
measure the removed physical backing separately from the receiving fee. A
separate real-HIR control retains every fixture function. None of these authored
controls claims hosted execution. Full current-main CI and quality gates are
still required.

Local execution passed all five direct component/source control bodies with
the complete live and previous receivers, helper and original minimum. Static
standard-library runs used the actual allocation observer, ledger and
Diagnostic cores, with explicit thin HIR, Work/Budget and flat-DAG adapters and
112-byte expression-path / 136-byte Diagnostic layout assertions. The largest
current Rust compilation used 202,352 KiB. The complete finite-budget/Drop body
also passed on Rust 1.88 (193,296 KiB compilation); the full production helper
passed current Clippy with warnings denied (133,104 KiB). The unchanged
200 MiB child-tree guard held execution during low global memory availability.
These component results do not qualify the separate real-HIR control, compiler,
C++ package replay, applications or full current-main CI.
