# Empty postcondition result carrier

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The production function receiver skips only the unused result-scope binding when `function.ensures` is empty. Result identity registration, all body/type/ownership checks and lifecycle/loan validation still run in their original order. The empty path authenticates the existing sticky payer with a zero-byte reservation before returning success. A nonempty postcondition retains the complete original construction and validation path.

The omitted carrier has no consumer: `validate_contracts_persistent` already returns immediately for an empty contract slice. The local scope is discarded after validation and does not publish authority or a program change. No type, ownership, graph, cleanup or backend policy changes.

`before.rs` independently compiles the complete immediately prior function receiver. The frozen complete source is archived with its original digest; a counted additive inverse restores every byte. Existing original receivers remain unchanged. Both HIR and both builder source readers join all three new child sources.

Five new exact controls cover complete current/prior function state and diagnostics, nonempty clauses, owned results, late errors, exact/short/zero reservations, sticky refusal, real allocation through receiving Drop, and complete source provenance. Existing whole-chain controls remain selected separately. Validator constructor custody is acquired identically before the function-only allocation observation; this does not replace a complete caller observation.

Source-only. No Cargo, tests, Clippy or application runtime were run. The archived 85286 Pattern receipt first refused CleanupReplay (10280 requested, 6934 remaining); it does not attribute that refusal to this result binding. No application fit, savings measurement, issue closure or broad funding claim.
