# Law-gated optimization v1

Status: bounded law-gated rewrite profile.

Audience: Project authors and maintainers evaluating bounded law and proof support.

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

The installed-Z3 gate emits and validates actual Core Wasm before and after
the candidate rewrite. Node executes both emitted modules with checked `i64`
imports: the normal entry returns the same value, an independent guarded
overflow entry fails with the same error before and after its rewrite, and a
seeded wrong-value module produces a distinct result. This
covers the admitted target behavior but does not establish a general lowering
theorem. The same gate times five samples of 100,000 calls on each emitted
module. One local arm64 run with Node v24.3.0 measured median 25.91 ns/call
before and 10.42 ns/call after the rewrite; this is a narrow call benchmark,
not a GPU result or a performance guarantee. The gate also refuses a call
operand and mismatched `i32` expression, rejects a non-`i64` fold input, and
refuses a stale CPU artifact after source drift. Floating-point kernels are
outside the CPU kernel vocabulary, so the report cannot mark one eligible.
