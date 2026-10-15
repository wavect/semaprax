# #833: registry-owned identifier census scratch, source-only checkpoint

Basis: `940fcf220e47d1975f757e76f0b3401691db9358`. This packet changes only
three identifier-census vectors, not all temporary private-layout storage.
The current registry authenticates the exact caller, parsed-program slice and
authored-index pointers before the new derivation borrows `self.programs[index]`.
All references therefore have the existing source owner's real lifetime; there
are no lifetime casts, shared/global scratch or serialized authority.

The existing registry header now contains an inline `Option<Census>` inside its
`RefCell`. The original first-preparation `size_of::<Self>()` prepayment includes
this enlarged header. Census backing is lazy, acquired only at the old census
point after the same unsupported-surface checks and first full root traversal.
Every distinct child must first reserve the complete old census backing again,
at exactly the original permission frontier, before borrowing any cached vector.
The prior child's numeric receipt and semantic work ledger grant no physical
permission. The original child owner has retired; no Budget identity, prepaid
spend, allowance or refusal status is cached or transferred. A successful census
then reuses physical backing, clears all three traversal stacks and resets
its private work/ordinal counters. The registry invocation ledger genuinely
pays one selection operation and five clear/reset operations, before those
operations execute, plus one prepaid teardown operation for each acquired buffer
generation. Every old census debit and every root/ownership traversal
still executes. An error poisons the registry and drops scratch; restoring a
budget/work remainder grants no new authority.

Early scratch is dropped after each attempt's final compact forecast and before
core acquisition. Its receipt is never refunded. A later unexpected cold caller
honestly reacquires backing and adds the resulting preparation receipt. A held
hit neither reacquires nor clears anything. This increases retained preparation
overlap and the registry header; peak improvement is not assumed.

The new `derive_with_scratch` additionally carries #834's exact
`let capacity = capacity.min(count);` **after** the unchanged names census,
workspace-length debit and checked full type fold. `capacity-bound-834.json`
binds that line to full archived source from
`b36501d32dd5199dd2ce3a7d45a39484dc9c048c` (observed integrated at `cb7aeb1`).
The original old940 derivation is also retained under the cfg(test) method
`derive_before_scratch`, with only its method name adapted. The complete
independent old registry calls that old method, so adopting atop #834 does not
silently turn the oracle into the current bounded-capacity implementation.
The original complete identifier census is retained unchanged and independently
executed by the old method. The old ownership-index source is unchanged and
archived in full. The old receiver never delegates to scratch reuse. The fresh-child test also
compares the entire old/new census prepayment result, consumed bytes, first
refusal and all diagnostics at zero, one-short and exact backing limits. Empty
successful scratch contains no earlier caller references. Teardown after owner
retirement deallocates existing borrowed-frame vectors and performs no semantic
walk, new allocation or budget acquisition.

Residuals: the layout traversal frame vector and ownership-index fact/frame
vectors still allocate per derivation. A persistent common parent physical
budget owner does not exist on the ordinary early route: each `prepare` creates
a distinct child Budget, and the registry ledger carries semantic work only.
Consequently reservation reuse/savings are blocked pending a genuine parent
owner architecture that preserves all original floors and boundaries. This
packet retains the complete original per-child backing reservation instead of
donating an earlier child's allowance. This partial implementation is not a
whole-issue closure claim. No runtime or application-frequency conclusion is
established.

`pending-selectors.json` resolves exactly seven new individual bodies and eleven
existing individual controls, with full source hashes and module registrations.
It includes distinct/interleaved/held callers, old/current declaration and owner
results, nonzero naming ordinals, full malformed diagnostic values/order,
exact/short/zero bytes and work, nested-floor preservation, foreign and restored
owners, release/reacquisition, and actual allocator/reservation/peak observations
through complete Drop. Zero eligible callers execute the real no-inference
lookup. No production trait derive, quota change, refund, fusion, padding or
assertion removal is involved. Allocator assertions and all Rust bodies remain
**unexecuted**. Their actual results must be reported by the parent batch.

`source-only.py` verifies five complete source inverses, two documentation
inverses, two current child hashes, exact independent old bodies, all new audit
joins, original-work ordering and source-bound unignored test registration.
`static_checks.py` replays the existing 93-source joined purity predicate,
unchanged module budgets and three session mirror predicates. Neither tool
compiles or executes Rust. Formatting was checked only via rustfmt parsing.

Adoption requires additive composition of all five full source owners with
current integration metadata, keeping all prior source archives, global Source36
bindings, dispatcher hooks and other lane proofs. The original production
`derive_with_work` must retain #834's bound in the integrated tree; the separate
cfg(test) `derive_before_scratch` must retain the complete old940 body. This
packet already bounds the new scratch method. Prior global proofs keep their
prior-head meaning; no private checkpoint or source-only proof is relabeled as
current-head executable qualification. #835's ready branch and ROOT were not
modified. No Cargo, Clippy, Rust tests, backend runs, Claude, review or remote
writes were performed.
