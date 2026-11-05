//! Complete call receiver parity, all receiving fees and owned handle Drop.
use super::*;
use crate::cleanup_plan::replay::structural_control::{
    call_receiving::{Kind, Permit},
    calls_before_receipt as before,
};
use crate::test_allocation_observer::measure;
use std::borrow::Cow;
type Engine = expression_facts::errors::EngineError;
const SOURCE: &str = r#"module call.custody;
@id("callee") fn callee(n:i64)->i64 {n}
@id("main:é") fn main()->i64 {callee(7)}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "call-custody.spx").unwrap()).unwrap()
}
fn subject(p: &ResolvedProgram) -> &ResolvedFunction {
    p.functions
        .iter()
        .find(|f| f.id.as_str() == "main:é")
        .unwrap()
}
fn call(f: &ResolvedFunction) -> &ResolvedExpr {
    let mut todo = vec![&f.body];
    while let Some(node) = todo.pop() {
        if matches!(node.kind, ResolvedExprKind::Call { .. }) {
            return node;
        }
        todo.extend(ChildCursor::new(node));
    }
    panic!("source call")
}
fn fact(e: &ResolvedExpr) -> CallFact<'_> {
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &e.kind
    else {
        panic!("call")
    };
    CallFact {
        callee: Cow::Borrowed(callee),
        instance: instance.as_ref(),
        arguments: call_payload::Arguments::Slice(args),
        type_arguments: call_payload::Types::Slice(type_arguments),
    }
}
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn detailed(f: &ResolvedFunction, e: Engine) -> Diagnostic {
    match e {
        Engine::Owned(e) => e,
        Engine::Structural(r) => r.legacy(f),
        _ => panic!("call error kind"),
    }
}
fn budget(f: &ResolvedFunction, extra: usize) -> ReplayBudget {
    let setup = std::mem::size_of::<PhysicalReservation>()
        + 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>()
        + f.id.as_str().len()
        + 512;
    let mut budget = ReplayBudget::with_physical(f, setup.saturating_add(extra)).unwrap();
    budget.remaining = usize::MAX;
    budget
}
fn controls<T>() -> usize {
    std::mem::size_of::<(
        Permit<'static, Engine>,
        Kind,
        DeclarationId,
        semantic_refusal::Atom,
        semantic_refusal::Receipt,
        Engine,
        Option<Engine>,
        Result<T, Engine>,
        Result<(), Engine>,
        Result<Permit<'static, Engine>, Engine>,
        &'static Permit<'static, Engine>,
    )>()
}
#[test]
fn actual_unknown_callee_and_competing_call_errors_keep_full_old_text_and_zero_payload_allocation()
{
    let p = program();
    let f = subject(&p);
    for text in [
        "missing".to_owned(),
        "missing:é\0雪".to_owned(),
        "long:é".repeat(4096),
    ] {
        let mut e = call(f).clone();
        let ResolvedExprKind::Call { callee, .. } = &mut e.kind else {
            unreachable!()
        };
        *callee = DeclarationId::new(text);
        for generic in [false, true] {
            let mut expression = e.clone();
            if generic {
                let ResolvedExprKind::Call { type_arguments, .. } = &mut expression.kind else {
                    unreachable!()
                };
                type_arguments.push(ResolvedType::I64);
            }
            let fields = fact(&expression);
            let mut a = budget(f, usize::MAX);
            let mut b = budget(f, usize::MAX);
            let old = before::signature(&p, f, &fields, &expression, &mut a)
                .err()
                .unwrap();
            let (new, observed) = measure(|| {
                signature::<Engine>(&p, f, &fields, &expression, &mut b)
                    .err()
                    .unwrap()
            });
            assert!(!observed.invalid);
            assert_eq!(observed.allocations, 0);
            assert_eq!(observed.live, 0);
            let Engine::Structural(receipt) = &new else {
                panic!("selected call must remain typed")
            };
            assert_eq!(receipt.stage(), "selected call");
            same(&old, &detailed(f, new));
        }
    }
    // A genuine existing callee retains the complete signature on both paths.
    let expression = call(f);
    let fields = fact(expression);
    let mut a = budget(f, usize::MAX);
    let mut b = budget(f, usize::MAX);
    let old = before::signature(&p, f, &fields, expression, &mut a).unwrap();
    let new = signature::<Engine>(&p, f, &fields, expression, &mut b).unwrap();
    assert_eq!(old.len(), new.len());
    for i in 0..old.len() {
        assert_eq!(old.parameter(i), new.parameter(i));
    }
}
#[test]
fn actual_call_controls_exact_short_zero_keep_same_first_ledger_without_refunding() {
    let p = program();
    let f = subject(&p);
    let fee = controls::<()>();
    for available in [fee, fee - 1, 0] {
        let (_, observed) = measure(|| {
            let mut b = budget(f, available);
            let base = b.physical.as_deref().unwrap().charged();
            let text = b.refusal_text.as_ref().unwrap().as_ptr();
            let answer = <Engine as Policy>::enter::<()>(f, &mut b);
            if available == fee {
                let permit = answer.ok().unwrap();
                let error =
                    <Engine as Policy>::static_error(&permit, Kind::CallCommitExpressionIsAbsent);
                assert_eq!(b.physical.as_deref().unwrap().charged(), base + fee);
                drop(error);
                drop(permit);
            } else {
                assert!(answer.is_err());
                let first = b.physical.as_deref_mut().unwrap().reserve(0).unwrap_err();
                let charged = b.physical.as_deref().unwrap().charged();
                let again = <Engine as Policy>::enter::<()>(f, &mut b);
                assert!(again.is_err());
                assert_eq!(
                    b.physical.as_deref_mut().unwrap().reserve(0).unwrap_err(),
                    first
                );
                assert_eq!(b.physical.as_deref().unwrap().charged(), charged);
                assert!(b.refusal_spent);
                assert_eq!(b.refusal_text.as_ref().map(|s| s.as_ptr()), None);
                let _ = text;
                drop(again);
                drop(answer);
            }
            drop(b);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    for allowance in [11, 10, 0] {
        let (_, observed) = measure(|| {
            let mut b = budget(f, fee);
            b.remaining = allowance;
            let answer = <Engine as Policy>::enter::<()>(f, &mut b);
            assert_eq!(answer.is_ok(), allowance == 11);
            assert_eq!(b.remaining, allowance.saturating_sub(11));
            if allowance < 11 {
                assert!(b.refusal_spent);
            }
            drop(answer);
            drop(b);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    // Explicit no-owner compatibility is not implicitly paid by a surrounding account.
    let expression = call(f);
    let fields = fact(expression);
    let mut old = budget(f, 0);
    let mut new = budget(f, 0);
    drop(old.physical.take());
    drop(new.physical.take());
    let a = before::signature(&p, f, &fields, expression, &mut old).unwrap();
    let b = signature::<Engine>(&p, f, &fields, expression, &mut new).unwrap();
    assert_eq!(a.len(), b.len());
}
#[test]
fn dynamic_callee_handle_is_not_relooked_up_and_both_last_owner_drop_orders_balance() {
    let p = program();
    let f = subject(&p);
    for input_first in [false, true] {
        let (_, observed) = measure(|| {
            // Input construction is explicitly unfunded, observed in full here;
            // only the error-construction subwindow claims zero new payload.
            let mut expression = call(f).clone();
            let text = "retained:é\0雪".repeat(64);
            let ResolvedExprKind::Call { callee, .. } = &mut expression.kind else {
                unreachable!()
            };
            *callee = DeclarationId::new(text.clone());
            let pointer = callee.as_str().as_ptr();
            let mut b = budget(f, usize::MAX);
            let fields = fact(&expression);
            let error = signature::<Engine>(&p, f, &fields, &expression, &mut b)
                .err()
                .unwrap();
            drop(fields);
            let Engine::Structural(receipt) = &error else {
                panic!("typed")
            };
            let retained = receipt.call_callee().expect("exact callee");
            assert_eq!(retained.as_str().as_ptr(), pointer);
            if input_first {
                drop(expression);
                assert!(detailed(f, error).message.contains(&text));
            } else {
                drop(error);
                drop(expression);
            }
            drop(text);
            drop(b);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[allow(
    dead_code,
    reason = "complete independently compiled same-host old receiving layout"
)]
mod prior_layout {
    use super::*;
    pub(super) enum Atom {
        SelectedSession(&'static str),
        SelectedChild(crate::cleanup_plan::renewal_profile::child_refusal::Refusal),
        RepeatedStatus(ExpressionId),
        UnknownStatus(ExpressionId),
        NoncontiguousSlot(usize),
        RepeatedStorage(StorageId),
        DeadCallFlag(ExpressionId, LivenessFlagId),
        RepeatedCallFlag(ExpressionId, LivenessFlagId),
        RepeatedVariantEpoch(ExpressionId),
        MismatchedFailure(ExpressionId),
    }
    pub(super) struct Receipt {
        #[cfg_attr(
            not(test),
            allow(
                dead_code,
                reason = "shared source anchor retained with the semantic receipt"
            )
        )]
        source: ExpressionId,
        atom: Atom,
    }
    pub(super) enum EngineError {
        Failed,
        Owned(Diagnostic),
        Fact(expression_facts::errors::FactReceipt),
        Structural(Receipt),
    }
}

#[test]
fn every_same_host_success_receiving_layout_matches_complete_prior_atom_and_error_sum() {
    fn equal<A, B>() {
        assert_eq!(std::mem::size_of::<A>(), std::mem::size_of::<B>());
        assert_eq!(std::mem::align_of::<A>(), std::mem::align_of::<B>());
    }
    use prior_layout as old;
    equal::<old::Atom, semantic_refusal::Atom>();
    equal::<old::Receipt, semantic_refusal::Receipt>();
    equal::<old::EngineError, Engine>();
    equal::<Option<old::EngineError>, Option<Engine>>();
    equal::<Option<old::Receipt>, Option<semantic_refusal::Receipt>>();
    equal::<Result<(), old::EngineError>, Result<(), Engine>>();
    equal::<Result<Signature<'static>, old::EngineError>, Result<Signature<'static>, Engine>>();
    equal::<Result<CallCursor<'static>, old::EngineError>, Result<CallCursor<'static>, Engine>>();
    equal::<
        Result<crate::cleanup_plan::replay::selected_context::Context<'static>, old::EngineError>,
        Result<crate::cleanup_plan::replay::selected_context::Context<'static>, Engine>,
    >();
}
#[test]
fn original_lookup_work_and_every_added_control_byte_have_independent_exact_boundaries() {
    use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
    use crate::cleanup_plan::replay::selected_context::{child, Context};
    fn child_controls<T>() -> usize {
        std::mem::size_of::<(
            child::Deferred,
            Option<child::Deferred>,
            Result<T, child::Deferred>,
            Result<T, child::Deferred>,
        )>() + std::mem::size_of::<(
            child::Permit<'static>,
            Refusal,
            Option<Refusal>,
            semantic_refusal::Receipt,
            Engine,
            Option<Engine>,
            Result<T, Refusal>,
            Result<T, Engine>,
            Result<child::Permit<'static>, Engine>,
            Result<Context<'static>, Engine>,
            Result<(), Engine>,
        )>()
    }
    let p = program();
    let f = subject(&p);
    let mut expression = call(f).clone();
    let ResolvedExprKind::Call { callee, .. } = &mut expression.kind else {
        unreachable!()
    };
    *callee = DeclarationId::new("missing:é");
    let fields = fact(&expression);
    let old_units = (fields.callee.as_str().len().min(64) + 1) * 32 + p.functions().len();
    let new_units = old_units + 11 + 4 + 11;
    let new_bytes = controls::<Signature<'_>>() + child_controls::<Option<bool>>();
    for (old_limit, new_limit) in [
        (old_units, new_units),
        (old_units - 1, new_units - 1),
        (0, 0),
    ] {
        let (_, seen) = measure(|| {
            let mut a = budget(f, usize::MAX);
            let mut b = budget(f, usize::MAX);
            a.remaining = old_limit;
            b.remaining = new_limit;
            let base_a = a.physical.as_deref().unwrap().charged();
            let base_b = b.physical.as_deref().unwrap().charged();
            let old = before::signature(&p, f, &fields, &expression, &mut a)
                .err()
                .unwrap();
            let new = signature::<Engine>(&p, f, &fields, &expression, &mut b)
                .err()
                .unwrap();
            if old_limit != 0 {
                same(&old, &detailed(f, new));
            } else {
                assert!(a.refusal_spent && b.refusal_spent);
                drop(new);
            }
            if old_limit == old_units {
                assert_eq!(a.physical.as_deref().unwrap().charged(), base_a);
                assert_eq!(b.physical.as_deref().unwrap().charged(), base_b + new_bytes);
            }
            assert_eq!(a.remaining, 0);
            assert_eq!(b.remaining, 0);
            drop(a);
            drop(b);
        });
        assert!(!seen.invalid);
        assert_eq!(seen.live, 0);
        assert_eq!(seen.allocated, seen.freed);
    }
    for available in [new_bytes, new_bytes - 1, 0] {
        let (charged, seen) = measure(|| {
            let mut b = budget(f, available);
            let base = b.physical.as_deref().unwrap().charged();
            let answer = signature::<Engine>(&p, f, &fields, &expression, &mut b)
                .err()
                .unwrap();
            if available == new_bytes {
                assert!(matches!(answer, Engine::Structural(_)));
                assert_eq!(b.physical.as_deref().unwrap().charged(), base + new_bytes);
            } else {
                assert!(matches!(answer, Engine::Owned(_)));
                let first = b.physical.as_deref_mut().unwrap().reserve(0).unwrap_err();
                let prior = b.physical.as_deref().unwrap().charged();
                let retry = signature::<Engine>(&p, f, &fields, &expression, &mut b)
                    .err()
                    .unwrap();
                assert_eq!(
                    b.physical.as_deref_mut().unwrap().reserve(0).unwrap_err(),
                    first
                );
                assert_eq!(b.physical.as_deref().unwrap().charged(), prior);
                drop(retry);
            }
            let charged = b.physical.as_deref().unwrap().charged();
            drop(answer);
            drop(b);
            charged
        });
        assert!(!seen.invalid);
        assert_eq!(seen.live, 0);
        assert_eq!(seen.allocated, seen.freed);
        assert!(
            seen.allocated <= charged,
            "all observed current control/setup backing fits same charged owner"
        );
    }
}
#[test]
fn complete_old_and_actual_typed_blocks_keep_call_errors_and_competing_owner_priority_through_drop()
{
    let source = r#"module blocks.custody;
@id("callee") fn callee(n:i64)->i64 {n}
@id("main:é") fn main()->i64 {let key=str"x"; let v=callee(7); if v==7 {v+str_len_bytes(key)}else{0}}
"#;
    let p = crate::hir::resolve(&crate::check(source, "blocks-custody.spx").unwrap()).unwrap();
    for attack in 0..3 {
        let mut input = p.clone();
        let f = input
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "main:é")
            .unwrap();
        if attack != 0 {
            fn change(e: &mut ResolvedExpr) -> bool {
                if let ResolvedExprKind::Call { callee, .. } = &mut e.kind {
                    if callee.as_str() == "callee" {
                        *callee = DeclarationId::new("missing:é");
                        return true;
                    }
                }
                match &mut e.kind {
                    ResolvedExprKind::Block { statements, tail }
                    | ResolvedExprKind::VecConstructor {
                        statements, tail, ..
                    } => {
                        for s in statements {
                            if let ResolvedStatement::Let { value, .. }
                            | ResolvedStatement::Assign { value, .. } = s
                            {
                                if change(value) {
                                    return true;
                                }
                            }
                        }
                        change(tail)
                    }
                    _ => false,
                }
            }
            assert!(change(&mut f.body));
            if attack == 2 {
                let entry = f.cleanup_plan.entry.0 as usize;
                f.cleanup_plan.blocks[entry].id = BlockId(u32::MAX);
            }
        }
        let f = subject(&input);
        let (_, seen) = measure(|| {
            let mut old = budget(f, usize::MAX);
            let mut new = budget(f, usize::MAX);
            // These are complete, independently constructed old/new prerequisites.
            let renewal_a = crate::cleanup_plan::renewal_profile::bindings_with_reservation(
                &input,
                f,
                old.physical.as_deref_mut().unwrap(),
            )
            .unwrap();
            let renewal_b = crate::cleanup_plan::renewal_profile::bindings_with_reservation(
                &input,
                f,
                new.physical.as_deref_mut().unwrap(),
            )
            .unwrap();
            let facts_a = expression_facts::expression_facts_with_reservation(
                f,
                old.physical.as_deref_mut().unwrap(),
            )
            .unwrap();
            let facts_b = expression_facts::expression_facts_with_reservation(
                f,
                new.physical.as_deref_mut().unwrap(),
            )
            .unwrap();
            let slots_a = structural_maps::validate_slots(f, &mut old).unwrap();
            let slots_b = structural_maps::validate_slots(f, &mut new).unwrap();
            let leaves_a = structural_maps::collect_leaves(f, &mut old).unwrap();
            let leaves_b = structural_maps::collect_leaves(f, &mut new).unwrap();
            let a=crate::cleanup_plan::replay::structural_control::blocks_before_receipt::validate_blocks_and_edges(&input,f,&facts_a,&slots_a,&leaves_a,&renewal_a,&mut old);
            let b = crate::cleanup_plan::replay::structural_control::validate_blocks_and_edges_paid::<
                Engine,
            >(
                &input, f, &facts_b, &slots_b, &leaves_b, &renewal_b, &mut new,
            );
            match (a, b) {
                (Ok(()), Ok(())) => assert_eq!(attack, 0),
                (Err(a), Err(b)) => {
                    if attack == 1 {
                        assert!(matches!(&b,Engine::Structural(r) if r.stage()=="selected call"));
                    } else {
                        assert!(
                            matches!(&b, Engine::Owned(_)),
                            "unmigrated competing owner error stays explicit"
                        );
                    }
                    same(&a, &detailed(f, b));
                }
                _ => panic!("full block verdict differs"),
            }
            drop(leaves_b);
            drop(leaves_a);
            drop(slots_b);
            drop(slots_a);
            drop(facts_b);
            drop(facts_a);
            drop(renewal_b);
            drop(renewal_a);
            drop(new);
            drop(old);
        });
        assert!(!seen.invalid);
        assert_eq!(seen.live, 0);
        assert_eq!(seen.allocated, seen.freed);
    }
}

#[test]
fn all_nineteen_finite_call_labels_match_the_original_six_field_diagnostic() {
    let p = program();
    let f = subject(&p);
    let cases = [
        (
            Kind::StructuralCallCursorAllocationRefused,
            "structural call cursor allocation refused",
        ),
        (
            Kind::StructuralCallCursorCapacityDiffersFromReservation,
            "structural call cursor capacity differs from reservation",
        ),
        (
            Kind::StructuralCallLookupDepthExceedsBound,
            "structural call lookup depth exceeds bound",
        ),
        (
            Kind::CallCommitExpressionIsAbsent,
            "call commit expression is absent",
        ),
        (
            Kind::SelectedCallCommitDoesNotSupportGenericCalls,
            "selected call commit does not support generic calls",
        ),
        (
            Kind::SelectedStringTransferCallIdentityDiffers,
            "selected String transfer call identity differs",
        ),
        (
            Kind::SelectedCallCommitDoesNotSupportEffectfulCallees,
            "selected call commit does not support effectful callees",
        ),
        (
            Kind::SelectedCallLacksItsLookupCursor,
            "selected call lacks its lookup cursor",
        ),
        (
            Kind::CallCommitDoesNotNameASelectedOrdinaryCall,
            "call commit does not name a selected ordinary call",
        ),
        (
            Kind::CallCommitFactsDisagreeWithCurrentTypedCall,
            "call commit facts disagree with current typed call",
        ),
        (
            Kind::CallCommitCalleeSignatureHasInconsistentArity,
            "call commit callee signature has inconsistent arity",
        ),
        (
            Kind::CallCommitArgumentDisagreesWithExactParameterTypeOrOwnership,
            "call commit argument disagrees with exact parameter type or ownership",
        ),
        (Kind::TooManyCallParameters, "too many call parameters"),
        (
            Kind::CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter,
            "call commit does not contain every and only owned droppable parameter",
        ),
        (
            Kind::CallCommitParametersAreNotInStrictSignatureOrder,
            "call-commit parameters are not in strict signature order",
        ),
        (
            Kind::CallCommitParameterIndexDoesNotFitUsize,
            "call-commit parameter index does not fit usize",
        ),
        (
            Kind::CallCommitParameterIndexExceedsCallArity,
            "call-commit parameter index exceeds call arity",
        ),
        (
            Kind::CallCommitDoesNotConsumeItsMatchingWholeArgumentEpoch,
            "call commit does not consume its matching whole argument epoch",
        ),
        (
            Kind::AtomicCallCommitConsumesOverlappingArgumentEpochs,
            "atomic call commit consumes overlapping argument epochs",
        ),
    ];
    assert_eq!(cases.len(), 19);
    for (kind, literal) in cases {
        let mut b = budget(f, controls::<()>());
        let permit = <Engine as Policy>::enter::<()>(f, &mut b).ok().unwrap();
        let (error, observed) = measure(|| <Engine as Policy>::static_error(&permit, kind));
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 0);
        let old = crate::cleanup_plan::replay::replay_error_old(f, literal.to_owned());
        same(&old, &detailed(f, error));
    }
}

#[test]
fn actual_borrowed_receiving_controls_are_paid_before_the_child_at_exact_short_zero() {
    type Controls<'a> = (
        Option<crate::cleanup_plan::ReplayBorrowedLease<'a>>,
        Result<(&'a ResolvedExpr, schema::borrowed::Call<'a>), Diagnostic>,
        Option<(&'a ResolvedExpr, schema::borrowed::Call<'a>)>,
        &'a Option<crate::cleanup_plan::ReplayBorrowedLease<'a>>,
    );
    let p = program();
    let f = subject(&p);
    let bytes = std::mem::size_of::<Controls<'_>>();
    for available in [bytes, bytes - 1, 0] {
        let mut b = budget(f, available);
        let base = b.physical.as_deref().unwrap().charged();
        let answer = <Engine as Policy>::borrowed_controls(f, &mut b);
        assert_eq!(answer.is_ok(), available == bytes);
        if available == bytes {
            assert_eq!(b.physical.as_deref().unwrap().charged(), base + bytes);
        } else {
            let first = b.physical.as_deref_mut().unwrap().reserve(0).unwrap_err();
            let prior = b.physical.as_deref().unwrap().charged();
            assert!(<Engine as Policy>::borrowed_controls(f, &mut b).is_err());
            assert_eq!(
                b.physical.as_deref_mut().unwrap().reserve(0).unwrap_err(),
                first
            );
            assert_eq!(b.physical.as_deref().unwrap().charged(), prior);
        }
    }
    for allowance in [4, 3, 0] {
        let mut b = budget(f, bytes);
        b.remaining = allowance;
        assert_eq!(
            <Engine as Policy>::borrowed_controls(f, &mut b).is_ok(),
            allowance == 4
        );
        assert_eq!(b.remaining, allowance.saturating_sub(4));
    }
}

mod source;
