# OPT843 sticky owner correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Immediate predecessor `538673710c463b911c2e5a38d8f7fc9275f99c62`.
Both invalid destination branches now invalidate the ordinary Memo with its
existing unit `reject_after_error` before returning the unchanged Diagnostic.
There is no new Result, payer query, allocation or authority grant. This prevents
a different correctly bound destination from curing a canonical receiver after
a foreign function/budget/ledger attempt. Both entry points still authenticate
before accessing any cached closure or canonical state.

One direct endpoint regression primes the actual closure or id cache, attempts
a foreign function, foreign budget, nested current ledger or expired slot,
compares all six Diagnostic fields, checks hidden views and unchanged work,
payer and allocation observations, and retries both the failed and a distinct
original-owner slot. It includes whole-owner Drop. The existing original test
bodies are byte-exact prefixes and the original runtime/source JSONs are
unchanged. This new complete two-source inverse packet is a distinct history;
the parent supplies the final merged-source successor before the old 538 views.

Only source/Python assertions and focused rustfmt ran. The regression has not
been compiled or executed. No performance or completion claim is made.
