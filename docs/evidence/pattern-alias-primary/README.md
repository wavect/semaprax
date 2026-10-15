# Pattern alias primary controls

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Issue #820: the new ordinary parsed-source case keeps complete primary diagnostic JSON for a repeated alias and reserved `result`, preserves the independent duplicate-field diagnostic and first-error order, checks the advice, and verifies the corrected unique alias. Only the documented help field is removed for comparison with the independently authored primary expectation. It does not alter production or the complete existing recovery test, which compares the recursive, saved iterative and current receivers in full across real source contexts.

No build or tests were executed in this lane. The two exact owning selectors require final-head qualification. The diagnostic pin was generated separately by the actual producer; this patch does not change its content or guidance.
