# OPT844 — c582 source-only checkpoint

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base c58243c6cbe4577efe6cfa81ca6312ff45c1c71a. No Rust, application,
allocation measurement, performance or cap-fit execution occurred here. This
checkpoint is not qualification and does not close OPT844 or OPT808.

Node is Empty, one Movement, one Epsilon, or Many(Box of two Vecs). The authored
64-bit layout assertion is 32 bytes versus the old 48. Label remains the exact
original two-variant type, and usize targets preserve even hostile usize::MAX.
A first edge pays its new Node enum initializer at the original admission
boundary, then stores it directly. No helper Result/forwarded receiver is added
on that path. Node vector growth charges the actual new layout, not an old bound.
A second edge acquires promotion controls, actual Box backing, both Vec headers
and all required backing before moving the old slot. Later Many insertions pay
their new payment Result and both sets of Box/Many pattern references, retaining
the entire original reserve_vec path. Old work, order, duplicate weights and
checked arithmetic remain unchanged; no refund, donor budget or fee credit exists.

Borrowed read methods replace the old Vec's borrowed slice/Deref view. They
return no owned edge, iterator, RefCell guard, Option, Diagnostic or Result.
The caller's existing iterator/index carries the same borrowed elements. Labels
are never reconstructed or Rc-cloned by those views. Many insertion controls
are separately paid and must not be confused with these immutable views.

The full old Node, Automaton and all builder/comparison bodies compile over
their original two Vecs. The old canonical algorithm is independently compiled
inside canonical_shared::before_nodes so it can use the unchanged original
numeric/payment helpers without widening their private APIs. It imports the
unchanged original Label because Dictionary's Key implementation is nominal.
The original borrowed comparison, strategy and storage receiver bodies are also
retained. Only module/import/test-registration/visibility adapters and formatting
are projected; complete originals and full inverse pairs are retained locally.
No old assertion body is removed. Existing readers use explicit borrowed views;
the three hostile strategy mutations now use the actual constructor API.

Eight exact pending bodies include all 216 three-node forward graphs, mixed and
parallel edge multiplicity, u64 overflow, full builder semantic/work equality at
every frontier and a later malformed edge, old/current Automaton and whole
comparison observations through final Drop, exact/short/zero reservation,
unfavorable many-edge cases, first/second backing failure during promotion,
complete six-field first/refusal retry behavior, nested floors and full source
custody. Actual old/current metrics must be reported separately for every case;
only the 127-edge singleton case asserts a funded/physical improvement. Those
assertions are unrun and must remain unchanged if they fail.

The allocation observer surrounds Automaton/ReplayBudget or the complete
compare receiver from construction through Drop. The identical pre-existing
with_limit payer bootstrap and fixture preparation are outside both probes.
Synthetic-chain results are not whole-comparison or application gains.

The three new JSON runtime inputs source-inverses.json, new-owners.json, and
old-receiver-adaptations.json become immutable at this checkpoint. Future heads
need strict independent whole-source views through compact844_input; they must
not rewrite these expected hashes. Static proof replays the unchanged preceding
full source proof in its authenticated c582 source and namespace view. All seven
new Rust owners join all three full-source audit unions; the 1500-line cap stays.

A later public 7a87 head contains independent MAIN and OPT831 carrier changes.
Comparing this c582 full old canonical receiver against that merged head would
be a COMPOSITE comparison, not an OPT844-only gain. Before final attribution,
advance to that head and retain a complete immediate old receiver alongside this
immutable checkpoint. The isolated Node pair itself has no OPT831 dependency.
