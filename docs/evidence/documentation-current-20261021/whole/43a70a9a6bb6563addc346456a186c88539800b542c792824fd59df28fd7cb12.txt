# Separate mechanical production lint correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The actual scoped940 production lib+CLI Clippy run failed with exactly three warnings promoted to errors. Its full receipt/log are preserved. This separate commit follows the780 fixture correction and changes only:

1. `CensusRefusal { error: error }` to field shorthand `error`.
2. Removal of an unused `ReplayContext` import. Every remaining type occurrence is fully qualified; trait-object method calls need no additional import.
3. Rename of the unused `expression_inner` parameter to `_function`; its type, argument position, callers, body and evaluation order are unchanged.

All three complete prior source files and whole-source inverses are preserved. No old oracle body, suppression, test, runtime, budget, error conversion, production visibility, compiler/Cargo/Clippy/Claude invocation, ROOT edit or remote change is included. Corrected Clippy results are UNEXECUTED. Integrator should add these exact full-source preinverses to current custody when adopting, and run the already-required scoped production qualification in the next consolidated batch. This packet requests no new test selector.
