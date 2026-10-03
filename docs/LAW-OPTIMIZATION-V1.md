# Law-gated optimization v1

The first optional rewrite is `ProjectCandidate::propose_checked_i64_add_zero`.
It accepts only one authenticated authored body expression of the form
`i64_place + 0i64`. It checks a current native Z3/Lean proof token for the
assumption-free universal relational law `n + 0 == n`, with one `i64` binder.
The exact LawSet and Project revision are replayed before inspecting the
expression. A stale token, changed law, missing proof, other numeric type,
effectful left operand, foreign call, or different arithmetic expression
cannot take this route.

The compiler constructs an ordinary `replace_expression` intention for the
same lexical place. Its existing candidate route rechecks source, type,
ownership, effects, contracts and target admission after the rewrite.
The result is an immutable candidate; it neither edits a source file nor
publishes a workspace generation. The selected place is evaluated once at
the same position. Checked `i64` addition by zero cannot overflow, so this
closed rewrite preserves its value and checked failure behavior.

`CpuReferenceSession::checked_add_reduction_eligibility` is the separate
read-only report `semaprax.law-reduction-eligibility.v1`. It rechecks the
current Project-bound CPU artifact and the same installed identity-law proof,
then recognizes only `fn(acc: i64, item: i64) -> i64 { acc + item }` as a fold.
It checks the input and output as live, disjoint, correctly typed buffers and
checks every current input against a declared nonnegative interval. A bound
on maximum element times maximum count proves every regrouping's partial sum
fits checked `i64`, including the zero identity. A scheduler that may reorder
elements separately requires commutativity; exact bounded integer addition
satisfies it under the same no-overflow bound. The report records the exact
Project revision, artifact fingerprint, law digests, scheduler, reason for
ineligibility, and that no rewrite or parallel execution happened. It does
not claim a GPU speedup or alter the CPU/Metal sequential fold.

This is a narrow code change and bounded static arithmetic decision backed by
exact law evidence, not a general optimizer or translation-preservation
theorem. Target differential and benchmark evidence for the rewrite remain
open.
