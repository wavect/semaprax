# Scoped replay oracle account v1

Audience: compiler contributors, tool authors, and reviewers.

Status: source foundation only. No build, test or allocation measurement was run in this lane. OPT810 remains open.

The test-only affine account separates allocation payment from admission. It has finite byte and work allowances, cumulative calls, balanced receiver counters, sticky first byte/work/physical refusal, epoch-bound leases and RAII scope cleanup. Missing, expired, nested and stale invocation authority fail closed. Oracle budgets retain Ordinary ceilings; they do not select retained admission, modify ACTIVE, refund work, normalize observations or cache verdicts.

## Activation boundary

The complete original `factored::cross_check` body remains unchanged. The two original failing owner tests also remain byte-exact, including their `1 << 20` literals, sticky retries, Drop receipts and aggregate assertions. Neither owner selects the new account. A private test receiver in `factored/oracle_hook.rs` exercises the proposed enum-then-factored pair on the live borrowed context and function. Its tests are pending; this checkpoint does not fix the two reported owner failures.

The proposed receiver uses independent fresh budgets and invocation guards, runs both receivers, and rejects physical/account refusal rather than converting it into a semantic verdict. It is deliberately inactive until the transitive acquisition audit is complete.

## Implemented source foundation

- Inline scoped account, byte/work receipts, authentication, unwind and expiry controls.
- Payer routing for replay backing, refusal text, overlap and clone cursors, without changing ordinary constructors or production ledger state.
- Explicit funded renewal inventories, moving string operands, string condition reads and helper classifications. Shared storage seams preserve the old work/depth ceilings and the independent old receivers.
- Borrowed diagnostic arguments in the replay root, counted before the complete message backing is reserved. The original diagnostic body remains an independent receiver with all six fields. Pre-owned messages fail closed on the funded route because their earlier allocation has not been authenticated.
- Additive finite-account helper allocation observations through output Drop, plus scripted exact/short/sticky boundary controls. These do not prove the complete pair.

## Open acquisition audit

| Boundary | Remaining work |
| --- | --- |
| Replay child diagnostics | Source-converted, unbuilt and unqualified: direct `format!` arguments to `replay_error` in the replay children now pass borrowed `format_args!` at the call, `semantic_refusal` formats branch-locally, and the lexical certificate/boundary `fail` closures forward `&str`. Still owned: `String` errors mapped through `replay_error` (`nested_shape.rs`, `paid_type_facts.rs`, `paid_type_facts/droppability.rs`, `ordinary_inventory/substitute.rs` `hir_error`); these fail closed on the funded route. Static census converted 37 direct formatting arguments plus all eight semantic-refusal arms and four lexical closures; the 18 remaining direct owned formatting calls belong to explicit test or original receivers. New controls observe finite-account diagnostic output through Drop and all eight atoms against a complete independent original adapter. These controls are unrun. |
| Byte helper census refusal | Source-changed, unqualified: the shared seam takes an explicit error constructor. Ordinary and selected routes pass the original owning `census_error`; the funded route passes an inline `io_static` carrier with the same six fields, and its diagnostics are discarded in favour of the recorded refusal, so it builds no owned diagnostic. The funded route constructs no real owned diagnostic, so there is no backing to prepay there. |
| Upstream error routing | Source-only: raw substitution H006 text now uses an explicit authenticated raw receiver with prior backing debit, preserving the complete original raw constructor and six fields. Account-authorized unbudgeted type probes use the existing paid droppability receiver and the same cumulative account; Ordinary defaults remain unchanged. Saved independent classifier receivers fail visibly if selected as account-funded evidence. |
| Classifier logical sink | Source-only explicit account receiver now constructs the unchanged ClassifierString layout without capturing ACTIVE. Both classifier storage domains select it only through the account payer; nested mutable forwarding preserves that selection and each logical append authenticates its live subject and cumulative lease before debiting the same finite account as replacement backing. Ordinary strict/capped/raw receivers remain unchanged. Scripted complete old logical frontier comparisons and actual finite baseline/exact/short/zero key acquisitions through Drop are additive and unrun. Account exhaustion is a typed sticky refusal, never a partial semantic key; no independent allowance or floor is granted. This checkpoint does not activate the live pair. |
| Direct helper diagnostics | Audit direct `Diagnostic::io`, error constructors and owned messages in lexical, call, type-fact, provider and normalization helpers. |
| Record field/destructure receiver | Source-only: the two direct record-match/update substitutions authenticate the current invocation and use the existing paid substitution with the same cumulative account. The explicit destructure twin prepays deque growth, field and binding nodes, prefix/ordinal copies and final binding backing. The complete ordinary receiver remains in its parent. Nominal owned whole-subtree admission still has an independent owning traversal; the account receiver refuses physically before calling that predicate, rather than manufacturing its boolean verdict. Additive full original nested success/error and substitution controls, finite exact/short/zero/sticky observations through Drop are unrun. |
| Success acquisition | Audit lexical HIR/certificates, nested shapes, type flags, normalized parameters, IDs/providers, latches and plan traversal before live activation. |
| Physical refusal | Complete sticky account propagation for every layout/allocation branch. |
| Whole receiver proof | Add continuous, independent allocation observation through Drop for both complete receivers, with full forwarding, renewal and hostile lease controls. |

No unfunded allocation is accepted as evidence of closure. Existing source comparisons, limits and old hostile controls are retained.

## Pending qualification

