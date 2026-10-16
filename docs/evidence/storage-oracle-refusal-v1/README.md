# Storage-backed test comparison refusal

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

Hosted f4a448da6 unit job 116636394120 reached the complete enumerating and
factored comparison pair. At three underfunded builder boundaries, enumeration
succeeded but the factored receiver's next physical reservation failed. Its
assertion then panicked instead of returning the sticky refusal through the
already fallible comparison hook.

The storage-backed test receiver now returns the original Diagnostic when an
error coincides with the actual enclosing builder ledger's sticky refusal.
The hook propagates that error. The materialization counter is restored on this
new error exit, as on the existing successful exit. Work, physical prices,
limits, comparison algorithms and the original funded-disagreement assertion
are unchanged. A false/absent verdict still panics, including under a refused
ledger; a semantic error without a ledger refusal also retains the old panic.
This code is test-only and changes no release compiler admission or graph bill.

The complete original receiver is archived with a fixed SHA-256 and its entire
body remains compiled in `before.rs`; only its former test registration is
omitted. An executable source inverse binds both files and reconstructs every
original receiver byte, including the original assertion. Existing real-HIR
comparison, hostile-plan, exact/short, sticky and last-Drop controls remain.
New real-HIR tests cover the complete old/current funded pairs and the full
comparison hook at zero, exact and one-byte-short physical budgets, including
counter restoration and actual allocation/Drop observations.

Local component qualification executes both actual receivers with the actual
bounded-output ledger and Diagnostic implementation on Rust 1.88 and 1.97. A
synthetic comparison engine/HIR carrier checks full error preservation,
message-pointer preservation, counter restoration, unchanged funded true
results, and exact false/None/semantic-error panic parity. It also checks that
a sticky refusal cannot mask a false verdict. The actual Rust source-inverse
test executes separately against the real include files and SHA dependency.
These checks do not qualify the full real-HIR tests; hosted execution remains
required. Production bundled-library resource refusals remain separate work.
