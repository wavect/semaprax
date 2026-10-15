# DISPOSITION (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The raw review completed once in 67.04s; model claude-sonnet-5-5, list estimate USD0.099318, actual billing unavailable. No final-revision approval is claimed.

C2: corrected Option::as_ref argument/receiver overlap with two pointer copies and two bookkeeping units. Full new controls are derived from initialized types: two token pointers, two Option receiver pointers, optional reference return, receiving reference and branch bool; seven units. No old header credit.

B1: the asserted caller reordering does not occur. The protected replay.rs before archive already constructs StatusSources before regions/entry. Only the exits call changes. Full source inverse verifies this.

B2: paid Tree None fallback intentionally initializes and fully prepays new argument/Option controls before executing the complete old receiver. A control requires its debit and work to equal the old route plus the derived new fees. Original tight receipt remains independently tested against the complete old path. No equal-tight-boundary claim for the new fallback.

C1/C3: module visibility adapters are exact and reversible. Unobserved fallback still authenticates exact subject/budget/ACTIVE first; no positive borrow fee on it. Foreign/restored ACTIVE and same-owner sticky controls are authored, not executed. U4/U5 preserve original entry/region preconditions and producer-before-exits ordering. Existing same-slot ReplayBudget replacement ABA is not claimed supported; actual straight-line owner does not replace it.
