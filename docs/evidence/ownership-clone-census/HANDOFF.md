# Ownership clone census accounting correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only base9285; no Cargo/build/tests/observer/caps edits. No heap18 attribution or app-fit claim.

The new ownership payer entered the real ResolvedType ClonePayload census without reserving its514 Option<(&ResolvedType,usize)> inline cursor. Replay allocation.rs200 explicitly requires paying that exact cursor before preflight initialization. This is inline accounting; census allocates no heap and shared IDs clone with no new backing.

Payer now owns one cursor-paid bool, funded before construction. Every clone authenticates reserve0. The first census prepays the complete514-frame array before initialization; subsequent clones reuse the same lexical payer reservation, while still paying complete actual clone payload and retaining sticky refusal. Storage's original containing-carrier reservation stays unchanged. No refund/new limit.

Independent Claude caught eager factory funding breaking unchanged primitive zero-limit controls. Corrected by additive type_flags_paid_lazy: authenticate, EXACT literal original primitive/cache gates, and invoke paid factory only on existing constructor branch. Original paid.rs body remains complete independently compiled. Ownership hook passes explicit same-active-payer authenticate/new function items. State is private to one lexical fallback invocation, never cached across budgets/validators.

Full original paid.rs archived/fixedSHA; removing only exact named lazy span restores original whole source byteexact and lazy fast gate text equals original. Four full ownership-source inverses still pass original fixed hashes; only exact current nominal hook string updated. Original owningtests prefix byteexact. No flagswalker/key/substitute/admission or ClonePayload changes.

Four additive controls cover literal Function+Nominal UTF8/NUL clone backing/cursor once/two clones, exact-short-zero-sticky/fullDrop; deep1/32/514/515 census boundary/refusal before acquisition; primitive/Array/Function/compound forgedcache zero-limit factory-not-called plus unknownmiss firstfunding; and full original paid source/fastgate equality. Pending-selectors retains26prior plus4new=30. All authored/unexecuted. No new Rustchildren; parent owns current hashes.

Actual tool-free Claude Sonnet5.5 firstParty reviews:112.26s list$0.2906575;59.25s list$0.1711155;38.63s list$0.1257575. Final no definite blocker; actual billing unknown. Static formatting/diff/full source inverse/gates/prefix checks passed. Final source inverse control added after review and locally checked. No execution claim.
