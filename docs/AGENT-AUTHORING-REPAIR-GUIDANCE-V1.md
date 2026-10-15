# Agent authoring repair guidance v1

Status: draft specification; qualification remains as recorded below.

Audience: compiler contributors, tool authors, and reviewers.

This tranche adds optional help to existing source verifier diagnostics and indexed language help. It changes no admitted syntax, profile, authority, binding metadata, primary code/message/span/path or backend behavior. Unknown and duplicate payload-field diagnostics remain separate from invalid/duplicate local binders. Scalar patterns use a fresh local; record and variant payloads can retain a field name while binding it with `field: fresh_alias`. The example name is not guaranteed available. Existing independent ownership and type diagnostics remain authoritative.

Transcript guidance selects fixed compiler-authored repeated-write, loop-condition, loop-body and cyclic-closure reasons. It recommends construction within existing bounds and one publication from an admitted location. It carries no invented site, loop or cycle witness. Complete real flow witnesses remain follow-up #821.

Help storage is optional actual owned backing: preserving admission checks remaining bytes and the active diagnostic floor before one fallible exact String acquisition. Already-sticky refusal or insufficient allowance omits help without replacing the selected error. A successful allocation consumes real bytes and can move later required-work refusal boundaries. Existing primary message/path allocation is outside this decorator's guarantee. No token, turn, application-fit or whole-invocation allocation gain is asserted.

Qualification is pending the next coherent batch. Owning selectors: source_verify::hints::authoring_repair_tests (four exact controls), language verifier_hints pattern_collision_explains_alias_without_replacing_the_outer_binding and transcript_repeated_writes_explain_bounded_publication, unchanged scoped recovery/collision controls, and exact CLI diagnostic-help coverage. Regenerate docs/AGENT-DIAGNOSTIC-HELP.json only via documentation agent_quick_reference::regenerate_diagnostic_help; never edit its output by hand.