Account controls: `cleanup_plan::replay::allocation::oracle::tests`.
Invocation and complete diagnostic controls: `cleanup_plan::replay::oracle_controls::tests`.
Inactive proposed receiver: `cleanup_plan::replay::factored::oracle_hook::tests`.
Funded helper modules and `hir_module_boundaries` need qualification with the complete original owning controls. The two original failing selectors remain pending unchanged:

- `project::incremental::cold_source_tests::explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse`
- `workspace_graph::identity_resolution::tests::one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries`

Static source census and provenance accompany the private commit. Graph refresh and executable qualification belong to integration; this document records no passing evidence.

External authoring for this continuation was blocked by two automatic approval reviews despite the recorded explicit sharing approval. The preceding checkpoints were authored with Claude; the upstream routing and explicit classifier continuations were implemented locally and remains unqualified.

Explicit classifier selectors: `bounded_output::classifier_string::explicit::tests` and `cleanup_plan::replay::paid_type_facts::oracle_logical_tests`. Original `bounded_output::classifier_string::tests` and `cleanup_plan::replay::paid_type_facts::droppability::key_tests` remain independent controls. All are pending.

Pending destructure control: `cleanup_plan::replay::record_destructure::funded::tests::nested_destructure_and_field_substitution_match_full_old_receivers_through_drop`. This paid storage seam does not establish the remaining nominal admission or complete pair acquisition proof.

## Paid Oracle local operations (#929, #930)

The test-only enumerated flat comparison specializes to its actual
`Rc<SkeletonObservation>` slices. Each member keeps its old paid step, acquires
the complete new pointer-operation receivers, then pays one pointer equality
operation. Identical immutable storage advances the comparison; a miss performs
the complete previous observation comparison. Length, terminal comparison, both
independent sorts and the final path comparison remain. This introduces new work
and byte cuts; no old-cut equivalence or whole-caller gain is claimed.

The Oracle continuation constructor separately acquires two actual Frame slots
and pays complete replacement backing before each reached growth. It pays new
receivers, relocation and release before formation. The absent fixed 516-slot
allocation carries no old fee or refund. The same depth ceiling, independent
budgets, invocation authority and finite outer caps remain. Deep cumulative
backing and work can exceed the fixed constructor. Complete old comparison and
fixed constructor/growth controls remain executable beside exact source images
and inverses. Reference changes alone establish no application or agent gain.

### Required qualification

Qualification requires these six controls under
`cleanup_plan::replay::factored::oracle_order::flat_identity_tests`:

- `same_rc_and_equal_separate_rc_keep_order_with_new_exact_and_short_cuts`
- `identity_keeps_original_zero_receiver_work_failure_before_new_bank`
- `expired_pointer_receiver_never_spends_foreign_scope_or_changes_old_diagnostic`
- `complete_old_order_is_exact_and_new_submodules_remain_source_bound`
- `pointer_fee_exact_short_and_sticky_preserve_positive_active_floor`
- `comparison_unwind_releases_owned_rc_inputs_and_oracle_authority`

It also requires these four controls under
`cleanup_plan::replay::continuation_storage::oracle_reached_tests`:

- `oracle_initial_and_relocated_backing_keep_exact_short_zero_and_drop`
- `constructor_unwind_drops_actual_new_and_complete_old_backing_and_authority`
- `expired_constructor_receiver_keeps_old_diagnostic_without_foreign_debit`
- `new_oracle_fee_frontiers_exact_short_and_sticky_preserve_positive_active_floor`

The unchanged owning callable and complete 64-row controls are also required:

- `workspace_graph::tests::callable_limit_exact_advances_and_one_over_is_g171`
- `cleanup_plan::replay::expression_facts::receiving::retained::case_certified::lookup_tests::receiving::whole_immediate_ordinary_result_exact_short_zero_and_real_nm_are_observed`
- `cleanup_plan::replay::expression_facts::receiving::retained::case_certified::lookup_tests::complete_immediate_old_ordinary_caller_preserves_results_and_funds_real_dense_route`

The source-only lexical flat-identity work candidate removes the outer
`ReplayBudget::charge_compact` call packet at the two newly introduced pointer
identity work sites. Its restricted macro accepts only the actual budget and
function places plus the original caller-location token. The operation remains
one unit with the same static phase. It creates no units/phase parameter aliases
or outer `Result<(), ByteFailure>` return/receiver. The original general method
is byte-identical and independently executable. The complete original prepare,
sticky, lent-skeleton dispatch, oracle-work, checked subtraction, lazy refusal
closure and diagnostic trace remain in their original order. Lending still calls
the original compact skeleton producer, including its repeated prepare and
checks. The caller-location token is evaluated once at the old trace point;
it is not an additional function transport or an authority-switch callback.

The per-pointer bank retains the reserve_compact_bytes actual/formal tuple and
its two compact result roles, Rc comparison loans and booleans. It removes only
`2*sizeof((&mut ReplayBudget,&ResolvedFunction,usize,&str))` plus
`2*sizeof(Result<(),ByteFailure>)`, matching the absent work forwarding boundary.
Every surviving nested helper's complete Diagnostic and compact/refusal
constructors retain their funding; the full once-only terminal conversion bank
is unchanged. A cfg(test) complete immediate-old items body executes lexically
at the same original signature with its original entire hot tariff. No reference
cap, work count, generation check, old receiver or refusal status is relaxed.
Authored controls compare the complete old/current invoices, all work cuts,
exact/short/sticky paths, real expired/foreign scopes, lending and local ceiling
trace fields, source bodies and full Drop. They are unrun: the source-level typed
delta establishes no whole-reference fit or application-admission claim.
