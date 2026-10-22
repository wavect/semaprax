# Bounded receiving-layer review

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

One9899-byte tool-disabled Sonnet source review completed53.8066seconds,
peak317472768bytes, exit0, listcost0.08264USD; actual billing unknown. Raw
payload/verdict/receipt retained unchanged, no retry or Rust execution. No
provable blocker was found in the pasted successor body. Conditional findings
are resolved below; this is source evidence, not runtime improvement approval.

Weighted is exactly Vec<(usize,u64)> (factored.rs808); SharedWeighted is its Rc.
physical_clone::rc_backing_bytes is size_of the actual two-counter RcBacking<T>
with T=Weighted, unchanged from own_weighted. Both One and Many therefore own
same typed backing; the tuple buffer is separately acquired. Label clones only
existing Rc<SkeletonObservation> or Copy SkeletonTerminal, creating no backing.

Common finish controls remain real on Many: weighted_successor still initializes
its Vec and receives ordered tuple values; its Rc initializer and actual caller
held Rc are distinct from the remaining Result receiver. The explicit Many map
argument/Result fee remains before that fallible helper; no Result fee is erased
while a real receiving layer remains. Promotion retains both existing per-new-
entry reserve_btree_node requests under the unchanged upper-bound model; no node
count, work unit, admission or allowance is discounted. Both requests still
precede either insert and leave One unchanged when either fails.

reserve_array(0) authenticates reserve_bytes(0) on the paid route. Unobserved
semantic controls retain their preexisting no-physical-charge behavior. Empty
route now truly skips constructing the owning IntoIter, Option tuple and tuple;
its retained map/control fees are not credited back. Nonempty iterator retains
all three full receiving layouts.

Raw reviewed One used Vec::with_capacity(1) +push. Final local successor instead
uses Vec::from([tuple]) with an additional separately prepaid full array
initializer; it avoids Once and a possible vec-init-then-push lint. There is no
claim that the array replacement alone saves bytes. Tests additionally inspect
actual capacity==1 for each singleton memo state. The raw review payload is not
misrepresented as reviewing that later small edit. Rc backing remains acquired
before Rc::new; failure after already-paid Vec creation drops that Vec without
further acquisition, same lifetime order as the previous own_weighted call.

Four new authored controls target ACTUAL production successors, not obsolete
helper-only frontiers: complete immediate831 signatures/states/six-field/work
and overflow differential; real before/current charged/allocated/peak/whole Drop
tuples; every byte cut including zero/promotion/vector/Rc/Many receiving; full
original helper/receiver/seven-control source equality. All seven original
owners remain unchanged+required, including competing Many overflow-before-cycle
and foreign/function/budget/restored/stale context. New/old physical tuples and
strict intended paid reduction remain unexecuted until parent qualification.
