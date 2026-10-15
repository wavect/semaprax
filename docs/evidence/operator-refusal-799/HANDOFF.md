# OPT799 operator refusal propagation

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Issue: https://github.com/wavect/semaprax/issues/799

Unary native-unit refusal and binary missing/refused operands no longer publish a synthetic checked result. Binary checks, left-to-right child checking and lazy ownership joins retain their original ordering. Only diagnostics produced by the operator itself gate its publication; earlier independent diagnostics do not poison a still-checked operand.

A refused let initializer retains a declaration-only marker for genuine duplicate-name checks. It supplies no Binding, Type, ownership or capture authority. Read-before/self/sibling checks stay active.

Complete original operators are independently compiled in before_799.rs with an exact method-name inverse to the whole original archive. The existing scalar comparison retains every full original value, binding-state and diagnostic assertion against that receiver, and additionally checks the live full ordered diagnostics for every fixture. Dedicated live/saved controls pin success, no-result refusal, nested cascade removal and lazy owned-call joins. Original recursive and pre-806 receivers are unchanged.

Unknown match scrutinee/pattern provenance remains a separate outstanding OPT799 seam. This packet does not close OPT799 or claim any application gain. No quotas, selected profile or budget changes. No Cargo/build/tests/Clippy were run. The bounded review is source-only; actual qualification remains pending.
