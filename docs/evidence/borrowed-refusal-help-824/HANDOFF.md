# Borrowed refusal help — source-only successor

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The selected TLS dominant/pending/sequential path is formatted through scoped immutable Ref guards instead of cloning owned path Strings. Pending remains first, sequential second, dominant fallback last. Every complete original message literal and branch remains. The guards never escape. Every formatting argument is a built-in String/str reference, usize, or bool branch selector; format_within/CappedString only touches the bounded_output budget/stage counters, not these diagnostic TLS cells, so this path has no custom Display callback or same-cell mutation/reentrancy.

Complete original formatter is independently compiled in borrowed_help_tests, restored byte-for-byte against its archive. The complete diagnostics.rs inverse retains all unrelated source, with exact counted hooks and fixed original SHA. Eight branch combinations include simultaneous pending/sequential evidence, live/precharge, resolving/retained and UTF8/NUL paths. New controls compare all Diagnostic fields, full original/current allocated bytes and complete Drop, plus0/sticky exact first refusal (no clone acquisition).

This removes reporting-time path clones only. Original TLS setup String acquisition and the owned Vec<Diagnostic> API remain explicit residuals. A nonempty owned error Vec cannot be newly prepaid after a sticky0 refusal; converting an inline error to that API after the observer would only hide its acquisition. No caller allowance, reset, cap/refund/forecast change, whole-constructor funding or application gain is claimed. Typed internal carrier/publication ownership boundary proposal is in the included review artifact.

Three exact new selectors and one audit child are in sibling JSONs. No Cargo/tests/builds/Clippy/Claude call was run for this mechanical local successor. Root must compose current full-source bindings and execute unchanged constructor/receiving/publication owners. Parent base6e265 remains the independent earlier next-batch wrapper/default packet; cherry-pick this successor only after that packet or resolve its ancestry deliberately.
