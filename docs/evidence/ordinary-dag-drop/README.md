# Ordinary DAG destruction source packet (#819)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This private source batch prepays ordinary destruction backing before node
publication. No Cargo, compiler, test binary or Clippy was executed by this lane.
Authored allocation counts, bounds and selectors are pending runtime evidence.

Every ordinary node retains a checked immutable reverse-pop pending bound:
`max(children.len(), max_i(i + B(child_i)))`; a sequence with a prefix uses
`max(1, B(prefix))`, and an empty sequence uses zero. Work retains one reusable
Rc scratch header and a fallibly acquired pending Vec. Growth charges the full
new allocation while the old allocation remains live, with no refunds. An
allocator capacity disagreement refuses publication. A scratch borrow is
released before incoming kinds can be dropped on error.

The existing replay work budget prepays the original construction ordinal plus
one child-bound fold and future pop per edge and one future node-release event.
The empty four-member union therefore has a distinct literal eighteen-unit
control; the complete independently compiled original five-unit receiver and
its assertions are unchanged. This deliberately changes ordinary low-work
admission and supplies no new quota. No application-fit or causal savings claim
is made.

The production Drop implementation is byte-identical to the original. The
independent saved module contains the complete old node, kind, destructor,
observation carrier and ordinary constructors. Its entry initializer reuses
the unchanged production entry initialization outside observation; it is not
an independent whole entry constructor. Full old receiver text is retained;
its only projection is the explicit import selecting the saved types. Existing
storage/tail receivers additionally release the new Work-owned scratch inside
their allocation observations, with fixed two/four insertion-count controls
and whole original-source equality after those exact insertions are removed.

The destructor preserves try_unwrap/drain semantics, root-tail-last and inner
tail-before-descendant order, retained Weak validity, shared external owners,
duplicate DAGs and foreign work/ledger children. Cat/Alt member allocation
release timing differs physically: old root member storage doubles as a
growable pending buffer, while the new root carrier is retained until field
Drop. A separate actual pointer-order controller records that difference and
compares semantic tail order; it does not claim full allocator-event identity.
The fixed test-only free-order recorder allocates no backing and never changes
the original allocation counters. Missing, excessive or unknown watch pointers
explicitly fail or invalidate the fixture.

Selected construction retains its existing session admission and work policy.
Its immutable cumulative registered-edge count bounds every live descendant;
ordinary parents pay all conservative excess capacity for foreign selected
children. This is not an optimum-capacity claim or selected-route activation.

Compatibility boundary: standalone legacy nodes and refused incoming legacy
kinds still use their original allocating destructor. A successful paid parent
can drain legacy children with its frontier, but failure before that parent is
published does not make legacy destruction paid. The dedicated refusal control
keeps this allocation visible. No aggregate funding or #819 closure is claimed.

`pending-selectors.json` binds exact source-authored controls; source-registration
checks and hashes are static evidence only. Root owns the future grouped build
and runtime qualification. Original snapshots and original numeric assertions
must not be regenerated to match current source.
