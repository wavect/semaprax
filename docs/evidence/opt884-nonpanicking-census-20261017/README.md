# Nonpanicking successor application report

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Baseline: `7622b5b10e28e95f18a6710e99f3b552491977f6`.

Independent source review found that `eprintln!` in the application Scope Drop
could panic on a closed stderr stream, replacing the original error or aborting
a genuine unwind. The successor restores the counter state with fallible TLS
access, then uses two fallible writes to locked stderr and ignores IO errors.
The new application invocation and phase restoration also tolerate TLS teardown.
Original single-pair observer operations and controls remain unchanged.

One authored control supplies a closed writer on normal return and synthetic
unwind, checks fixture teardown precedes both attempted report lines, and checks
the original panic payload survives. It does not wrap a real producer.

Whole before/current bytes and a finite full-byte forward/inverse mapping are
retained. These certify source custody, not runtime correctness. No Cargo, rustc,
build, test, binder, heavy process, application, or gain qualification ran.
Scoped rustfmt and diff whitespace checks are source-only.

Actual pricing, primitive banks, warm/cold flags, field layout, frame loop,
application caps/profiles/fixtures, and both existing application scope positions
are untouched. Original parent and baseline worktrees remain sealed.
