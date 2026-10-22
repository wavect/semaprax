# OPT839 held authentication successor — source-only

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base is exact `2b5cae170e13b16c4a5f57a619b302405e2934a6`; the protected
`59976776de8d6c03eccc2415d5c06636b00f8b7d` predecessor adapter is a dependency,
not another feature pick. No Cargo, Rust discovery, build, tests or Clippy ran.

The warm branch in `memo/retained.rs` exists only after controls are paid and a
live binding exists. It reads the held ledger, compares exact function/budget
addresses, and then calls the unit gateway in
`physical_budget/authentication.rs`. The held `Option<Diagnostic>` is neither
reinitialized nor cloned. Failure constructs one Diagnostic directly in its
paid slot and publication moves its six fields once; later terminal retries
retain the original empty sentinel. The cold ordinary Memo path is unchanged.

The gateway preserves LegacyUnobserved, ActiveBuilder, SelectedReservation and
test Oracle zero frontiers. Active/selected still authenticate the reserved
floor through reserve_active_required(0). The narrow physical bool leaves
charged/remaining unchanged and stores only the same enclosing first refusal.
Oracle unit predicates retain exact attempt/counter-overflow, sticky, expired
and first-receipt behavior. Full warm authentication makes fewer real requests;
its ordinal is not padded to impersonate the old positive Result payment.

Constructor inventory: the existing self/Option/Diagnostic and cold Result
controls remain paid unchanged; the ordinary Memo::authenticate,
ReplayBudget::reserve_bytes and PhysicalReservation::reserve bodies are
byte-exact. On warm success the actual removed Diagnostic-bearing Results are
not constructed through another helper. Ledger predicates, borrowed function,
budget and slot references, scalar bools/indices and Oracle State/Option copies
remain real, just as in the original zero/Oracle-with branch; no scalar fee is
discounted. Existing cache work-charge/prepare_builder Results, outer closure
Result, get Option/Rc clone, miss and insertion Results remain independent and
paid. This is not an all-constructor or all-Result removal claim.

The complete immediate old canonical receiver, Memo and retained wrapper are
compiled independently. Original base94 before_scratch remains byte-exact; immediate839 before_ordered
retains its complete body and uses only an authenticated alias adaptation to
the independent old retained wrapper, so its old fee domain stays independent. The original per-auth fee test body is
unchanged and resolves its lexical Memo to the immediate old copy; its original
floor/cache test body resolves a complete immediate-old canonical receiver.
New controls exercise the current receiver separately. Full changed-source
archives and inverse/forward patches authenticate every old byte before the
protected older source projections. Existing protected metadata is unchanged.
The added source-owner modules join HIR, workspace, interop and documentation
unions. The documentation owner verifies every exact include and immutable
ordinary receiver rather than substituting a smaller root.

All success/diagnostic/physical/Drop/work/foreign/floor/source selectors remain
pending. Actual allocation, reservation and whole-owner Drop are reported
separately by the authored controls. No speed, byte saving, application-cap
recovery, production d6bf fix, or cleanup-only interpretation of the prior
48.84MB ALL-final-HIR observation is claimed.
