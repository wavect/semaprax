# Project Diagnostic Change v1

Status: Partial; implementation and regression tests authored, unrun.

Audience: agent builders, compiler contributors, and reviewers.

This additive Semantic Change operation turns the existing
[candidate integer-literal repair](PROJECT-CANDIDATE-DIAGNOSTICS-V1.md) into an
exact, replayable intention. Canonical source and full Project admission remain
authoritative. No general diagnostic repair, invalid semantic image, automatic
selection, source publication, test execution, or runtime equivalence is claimed.

## Closed request

```json
{
  "kind": "repair_diagnostic",
  "target": "calculator.add",
  "rejected_intent": {
    "kind": "replace_function_body",
    "target": "calculator.add",
    "body": {"kind": "i32", "value": 42}
  },
  "repair_id": "sha256:<64 lowercase hexadecimal digits>"
}
```

The outer object has exactly those four fields. The rejected intention has
exactly `kind`, `target`, and `body`; its kind must be `replace_function_body`
and its target must equal the outer target. The body has exactly `kind` and
`value`, selecting an explicit `i64`, `i32`, `u8`, or `usize` integer literal.
The existing repair class additionally requires the literal to fit its stated
type, differ from the retained function return type, and fit that return type
without changing its integer value. A replacement expression, supplied
diagnostic, source text, HIR payload, or nested repair is not accepted.

`ProjectCandidateAttempt::repair_catalog` now includes a compiler-derived
`semantic_change_intent` beside each offered repair. Pass that exact object to
`SemanticChange::new` bound to the current Project revision, then use ordinary
`ProjectCandidate::apply` bound to the current candidate digest. The change
catalogue advertises this constructor only for supported integer-returning
monomorphic explicit top-level functions other than `main`; discovery does not
assert that an arbitrary rejected intention has an available repair.

## Independent derivation and history

Application first executes the exact rejected body intention against the
current immutable candidate. It must actually fail. The resulting bounded
diagnostics and exact predecessor/history are used to regenerate the ordinary
attempt digest. The existing repair class derives a retagged literal from the
retained HIR return type, fully admits that proposal, and regenerates its repair
ID. The requested ID must match exactly before that compiler-derived body
transformation is used.

The outer ordinary application then canonically formats and reparses sources,
rebuilds and independently replays the Project, and checks identity, contract,
effect, ownership, cleanup, profile, and admitted target requirements. This
does not trust the previously derived candidate as a substitute for admission.
The completed history records `repair_diagnostic`, the rejected intention, and
the exact repair ID. It does not replace that history with a plain body edit.
Thus replay and recovery regenerate both the rejection and its offered repair.

The older attempt-level `repair_diagnostic(expected_attempt, repair_id)` API is
unchanged: it returns the ordinary body-change candidate. The new wire operation
can produce the same Project source revision but a different candidate digest,
because its intention history is different. The catalogue's existing
`validated_candidate_revision` continues to identify the ordinary proposal,
not the new history-preserving result.

Repair IDs bind the exact candidate predecessor, including its history. A
selector cannot be reused after an unrelated edit by merely rebinding the outer
Project revision. Rebase of a repair intention fails explicitly; callers must
rediscover a repair on the intended predecessor. Merge may retain an unchanged
history/prefix, but does not remint a repair selector in a suffix being rebased.

## Bounds, diagnostics, and evidence

Existing Semantic Change byte/depth/node limits and the 32-intention limit apply.
The existing attempt bounds remain 256 diagnostics, 1 MiB diagnostic text, and
2 MiB reports. Rejection/repair derivation shares immutable admitted revisions
and copies bounded predecessor reports/history; it does not replay that history
recursively or retain failed source/HIR. Two nested ordinary applications are
possible: the rejected attempt and the proposed body repair. Neither can invoke
this repair operation recursively because the rejected kind is closed.

`SPX-G268` rejects the request grammar and malformed selectors. `SPX-G270`
rejects accepted rather than rejected attempts, unavailable repairs, and stale
or mismatched repair IDs. `SPX-G271` rejects repair-selector rebasing. Existing
attempt capacity, constructor, compiler, and candidate stale/replay diagnostics
remain unchanged. No separate G269 condition is introduced in this tranche.

The new regressions in
[`tests/project_candidate_diagnostics_v1.rs`](../tests/project_candidate_diagnostics_v1.rs)
cover real repair history, exact replay/recovery, equivalent ordinary source
revision with distinct candidate history, literal/target/extra-field tampering,
recursive and successful-attempt rejection, predecessor binding, explicit rebase
conflict, and unchanged original files. They are authored and unrun at the user's
request. No compiler check, interpreter, target executable, or local gate was run.
