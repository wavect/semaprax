# Unique singleton ownership-state move

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only successor of `364d824194cd1df76b5c273aa0fb9cbafb602e29`.
No Rust, Cargo, test, benchmark or allocation execution occurred. No numerical
reservation recovery, application fit or issue closure is claimed.

The one incoming boxed state has already passed the original complete control
group validator and original `join_units` work debit. Its live_order and
conditional_variants are therefore exactly the first/only values that the old
joined_ownership_state cloned. The new route consumes that same boxed state
before normalization. live_order stays owned by it. conditional_variants moves
through the unchanged BranchJoins::normalize producer and is received into the
same field. No empty placeholder Vec, tuple, wrapper, capturing closure, Result
or authority is added. Partial normalization failure drops the moved input and
remaining state fields through their real original owners; PathState has no
custom Drop implementation.

Removed actual constructors are the singleton joined_ownership_state iterator,
first-reference/helper Result and both vector clones, including conditional
variant nested identity/case containers. The removed reserve_ownership_clone
call prices only those physical clones and contains no work charge. It is not
charged on the route where those clones do not exist. No surviving fee is
removed or treated as credit. Original incoming/state/Box ownership and funded
backing remain retained through Drop. Normalization still performs every root
comparison, branch classification, self-join, work charge and backing payment;
its Result/arguments are unchanged. All actual lexical checks, transitions,
exits, edge insertions and successor scheduling remain byte-exact. The complete
multistate grouping and joined_ownership_state helper remain byte-exact.

`before_state_move.rs` is the complete immediate364d receiver, with only sibling
module registrations removed, using the same unchanged funded dependencies. Its
own singleton still calls its own original joined_ownership_state and clones the
original fields. It never dispatches the new validator. Full original root bytes,
registration-only adapters and one strict current→Git364d inverse are retained.
The two new tests compare whole independent old/current replay, complete six-field
hostile diagnostics, exact work and unchanged plans, and whole allocator/Drop and
zero/exact/one-short frontiers. Improvement assertions are authored, unexecuted.
Existing three singleton and three normalization controls are listed separately.

`verify-source.py --repo <checkout>` checks complete current SHA→unique edits→
Git/archive equality, the full independently compiled old body, exact multistate
and all other production spans, unchanged storage/normalization/boxed dependencies,
old tests, two new owner hashes and source-size limits. Its source-only result is
`source-proof.json`.

Protected runtime input: `current-to-364d.json`. Canonical composition owns the
strict inherited raw→364d front before old root readers, plus all three complete
raw audit joins and authority classification for both new children. New owners
are metadata-only hashes in `new-owners.json`; no self-referential digest exists.
Old immutable runtime JSONs/assertions are untouched. No global helper/audit file
is edited in this disjoint component. `pending-selectors.json` contains two new
lib owners and six existing directly relevant controls; final registration is
required against the actual merged frozen source.
