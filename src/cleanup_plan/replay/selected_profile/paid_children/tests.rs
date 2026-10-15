//! The full prior receiving corpora and old receivers stay independent.
use super::collections;
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::cleanup_plan::replay::{
    ReplayBudget, expression_facts, selected_context, selected_profile,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::SourceRecordReplayFixture;
mod fixtures;
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn detailed(f: &ResolvedFunction, error: expression_facts::errors::EngineError) -> Diagnostic {
    match error {
        expression_facts::errors::EngineError::Owned(error) => error,
        expression_facts::errors::EngineError::Structural(receipt) => receipt.legacy(f),
        _ => panic!("unexpected profile error"),
    }
}
fn nodes(root: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    let mut todo = vec![root];
    let mut found = Vec::new();
    while let Some(expr) = todo.pop() {
        found.push(expr);
        todo.extend(crate::hir::lexical_exit::ChildCursor::new(expr));
    }
    found
}
#[test]
fn full_original_context_string_vector_record_corpora_and_renewal_identities_match_through_drop() {
    for source in [fixtures::STRING, fixtures::VECTOR, fixtures::RECORD] {
        let fixture = SourceRecordReplayFixture::new(source);
        fixture.with_prepared_context(|context| {
            for f in context.functions().iter() {
                if f.id.as_str() != "app.probe" && f.id.as_str() != "app.main" {
                    continue;
                }
                let ids = nodes(&f.body);
                let (_, seen) = measure(|| {
                    let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                    let mut current = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                    let before = selected_context::Context::new_paid_before_children(
                        context, f, &mut old, true,
                    );
                    let after = selected_context::Context::new_paid(context, f, &mut current, true);
                    match (before, after) {
                        (Ok(before), Ok(after)) => {
                            let a = before.primitive_renewals(f).unwrap();
                            let b = after.primitive_renewals(f).unwrap();
                            assert_eq!(a.appends(), b.appends());
                            assert_eq!(a.has_helpers(), b.has_helpers());
                            for expr in &ids {
                                match (a.get(&expr.id), b.get(&expr.id)) {
                                    (Some(a), Some(b)) => assert!(std::ptr::eq(a, b)),
                                    (None, None) => {}
                                    _ => panic!("complete renewal binding differs"),
                                }
                            }
                            drop(after);
                            drop(before);
                        }
                        (Err(a), Err(b)) => same(&detailed(f, a), &detailed(f, b)),
                        _ => panic!("complete paid Context verdict differs"),
                    }
                    assert!(
                        current.physical.as_deref().unwrap().charged()
                            >= old.physical.as_deref().unwrap().charged()
                    );
                    drop(current);
                    drop(old);
                });
                assert!(!seen.invalid);
                assert_eq!(seen.live, 0);
                assert_eq!(seen.allocated, seen.freed);
            }
        });
    }
}
#[test]
fn complete_old_collection_receivers_keep_wrong_arity_modes_and_competing_priority() {
    let fixture = SourceRecordReplayFixture::new(fixtures::VECTOR);
    fixture.with_prepared_context(|context|{
        let functions=context.functions();let f=functions.iter().find(|f|f.id.as_str()=="app.probe").unwrap();
        let original=nodes(&f.body).into_iter().find(|e|matches!(&e.kind,ResolvedExprKind::Call{callee,..} if crate::vec_ops::by_id(callee.as_str()).is_some())).unwrap();
        for attack in 0..4{
            let mut expression=(*original).clone();
            let ResolvedExprKind::Call{args,type_arguments,instance,..}=&mut expression.kind else{panic!("call")};
            match attack{0=>args.clear(),1=>type_arguments.clear(),2=>{if let Some(arg)=args.first_mut(){arg.ownership=OwnershipMode::Borrow;}expression.ownership=OwnershipMode::Borrow;},3=>{args.clear();type_arguments.clear();},_=>unreachable!()}
            assert!(instance.is_none());
            let (_,seen)=measure(||{
                let mut a=ReplayBudget::with_physical(f,usize::MAX).unwrap();let mut b=ReplayBudget::with_physical(f,usize::MAX).unwrap();
                let old=selected_profile::collections::call_facts(context,f,&expression,&mut a);
                let new=collections::call_facts(context,f,&expression,&mut b);
                match(old,new){(Ok(a),Ok(b))=>assert_eq!(a,b),(Err(a),Err(b))=>same(&a,&detailed(f,b)),_=>panic!("exact collection verdict differs")}
                drop(b);drop(a);
            });assert!(!seen.invalid);assert_eq!(seen.live,0);assert_eq!(seen.allocated,seen.freed);
        }
    });
}

fn append_subject(f: &ResolvedFunction) -> (&crate::hir::ResolvedBinding, &ResolvedExpr) {
    for expr in nodes(&f.body) {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expr.kind
        {
            for stmt in statements {
                if let crate::hir::ResolvedStatement::Assign { binding, value, .. } = stmt {
                    if binding.ty == ResolvedType::String {
                        return (binding, value);
                    }
                }
            }
        }
    }
    panic!("real String append subject")
}
fn app_work(b: &ReplayBudget) -> usize {
    if b.lent_skeleton {
        usize::MAX - b.skeleton_remaining
    } else {
        usize::MAX - b.remaining
    }
}
fn set_work(b: &mut ReplayBudget, lent: bool, allowance: usize) {
    b.lent_skeleton = lent;
    b.remaining = if lent { 0 } else { allowance };
    b.skeleton_remaining = if lent { allowance } else { 0 };
    b.skeleton_spent = 0;
}
fn append_extra_bytes() -> usize {
    use crate::cleanup_plan::replay::selected_context::{
        Context,
        child::{Deferred, Permit},
    };
    use crate::cleanup_plan::replay::semantic_refusal::Receipt;
    use crate::hir::{ExpressionId, ResolvedBinding, ValueId};
    use std::collections::{BTreeMap, BTreeSet};
    type E = expression_facts::errors::EngineError;
    // Complete actual new interfaces; no prior-header credit or observed gap.
    std::mem::size_of::<(
        Deferred,
        Option<Deferred>,
        Result<bool, Deferred>,
        Result<bool, Deferred>,
    )>() + std::mem::size_of::<(
        Permit<'static>,
        Refusal,
        Option<Refusal>,
        Receipt,
        E,
        Option<E>,
        Result<bool, Refusal>,
        Result<bool, E>,
        Result<Permit<'static>, E>,
        Result<Context<'static>, E>,
        Result<(), E>,
    )>() + std::mem::size_of::<(
        Refusal,
        Result<bool, Refusal>,
        Result<(), Refusal>,
        Result<bool, Refusal>,
        Option<Refusal>,
        BTreeSet<ValueId>,
        BTreeMap<ExpressionId, ExpressionId>,
        BTreeMap<ExpressionId, &ResolvedBinding>,
    )>()
}
#[test]
fn full_append_original_lent_unscoped_exact_short_zero_and_sticky_first_owner_stay_bound() {
    let fixture = SourceRecordReplayFixture::new(fixtures::STRING);
    fixture.with_prepared_context(|context| {
        let functions = context.functions();
        let f = functions
            .iter()
            .find(|f| f.id.as_str() == "app.probe")
            .unwrap();
        let (binding, value) = append_subject(f);
        for lent in [false, true] {
            let (_, seen) = measure(|| {
                let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let mut new = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                set_work(&mut old, lent, usize::MAX);
                set_work(&mut new, lent, usize::MAX);
                let old_base = old.physical.as_deref().unwrap().charged();
                let new_base = new.physical.as_deref().unwrap().charged();
                assert_eq!(
                    selected_profile::collections::append(f, binding, value, &mut old).unwrap(),
                    collections::append(f, binding, value, &mut new).unwrap()
                );
                let old_work = app_work(&old);
                let new_work = app_work(&new);
                assert_eq!(
                    new_work,
                    old_work + 4 + 11 + 8,
                    "all original units plus declared control initializers"
                );
                let old_bytes = old.physical.as_deref().unwrap().charged() - old_base;
                let new_bytes = new.physical.as_deref().unwrap().charged() - new_base;
                assert_eq!(new_bytes, old_bytes + append_extra_bytes());
                if lent {
                    assert_eq!(old.skeleton_spent, old_work);
                    assert_eq!(new.skeleton_spent, new_work);
                    assert_eq!((old.remaining, new.remaining), (0, 0));
                }
                // Match the same actual old cursor boundary, with every current
                // control paid first; do not fit a margin to an observed gap.
                for (a, b) in [(old_work, new_work), (old_work - 1, new_work - 1), (0, 0)] {
                    let mut before = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                    let mut after = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                    set_work(&mut before, lent, a);
                    set_work(&mut after, lent, b);
                    let first =
                        selected_profile::collections::append(f, binding, value, &mut before);
                    let second = collections::append(f, binding, value, &mut after);
                    if a == old_work {
                        assert_eq!(first.unwrap(), second.unwrap());
                    } else if a > 0 {
                        same(&first.unwrap_err(), &detailed(f, second.unwrap_err()));
                    } else {
                        assert!(first.is_err() && second.is_err());
                    }
                    if a < old_work {
                        let left =
                            selected_profile::collections::append(f, binding, value, &mut before)
                                .unwrap_err();
                        let right = detailed(
                            f,
                            collections::append(f, binding, value, &mut after).unwrap_err(),
                        );
                        same(&left, &right);
                    }
                    drop(after);
                    drop(before);
                }
                let frame_bytes =
                    std::mem::size_of::<[Option<crate::hir::lexical_exit::ChildCursor<'_>>; 514]>();
                assert_eq!(
                    old_bytes, frame_bytes,
                    "complete old append only acquires its original cursor"
                );
                for short in [0, 1, frame_bytes] {
                    let mut before =
                        ReplayBudget::with_physical(f, old_base + frame_bytes - short).unwrap();
                    let mut after = ReplayBudget::with_physical(
                        f,
                        new_base + append_extra_bytes() + frame_bytes - short,
                    )
                    .unwrap();
                    set_work(&mut before, lent, usize::MAX);
                    set_work(&mut after, lent, usize::MAX);
                    let a = selected_profile::collections::append(f, binding, value, &mut before);
                    let b = collections::append(f, binding, value, &mut after);
                    if short == 0 {
                        assert_eq!(a.unwrap(), b.unwrap());
                    } else {
                        same(&a.unwrap_err(), &detailed(f, b.unwrap_err()));
                    }
                    assert_eq!(
                        before.physical.as_deref_mut().unwrap().reserve(0).is_err(),
                        short != 0
                    );
                    assert_eq!(
                        after.physical.as_deref_mut().unwrap().reserve(0).is_err(),
                        short != 0
                    );
                    drop(after);
                    drop(before);
                }
                drop(new);
                drop(old);
            });
            assert!(!seen.invalid);
            assert_eq!(seen.live, 0);
            assert_eq!(seen.allocated, seen.freed);
        }
        // Broken-owner compatibility stays exact and explicitly unfunded.
        let mut before = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let mut after = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        drop(before.physical.take());
        drop(after.physical.take());
        same(
            &selected_profile::collections::append(f, binding, value, &mut before).unwrap_err(),
            &detailed(
                f,
                collections::append(f, binding, value, &mut after).unwrap_err(),
            ),
        );
    });
}

#[test]
fn actual_session_child_refusal_retains_full_old_diagnostic_and_public_paid_report_owner() {
    use crate::cleanup_plan::ReplayPhysicalReservation;
    use crate::cleanup_plan::replay::selected_entry::{Acquired, ReportForm};
    const FORM: ReportForm = ReportForm::new("SPX-T781", "selected finite profile refusal");
    let fixture = SourceRecordReplayFixture::new(fixtures::VECTOR);
    fixture.with_prepared_context(|context| {
        let functions = context.functions();
        let f = functions
            .iter()
            .find(|f| f.id.as_str() == "app.probe")
            .unwrap();
        let mut foreign = f.clone();
        fn poison(expr: &mut ResolvedExpr) -> bool {
            if let ResolvedExprKind::Call {
                callee,
                type_arguments,
                ..
            } = &mut expr.kind
            {
                if crate::vec_ops::by_id(callee.as_str()).is_some() {
                    type_arguments.clear();
                    return true;
                }
            }
            if let ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } = &mut expr.kind
            {
                for statement in statements {
                    match statement {
                        crate::hir::ResolvedStatement::Let { value, .. }
                        | crate::hir::ResolvedStatement::Assign { value, .. } => {
                            if poison(value) {
                                return true;
                            }
                        }
                        crate::hir::ResolvedStatement::While {
                            condition, body, ..
                        }
                        | crate::hir::ResolvedStatement::LexicalWhile {
                            condition, body, ..
                        } => {
                            if poison(condition) || poison(body) {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
                return poison(tail);
            }
            false
        }
        assert!(poison(&mut foreign.body));
        let (_, seen) = measure(|| {
            // Detailed and outward observations use real independent sessions;
            // explicit legacy rendering in this owner is not claimed prepaid.
            let prepare = || {
                Acquired::try_acquire(ReplayPhysicalReservation::new(usize::MAX), FORM)
                    .ok()
                    .unwrap()
                    .prepare(f)
                    .ok()
                    .unwrap()
            };
            let mut old = prepare().into_paid_session(context, f).ok().unwrap();
            let mut current = prepare().into_paid_session(context, f).ok().unwrap();
            let original = old.validate_paid_original(&foreign).unwrap_err();
            let actual = current.validate_paid(&foreign).unwrap_err();
            let expression_facts::errors::EngineError::Structural(receipt) = &actual else {
                panic!("real finite child must not fall back to Owned")
            };
            assert_eq!(receipt.stage(), "selected child");
            let actual = detailed(&foreign, actual);
            assert!(
                actual
                    .message
                    .contains("selected Vec call has invalid type arguments")
            );
            same(&detailed(&foreign, original), &actual);
            drop(current);
            drop(old);
            let mut old = prepare().into_paid_session(context, f).ok().unwrap();
            let prepared = prepare();
            let backing = prepared.report_backing();
            let mut current = prepared.into_paid_session(context, f).ok().unwrap();
            let original = old.validate_paid_original(&foreign).unwrap_err();
            let actual = current.validate_paid(&foreign).unwrap_err();
            assert!(matches!(
                current.validate_paid(f),
                Err(expression_facts::errors::EngineError::Failed)
            ));
            let a = crate::workspace_report::WorkspaceReportFailure::selected(
                old.into_failed_preparation().refuse_engine(original),
                Some(FORM),
            );
            let b = crate::workspace_report::WorkspaceReportFailure::selected(
                current.into_failed_preparation().refuse_engine(actual),
                Some(FORM),
            );
            assert!(a.has_paid_report() && b.has_paid_report());
            let ar = a.reported().unwrap();
            let br = b.reported().unwrap();
            assert_eq!((ar.len(), br.len()), (1, 1));
            same(&ar[0], &br[0]);
            let output = b.into_paid_report().ok().unwrap();
            assert_eq!(output.as_ptr(), backing.0);
            assert_eq!(output[0].message.as_ptr(), backing.1);
            same(&a.into_paid_report().ok().unwrap()[0], &output[0]);
            drop(output);
        });
        assert!(!seen.invalid);
        assert_eq!(seen.live, 0);
        assert_eq!(seen.allocated, seen.freed);
    });
}

#[test]
fn all_finite_labels_and_ignored_classification_keep_full_original_fields() {
    use crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal as P;
    let fixture = SourceRecordReplayFixture::new(fixtures::VECTOR);
    fixture.with_prepared_context(|context| {
        let functions = context.functions();
        let f = functions
            .iter()
            .find(|f| f.id.as_str() == "app.probe")
            .unwrap();
        // Independent literals inventoried from the complete prior child bodies.
        let cases = [
            (
                P::SelectedVecCalleeWorkOverflow,
                "selected Vec callee work overflow",
                true,
            ),
            (
                P::SelectedVecCallRequiresASeparatelyAuthenticatedOperation,
                "selected Vec call requires a separately authenticated operation",
                true,
            ),
            (
                P::SelectedVecCallHasInvalidTypeArguments,
                "selected Vec call has invalid type arguments",
                true,
            ),
            (
                P::SelectedVecSourceIsAbsent,
                "selected Vec source is absent",
                true,
            ),
            (
                P::SelectedVecElementAndRootDisagree,
                "selected Vec element and root disagree",
                true,
            ),
            (
                P::SelectedVecOperationOrArityIsOutsideTheExactElementFamily,
                "selected Vec operation or arity is outside the exact element family",
                true,
            ),
            (
                P::SelectedVecArgumentTypeOrModeDisagrees,
                "selected Vec argument type or mode disagrees",
                true,
            ),
            (
                P::SelectedVecResultTypeOrModeDisagrees,
                "selected Vec result type or mode disagrees",
                true,
            ),
            (
                P::SelectedVecFieldReadHasInvalidArity,
                "selected Vec field read has invalid arity",
                true,
            ),
            (
                P::SelectedFieldKeyOverflow,
                "selected field key overflow",
                true,
            ),
            (
                P::SelectedVecFieldElementIsNotCurrentFlatRecord,
                "selected Vec field element is not current flat record",
                true,
            ),
            (
                P::SelectedVecFieldIdentityIsForeign,
                "selected Vec field identity is foreign",
                true,
            ),
            (
                P::SelectedVecFieldModeIsUnsupported,
                "selected Vec field mode is unsupported",
                true,
            ),
            (
                P::SelectedVecFieldRootIndexResultTypeOrModeDisagrees,
                "selected Vec field root, index, result type or mode disagrees",
                true,
            ),
            (
                P::SelectedAppendLostReservation,
                "selected append lost reservation",
                true,
            ),
            (
                P::SelectedCollectionElementIsNotABoundedLeaf,
                "selected collection element is not a bounded leaf",
                false,
            ),
            (
                P::SelectedTypeHasUnsupportedArgumentCount,
                "selected type has unsupported argument count",
                false,
            ),
            (
                P::SelectedTypeKeyLengthOverflow,
                "selected type key length overflow",
                false,
            ),
            (
                P::SelectedRowLoanElementIsNotACurrentFlatRecord,
                "selected row loan element is not a current flat record",
                true,
            ),
            (
                P::SelectedRowLoanHasInvalidArity,
                "selected row loan has invalid arity",
                true,
            ),
            (
                P::SelectedRowLoanCarrierIndexOrBorrowedResultDisagrees,
                "selected row loan carrier, index or borrowed result disagrees",
                true,
            ),
            (
                P::SelectedStringTransferRequiresOneNamedOwnedString,
                "selected String transfer requires one named owned String",
                true,
            ),
            (
                P::SelectedTypeInventoryIsIncomplete,
                "selected type inventory is incomplete",
                true,
            ),
            (
                P::RecordClassificationWorkOverflowed,
                "record classification work overflowed",
                true,
            ),
            (
                P::SelectedConstructionHasNoExactDirectRecord,
                "selected construction has no exact direct record",
                true,
            ),
            (
                P::SelectedConstructionFieldIdentityOrderTypeOrOwnershipDisagrees,
                "selected construction field identity, order, type or ownership disagrees",
                true,
            ),
            (
                P::SelectedProjectionHasNoExactDirectRecord,
                "selected projection has no exact direct record",
                true,
            ),
            (
                P::SelectedProjectionFieldIsForeign,
                "selected projection field is foreign",
                true,
            ),
            (
                P::SelectedProjectionTypeDisagreesWithCurrentField,
                "selected projection type disagrees with current field",
                true,
            ),
            (
                P::SelectedVecHasInvalidTypeArgumentCount,
                "selected Vec has invalid type argument count",
                false,
            ),
            (
                P::SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily,
                "selected Vec element is outside the current bounded flat family",
                false,
            ),
            (
                P::SelectedVecCurrentDirectoryExceedsItsBoundedTranche,
                "selected Vec current directory exceeds its bounded tranche",
                false,
            ),
            (
                P::SelectedVecCurrentDirectoryIsIncomplete,
                "selected Vec current directory is incomplete",
                false,
            ),
            (
                P::SelectedVecCurrentDirectoryRepeatsItsIdentity,
                "selected Vec current directory repeats its identity",
                false,
            ),
            (
                P::SelectedVecDeclarationIsAbsentFromCurrentTypes,
                "selected Vec declaration is absent from current types",
                false,
            ),
            (
                P::SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection,
                "selected Vec declaration is not the exact compiler-owned collection",
                false,
            ),
            (
                P::SelectedCollectionProbePrepaymentRefused,
                "selected collection probe prepayment refused",
                false,
            ),
            (
                P::SelectedFlatRecordArgumentsAreUnsupported,
                "selected flat-record arguments are unsupported",
                false,
            ),
            (
                P::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields,
                "selected flat-record inventory has unauthenticated or unsupported fields",
                false,
            ),
            (
                P::LexicalLoopLatchDiffersFromItsIndependentlyAuthenticatedLowering,
                "lexical loop latch differs from its independently authenticated lowering",
                false,
            ),
            (
                P::LexicalFunctionOwnerDiffersFromCanonicalResultBodyIdentities,
                "lexical function owner differs from canonical result/body identities",
                false,
            ),
            (
                P::StaticUtf8LiteralExceeds65536DecodedBytes,
                "static UTF-8 literal exceeds 65536 decoded bytes",
                false,
            ),
        ];
        assert_eq!(cases.len(), 42);
        for (label, detail, prefixed) in cases {
            let old = if prefixed {
                crate::cleanup_plan::replay::replay_error(f, detail)
            } else {
                Diagnostic::io("SPX-H006", detail)
            };
            let new = crate::cleanup_plan::replay::semantic_refusal::Receipt::selected_child(
                f,
                Refusal::Profile(label),
            )
            .legacy(f);
            same(&old, &new);
        }
        for arguments in [
            vec![],
            vec![ResolvedType::I64],
            vec![ResolvedType::I64, ResolvedType::I64],
        ] {
            let ty = ResolvedType::Nominal {
                declaration: crate::hir::DeclarationId::new(crate::prelude::VEC_ID),
                arguments,
            };
            let old = crate::cleanup::selected_vector_element(context, &ty);
            let new = crate::cleanup::selected_inline::selected_vector_element_inline(context, &ty);
            match (&old, &new) {
                (Ok(a), Ok(b)) => assert_eq!(a, b),
                (Err(a), Err(b)) => same(a, &b.legacy()),
                _ => panic!("classifier verdict differs"),
            }
            // The renewal eligibility query historically ignores classification
            // failures; the new Copy refusal preserves that exact false branch.
            assert_eq!(old.ok().flatten().is_some(), new.ok().flatten().is_some());
        }
    });
}
