# Static initial layout — narrow private successor

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base: `303d6d888b47d8bed4fbe94533b9346167895a89` (immutable).
This cut DOES NOT CLOSE the initial primitive or raw-failure acquisition ABI.

The first size computation was an ordinary `initial_controls()` call returning
usize, a named bytes receiving local, then reserve's bytes parameter. Replace
that runtime helper/local with the compile-time INITIAL_BYTES constant used
straight at the unchanged first physical.reserve. The exact payment roles become
constant argument materialization plus reserve parameter (2), replacing helper
return, named local and reserve parameter (3). Other initial roles are unchanged:
constant work argument/debit parameter (2), checked-sub self/RHS (2), and next
binding (1). Thus the initial inventory has seven usize fields and 42 fields
total; common per-method controls remain32. Layout bytes are still derived from
actual Rust types; no flat byte reduction or guessed numerical gain is inserted.

The new constant is available without evaluating a runtime producer. All physical
and work refusal selection order remains as before apart from the derived
constructor size/work count. Builder/RefCell/Vec/output formation remains after
successful acquisition. No caps, allowances, refunds, retries, unchecked preowned
slots, new wrappers, retained payer or diagnostic conversions are introduced.

Residual, deliberately preserved: Permit::acquire<()> pays its actual unit,
EngineError and Refusal endpoints, not Result<CleanupInventory,InventoryFailure>.
Before its own admission the first reserve still forms primitive argument/self/
Result/match receivers and can publish Kind::Receiving → InventoryFailure → raw
inventory Result. A failed self-payment cannot retroactively fund those objects.
No larger unrelated caller Result is credited against them. Closing that whole
boundary would require a genuinely prepaid specialized primitive/output interface;
this component does not silently invent one. Other per-method acquisition ABI,
schema/supplemental and remaining #781 gaps remain explicit as well.

Source-bound trace is also saved externally at
/Volumes/Semaprax/provenance/raw-inventory-initial-abi-303d-20261016/REPORT.md.
Weighted peer agreed the narrow 3→2 role correspondence and retained residual;
that is not whole initial-ABI or Rust qualification.

Only two source inputs change. source-inverses.json binds their complete 303d
before archives and exact current inverses. The unchanged 303d verifier replays
through those authenticated whole-input views without modifying physical source,
retaining its 12 inverses/eight owners/nine original dependencies. Both original
old builder/callback/checker and the complete two-owner test code are unchanged;
exact pending selectors are copied without additions. Old303d packets are exact.

Run `python3 docs/evidence/static-initial-layout-781-next-20261016/verify-source.py --repo .`.
Source check and targeted rustfmt/diff checks pass. No Cargo, Rust tests, ROOT
writes, global generator, source-authority regeneration, performance benefit,
initial-ABI closure or #781 closure is claimed. Future canonical source composition
must restore current→303d before the protected original raw-inventory input view.
Keep this private and outside the parent's current174 scope.
