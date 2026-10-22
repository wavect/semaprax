# Actual receiving correspondence

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The bank is an actual-layout inventory, not a forecast of available allowance.
Every successful bank/work debit remains spent on later failure. The initial
metadata acquisition ABI itself is explicitly unreceived on failure; successful
output/control admission does not retroactively fund its failed primitive call.
The complete original Diagnostic builder/callback remains separate.

Each method's common tuple has 32 fields:

- 2 `Result<T, InventoryFailure>`: method returned and caller receiving values.
- 2 InventoryFailure and 2 Kind: opaque constructor/receiving and inner closed
  constructor/receiving roles, preserving their actual full inline layouts.
- 2 `Result<(), PhysicalRefusal>`: new bank primitive return/match receiving.
- 2 `Result<(), InventoryFailure>`: metadata debit return/match receiving.
- 8 usize: amount function return, named bytes local, reserve bytes parameter;
  constant WORK argument and debit units parameter; checked-sub self/RHS; next
  pattern binding. Writing next into existing caller-owned remaining creates no
  new remaining-field constructor. WORK is a const, not another runtime local.
- 2 Option<usize>: checked-sub return and match receiving.
- 2 &RefCell<Ledger>: initializer and borrow_mut receiver.
- 2 RefMut<Ledger>: returned guard and named held receiving.
- 1 &mut RefMut and 1 &mut Ledger: deref-mut receiver/returned ledger borrow.
- 1 &mut &mut Physical and 1 &mut &mut usize: destructured field bindings.
- 2 &mut Physical: added bank caller reborrow and method-self receiver. The
  original later primitive reserve still exists; it is not credited as removed.
- 2 &mut usize: metadata debit caller reborrow and callee parameter.

The initial 43-field bank has two root inventory Results, two opaque failures,
two Kinds, one finite Reason, both bank/debit Result pairs, the same eight scalar
and two Option roles, two physical and two remaining references, two each
Builder/Ledger/RefCell, initializer+field pairs for each of four root Vecs, two
CleanupInventory and two CleanupEntryState aggregate roles. Its lexical match
eliminates the unnecessary is_err method borrow and bool; no such role is priced.
It precedes forming Builder, Ledger, RefCell, Vec and output aggregates.

Additional local leaf error initializers use MAX of their actual alternative
layouts and ONE initializer, never a tuple sum for mutually exclusive leaves.
clone_owned can construct overflow Reason OR physical Clone; needs_drop can
construct unauthenticated-Option Reason OR lexical Profile/TypeDrop; the first
such local error immediately returns. Nested charge/probe/helper failures carry
an already-formed InventoryFailure and their own separate banks, without a
second caller-local leaf initializer. selected shape/leaf/lifecycle and pattern
methods follow the same explicit terminal rule. No old heap or layout is credited
against these fees, and no debit is refunded.

CONTROL_INITIALIZERS=32 plus the actual local one-shot leaf initializer, and
INITIAL_CONTROL_INITIALIZERS=43, are additional explicit constructor work. The
original logical callback sequence is retained separately: a non-measured trace
compares every actual original work call left-to-right. Its heap trace is enabled
only for that pass; a const drop-free TLS bool prevents accessing trace storage
in physical observation windows. A separate allocation-free metadata counter
records only successfully debited new initializer work.

Receiving/alternative peer review is finite, not full constructor qualification:
/Volumes/Semaprax/provenance/raw-inventory-receiving-refinement-review-20261016.json
SHA256 fcf89581cd8d4c9a068cc7c797e5415bdc940b184d5873d94c4acb4e7fff9f7a.
No Rust execution or favorable cost claim follows from this source review.
