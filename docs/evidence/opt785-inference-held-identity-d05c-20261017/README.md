# #785 inference target identity reuse — source-only partial migration

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base d05c354dcb3470daf46308a6fd46dc9c493693e3. No Cargo, runtime, application comparison, live-agent measurement or issue closure. Public DeclarationId::new and pure-query compatibility behavior stay unchanged; full dynamic construction migration remains open.

The two real generic_inference target constructors (owner at41 and template at386 in the base) clone the exact source stable-ID String then create a second Arc backing. Both now call the existing DeclarationIndex::held_identity with the same borrowed spelling. The initial index construction retains its original explicit payer and pays the complete backing once; no allowance is created or transferred, no old charge is refunded, and this change does not describe legacy-created indexes as paid.

Production ordinary_call_signature_ref receives its FunctionRef from resolve_expr.rs:482–505: the current index resolves the call's function ID, then the same resolver source_functions inventory selects the target by exact stable-ID. infer_call_arguments_ref is reached with that same target; recursive evidence_call_result selects from the same resolver source inventory. DeclarationIndex::held_identity (source.rs:16–33) has no fallback/interning/authority: the index must already contain that subject, and the result is its Arc clone. Existing declaration-kind, generic-profile, argument-order and source-verifier checks stay in their original positions. Detached arbitrary FunctionRefs outside this premise are not a newly admitted API.

No new constructor/helper/predicate or Result forwarding layer is introduced at these sites. The previously allocated initializer/receiver is replaced by the existing held-identity receiver; pointer identity remains only a test observation, while production Eq/Ord/Hash use bytes. The full generic argument producers, retained derive constructor, inference bounds, Result and Diagnostic ordering remain unchanged. The existing saved policy in resolver_held executes the complete former String clone plus shared allocation at each affected lookup, enabling common-algorithm comparisons. This is an explicit old allocation-policy oracle, not a frozen whole old engine.

Two appended existing-module controls are pending:
- hir::declaration_index::source::identity_reuse_tests::resolver_held::inference_and_call_signature_keep_complete_old_results_and_index_backing_through_drop
- hir::declaration_index::source::identity_reuse_tests::resolver_held::inference_index_payment_refuses_exact_short_and_preserves_original_payer_before_lowering

The first keeps complete inferred types and signature/instance result parity, records actual old construction requests outside allocation observations, compares exact removed payload requests and full Drop, and exercises inference on an unrelated exhausted output account without granting new payment. The second uses the original explicit identity-resolution callback on the whole real resolver, derives its exact successful request frontier, retains every earlier request at zero/short refusal, propagates its exact error, and drops partial/successful roots inside physical observation. Request-recorder backing is acquired outside observation. All original test bodies are an exact prefix and full original programs/inverses are retained.

Future verification is parent-owned: compile these two selectors in the existing semaprax lib harness, discover their actual literal names, run them with affected original generic-inference/held-identity controls in the next combined batch, scoped production Clippy, and source custody rebind after review. Do not rerun suites, alter current frozen selection, repin old expected hashes or call this a current measured saving. Existing common source ownership may need one finite current-to-d05 input stage for the two changed files; no global authority generator was run here.
