# Reservation call-site layout follow-up

Status: private component evidence; owning current-main unit execution pending.

Audience: compiler maintainers and allocation reviewers.

The two existing stage-layout controls compared today's budget against the
complete budget from before the call-site diagnostic. On current definitions,
both original bodies reproduce the 72-versus-64-byte mismatch. Removing only
`first_site` in the copied diagnostic probe makes both original controls pass.
Production retains that sticky pointer and all diagnostic behavior.

The complete pre-site budget in `subjects.json` matches Git immediately before
`e1d881fa84af096f834e5abb6738b60e1f21d6fe`, which introduced the reservation
caller. `HistoricalBudget` compiles that exact subject and remains compared
against each complete original `BeforeBudget`. The actual current budget is
separately compared against a typed carrier containing the complete original
budget and one optional static call-site pointer. This accounts for exactly
eight added bytes on the tested 64-bit targets; the old size is not replaced.
All original enum, receipt, guard, refusal and paid-carrier controls remain.

Fixed hashes retain both whole original test functions and both complete budget
subjects. The source control permits only the explicit comparison expansion,
binds the entire live budget and static pointer alias, and proves that removing
exactly that one field produces the compiled historical subject. Another field,
type, reordered source definition or edited old control fails that binding.

`source-follow-up.json` reconstructs both complete prior-head test files through
literal inverses. The two affected older inverse packets retain their original
hashes and archives. The canonical-stage binding report and combined integration
report remain byte-exact historical evidence with their original claims.

The actual two layout bodies and new source control passed against exact current
type definitions in a bounded static-standard-library probe. The original two
expanded layout controls also passed on Rust 1.88 with their historical/current
comparisons. This verifies layout and source custody only; complete ledger,
unwind, sticky-refusal and owning HIR execution still require current-main CI.
No production file, allowance, diagnostic, stack bound or semantic fixture is
changed by this follow-up.
