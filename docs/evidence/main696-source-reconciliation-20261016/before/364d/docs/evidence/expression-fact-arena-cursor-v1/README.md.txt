# Paid expression-fact Arena cursor

Current-head CI run 38770620786, job 116546107241, located the actual std/test-bytes
builder refusal at `expression_facts/capacity.rs:32`: its original factory
requests its complete `FactCapacity` plus a 514-entry expression-frame array.
The live non-test 64-bit request was 32,920 bytes. The static forecast admitted
the unchanged five-module fixture; the cumulative 64 MiB live payer refused.
This is the paid Arena route used by `expression_facts::with_budget`.

The paid Arena now acquires an `ArenaCapacity` containing the complete original
base/type cursor and its own expression-frame Vec. Its depth ceiling remains
514. Each replacement is fully paid before allocation, including receiving
controls and actual slot copy/initialization work. Old backing stays charged
until normal release. Requires/body/ensures reuse only this same held owner;
reuse authenticates the payer's sticky state, including a closed Oracle epoch.
No arbitrary caller Vec, quota increase, refund, price credit, global cache,
verification shortcut, traversal reorder or payload discount is introduced.
Deep walks can cost more cumulative backing than the previous fixed array.

The complete original factory remains unchanged. A complete original Arena
entry and original fixed-array producer remain independently compiled. Full
old capacity, Arena and producer source archives are SHA-bound; only the named
constructor dispatch, test registrations and cursor-control substitutions are
inverted. The original operation/payload dispatcher and existing legacy-source
custody gate remain intact. Additional source tests bind the complete new owner.

The isolated SSD probe executes the actual new owner, actual payer methods,
actual PhysicalReservation, actual Oracle epoch core and actual bounded-output
core against synthetic HIR/frame carriers. Both Rust 1.88 and 1.97 Oracle
layouts, and the production-payer layout, pass all growth, exact/one-short/zero,
copy work, overlap/release, same-owner reuse, exhausted-work and stale-epoch
controls. These are owner-protocol checks, not full-HIR or application-fit
qualification. Owner-probe compilers ran sequentially with a 240 MiB RSS cutoff; sampled
peaks were below 140 MiB. The separate source-custody and isolated Clippy owner
checks pass below 160 MiB.

Actual-HIR tests compare every original/current fact and error, require the
same raw-census 514/515 boundary, observe release and lower shallow-owner cost,
and exercise the unchanged builder route's existing controls. These await
hosted CI, as do std/test, std/test-bytes and the full quality profile. The
compiler's graph budget bills will change; browser pins require fresh hosted
whole-wire records before any subsequent update.
