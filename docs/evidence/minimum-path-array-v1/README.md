# Minimum ownership path array capacity

Status: private component evidence; full HIR and hosted application qualification pending.

Audience: compiler maintainers and allocation reviewers.

Hosted STD-08 job `116744131720` and task-service job `116744132222` refused
ownership replay backing under the unchanged 67,108,864-byte builder limit.
Four result-producing receivers have a known minimum output length: append,
status split, contract split, and sequencing with a nonempty suffix frontier.

For an empty destination and minimum length M of at least eight, the constructor
prepays its complete receiving layouts and acquires C, the next power of two
at least M. The old paid push loop acquired capacities 1, 2, ..., C. Since the
complete output has at least M elements, both routes reach C and then the same
later capacities. The removed replacement arrays total C - 1 element slots.
There is no refund, forecast credit, changed ceiling, skipped path, new work
allowance, or changed observation representation. Every old push and work event
remains in its original loop. Tiny, nonempty-destination, and unobserved routes
keep their original backing behavior.

`functions.json` preserves all four complete old function bodies and hashes;
the test-only `before.rs` compiles them with only explicit parent visibility.
The named receiver custody packet also reverses every change across the whole
replay root and both live source inventories. Both inventories include every
new production, saved receiver, and test module.

The local component probe compiled the actual constructor, append and push
loops, physical growth, bounded-output ledger, diagnostic and allocation
observer with thin budget/work adapters and a synthetic 112-byte path carrier.
It passed contents/order, final and future capacities, work/materializations,
exact removed backing and new metadata fee, tiny/empty/nonempty/legacy routes,
zero/exact/short limits, and complete Drop. The compiler peaked at 148,000 KiB
under the 200 MiB aggregate process-tree stop guard. Real HIR tests are authored
for CI; this probe does not qualify HIR, oracle or application fit.
