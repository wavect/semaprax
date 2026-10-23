# Expression-fact source test entries

Status: two original complete source tests and the new hostile-input regression
pass locally in a source-only harness; full CI is pending.

Audience: compiler contributors and reviewers.

The receiving test still authenticates its original provenance, executes every
original projection hook and compares each complete original source. Its
expression-fact projection contains a later 409-byte profiling scope block.
The new inverse authenticates that complete projected input, removes only the
exact block once and compares the complete immutable original. The complete
projected input and the immediate-before test module are retained here.
Five negative cases reject empty, truncated, appended, changed-operation and
already-old inputs. Production profiling and accounting remain unchanged.

The case-proof test uses the existing authenticated CI source reader for the
retained receiver, matching its historical comparison's namespace. Its full
receiver comparison and independent ordinary-status source comparison remain
unchanged. The entire immediate-before case-proof test module is retained.

The source harness uses the actual source reader and actual complete test
functions, with only literal acquisition paths rooted to the SSD checkout.
No mock IR or product runtime qualification is claimed by these source checks.
