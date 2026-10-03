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

This is a narrow code change backed by exact law evidence, not a general
optimizer or a translation-preservation theorem. The CPU reference and Metal
sequential fold continue to execute in their existing order. Parallel
reduction eligibility is a separate read-only decision and is not yet
provided by this v1 route.
