# Actual a417 Legacy diagnostic fixture correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Owner131 failed before the recursive-layout comparison: the unchanged record-only input violates the parser requirement for at least one function. Preserve that exact original input as a full six-field P101 negative. Only the recursive semantic fixture appends one ordinary scalar main, retaining the original recursive record/declaration IDs and position; full old/current diagnostic equality and the other three malformed cases stay intact. No protocol/source admission, production changes or test execution. Full prior test file archived exactly.
