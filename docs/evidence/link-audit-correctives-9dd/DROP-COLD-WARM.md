# Complete-observer global ownership discrimination — source candidate

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual9dd owner83 failed live6420==0 at linked tests.rs270. This is a live
retention failure, not evidence of allocation during Drop. The complete log and
complete prior regression source are retained unchanged.

The test fixture calls the public compatibility resolver. Its first use reaches
prelude::declarations, whose OnceLock<Vec<TypeDeclaration>> intentionally retains
the complete eleven-type process prelude. The source shows a durable global
owner distinct from the linked cases Owner; its actual byte contribution to this
failure is not established until the corrected control executes.

The corrected control keeps all setup, parsing, HIR construction, synthetic512
recursive levels, input Drop, surviving-case view access and index final Drop
inside each complete observer. It explicitly observes the actual OnceLock call
before the fixture and records its live delta, with no hardcoded allowance.
Two independent fresh owned complete initializers are compared against the
actual global values and dropped in that same observer; every one of those
allocations must free. Final live bytes must equal only the observed initializer
delta; all other bytes must balance. A second genuinely warm pass requires delta0
and preserves the original full live0/allocated==freed assertions. An unclassified
payload/index leak still fails, including on the cold pass. Actual deltas are
reported after the observers; no observer scope is narrowed.

This is a test ownership-classification change, not a production Drop repair,
new payer, authority, cap, or evidence of full public compatibility funding.
The prior full zero-expectation body remains in the unchanged before archive and
as the warm executable regression. No Cargo/build/Rust execution here.
