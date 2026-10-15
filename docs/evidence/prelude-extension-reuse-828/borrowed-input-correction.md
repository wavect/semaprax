# Borrowed compiler input correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual be8f owners94/101 failed complete producer/refusal/Drop observation with retained allocations. The complete-slot producer fetched global compiler declarations before its first work debit. Cold direct-producer tests therefore initialized the process-lifetime compiler AST inside the allocation window, including zero-work refusal. This is an input-owner construction, not a reusable extension payload owned by the invocation.

The producer now receives an explicit borrowed compiler TypeDeclaration from the already-authenticated selected prefix. It no longer fetches or initializes global compiler AST. The actual caller authenticates exact immutable prefix identity before this call; all name/ID/kind and foreign-AST controls remain. Direct owning controls prepare that existing immutable input outside the observer, and keep the complete paid producer, every partial refusal, all allocations and final Drop inside. Every old allocation/free/invalid/live/work/sticky assertion is unchanged. No duplicate AST, paid model discount, extra quota, cap or work-bound change.

The all-prefix core control now selects Box through actual admitted box_new/box_into_inner calls rather than an unsupported bare Box signature that did not activate the profile; List is correctly a value parameter (its actual operations use ParamMode::Value). All seven prefixes, source verification, complete old/current fresh/borrowed/selective HIR/wire and payload lifetime checks remain.

The complete pre-correction source owners are retained under before-borrowed-input. v14 and own18 source inverses prepend exact correction steps, preserving all historical originals. The old no-extension receiver is retained under cfg(test), because exhaustive live source callers show only the existing cfg(test) reservation wrapper uses it. No compilation or execution is claimed by this source checkpoint.
