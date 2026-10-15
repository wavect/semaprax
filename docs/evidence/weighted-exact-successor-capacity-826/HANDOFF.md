# OPT826 exact small weighted successor backing

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Independent source-only correction from `93fdc103c`; no Cargo/build/test execution.
The frozen root is unchanged.

The production shared canonical receiver previously reserved targets.len() slots
then used Vec's FromIterator collector, whose minimum growth capacity can retain
four slots for one through three targets. The new private producer reserves the
same actual tuple layout, explicitly creates Vec::with_capacity(targets.len()),
and extends it with the original ordered target iterator before acquiring the
separately paid Rc header. This changes neither tuple values/order, weighted
algebra, copy work, memo policy, refusal stickiness nor any cap or allowance.

Authored controls inspect exact capacity and actual requested Vec+Rc Layout
backing at widths zero through four. They cover exact total limits, vector
one-short/zero refusal before acquisition, later Rc one-short refusal with only
the already funded vector allocated and fully dropped, and a subsequent empty
request refusing without allocation on the same sticky payer. A separate full
old/current receiver control exercises genuine zero-to-four target automata,
repeated weights and checked overflow with exact ids/signatures/work equality.

The complete old canonical receiver is unchanged, including its historical
collector behavior. Both production and new test sources remain in the existing
complete HIR source inventory; no relocation or audit exclusion was introduced.
The source binding pins complete original and current files. Static formatter,
whitespace and original-byte checks passed; runtime and current-head physical
qualification remain pending. No savings or application-fit result is claimed.

The frozen root's first focused test compilation also identified the relocated
test-only CanonicalFrame still used by the complete borrowed receiver. This
batch restores an identical cfg(test) root declaration, with the same Weighted
payload and enum layout. The full independent canonical_before receiver and
borrowed receiver are unchanged. The existing whole-source inverse removes only
the additive test frame before joining the old receiver, so its original full
archive remains authoritative. The large compressed custody file is not globally
formatted. Root owns separate LowerHex qualification fixes.
