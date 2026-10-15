use super::*;

type CanonicalLoans = (
    BTreeMap<(ExpressionId, LoanCause), LoanId>,
    BTreeMap<(ExpressionId, LoanPointPhase, LoanId), Place>,
);

fn old(program: &ResolvedProgram) -> Result<CanonicalLoans, Diagnostic> {
    let mut canonical_loan_ids = BTreeMap::new();
    let mut canonical_loan_liveness = BTreeMap::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        let plan = crate::loan_plan::build_plan(program, function)?;
        let origins = plan
            .loans
            .iter()
            .map(|loan| (loan.id, loan.origin.clone()))
            .collect::<BTreeMap<_, _>>();
        for endpoint in &plan.endpoints {
            let live = match endpoint.point.phase {
                LoanPointPhase::Before => &endpoint.live_before,
                LoanPointPhase::After => &endpoint.live_after,
            };
            for loan in live {
                let origin = origins.get(loan).cloned().ok_or_else(|| {
                    hir_error("canonical shared-loan endpoint names an unknown loan")
                })?;
                if canonical_loan_liveness
                    .insert(
                        (
                            endpoint.point.expression.clone(),
                            endpoint.point.phase,
                            *loan,
                        ),
                        origin,
                    )
                    .is_some()
                {
                    return Err(hir_error("canonical shared-loan liveness is duplicated"));
                }
            }
        }
        for loan in plan.loans {
            if loan.start.phase != LoanPointPhase::Before
                || canonical_loan_ids
                    .insert((loan.site, loan.cause), loan.id)
                    .is_some()
            {
                return Err(hir_error(
                    "canonical shared-loan identity is ambiguous or malformed",
                ));
            }
        }
    }
    Ok((canonical_loan_ids, canonical_loan_liveness))
}

fn fixture() -> ResolvedProgram {
    let source = r#"module validation.origins;
@id("packet") record Packet { @id("payload") payload:Bytes, @id("sibling") sibling:Bytes }
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);
 let a=byte_len(view); let b=byte_len(view); let c=byte_len(view);
 if a==b {byte_len(view)} else {c+byte_len(view)}
}
@id("second") fn second(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.sibling); byte_len(view)+byte_len(view)
}
@id("main") fn main()->i64 {0}
"#;
    let source = crate::check(source, "canonical-origin-directory.spx").unwrap();
    crate::hir::resolve(&source).unwrap()
}

#[test]
fn independent_origin_directory_matches_saved_owned_endpoints_and_current_projected_roots() {
    let program = fixture();
    let (ids, live) = old(&program).unwrap();
    let directory = Inventory::rebuild(&program).unwrap();
    assert_eq!(directory.ids, ids);
    assert_eq!(directory.liveness.len(), live.len());
    for (key, place) in live {
        let ordinal = directory.liveness.get(&key).unwrap();
        assert_eq!(directory.origins.get(*ordinal), Some(&place));
    }
    // One SliceView plus one BorrowedCall per authored byte_len read.
    // Canonical construction retains call-long loans as well as local views.
    for (id, reads) in [("first", 5), ("second", 2)] {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        let plan = crate::loan_plan::build_plan(&program, function).unwrap();
        assert_eq!(
            plan.loans
                .iter()
                .filter(|loan| loan.cause == LoanCause::SliceView)
                .count(),
            1
        );
        assert_eq!(
            plan.loans
                .iter()
                .filter(|loan| matches!(loan.cause, LoanCause::BorrowedCall { argument: 0 }))
                .count(),
            reads
        );
        assert_eq!(plan.loans.len(), 1 + reads);
    }
    assert_eq!(directory.origins.len(), 9);
    assert!(directory
        .origins
        .iter()
        .all(|place| place.projections.len() == 1));
    for (field, count) in [("payload", 6), ("sibling", 3)] {
        let projected = vec![PlaceProjection::Field(DeclarationId::new(field))];
        assert_eq!(
            directory
                .origins
                .iter()
                .filter(|place| place.projections == projected)
                .count(),
            count
        );
    }
    assert!(
        directory
            .liveness
            .keys()
            .filter(|(_, _, loan)| *loan == LoanId(0))
            .count()
            > 2
    );
    let validator = HirValidator::new(&program).unwrap();
    validator.validate().unwrap();
    let mut drift = program.clone();
    let first = drift
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "first")
        .unwrap();
    first.loan_plan.loans[0].origin.projections[0] =
        PlaceProjection::Field(DeclarationId::new("sibling"));
    assert_eq!(crate::hir::validate(&drift).unwrap_err().code, "SPX-H006");
}

#[test]
fn endpoint_origin_payload_is_moved_once_and_late_refusals_keep_saved_priority() {
    let program = fixture();
    let (_, saved_allocations) = crate::test_allocation_observer::measure(|| {
        let owned = old(&program).unwrap();
        drop(owned);
    });
    let (_, directory_allocations) = crate::test_allocation_observer::measure(|| {
        let directory = Inventory::rebuild(&program).unwrap();
        drop(directory);
    });
    assert!(!saved_allocations.invalid && !directory_allocations.invalid);
    assert_eq!(saved_allocations.live, 0);
    assert_eq!(directory_allocations.live, 0);
    assert!(directory_allocations.allocated < saved_allocations.allocated);
    // This is a relative physical-allocation observation through Drop. It
    // does not stand in for the separately owned whole-validator payer proof.
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "first")
        .unwrap();
    let plan = crate::loan_plan::build_plan(&program, function).unwrap();
    let expected_projection = plan.loans[0].origin.projections.as_ptr();
    let expected_field = match &plan.loans[0].origin.projections[0] {
        PlaceProjection::Field(field) => field.as_str().as_ptr(),
        _ => panic!("field"),
    };
    let mut inventory = Inventory {
        ids: BTreeMap::new(),
        liveness: BTreeMap::new(),
        origins: Vec::new(),
    };
    inventory.append(plan).unwrap();
    assert_eq!(
        inventory.origins[0].projections.as_ptr(),
        expected_projection
    );
    let PlaceProjection::Field(field) = &inventory.origins[0].projections[0] else {
        panic!("field")
    };
    assert_eq!(field.as_str().as_ptr(), expected_field);
    assert_eq!(inventory.origins.len(), 6);
    for ((_, _, loan), ordinal) in &inventory.liveness {
        assert_eq!(*ordinal, usize::from(loan.0));
        assert_eq!(inventory.origins[*ordinal], inventory.origins[0]);
    }
    assert!(inventory.liveness.len() > 1);
    // Both faults are present: endpoints are checked before start-phase/ID
    // validation, exactly as the saved independent constructor did.
    let mut plan = crate::loan_plan::build_plan(&program, function).unwrap();
    plan.loans[0].start.phase = LoanPointPhase::After;
    assert!(plan.loans.iter().all(|loan| loan.id != LoanId(255)));
    let endpoint = plan
        .endpoints
        .iter_mut()
        .find(|endpoint| match endpoint.point.phase {
            LoanPointPhase::Before => !endpoint.live_before.is_empty(),
            LoanPointPhase::After => !endpoint.live_after.is_empty(),
        })
        .unwrap();
    // Mutate the endpoint's active phase, rather than an inactive Before
    // inventory on an After endpoint. The malformed start remains present;
    // the authenticated unknown endpoint must keep its original priority.
    let active = match endpoint.point.phase {
        LoanPointPhase::Before => &mut endpoint.live_before,
        LoanPointPhase::After => &mut endpoint.live_after,
    };
    *active = vec![LoanId(255)].into_boxed_slice();
    let mut empty = Inventory {
        ids: BTreeMap::new(),
        liveness: BTreeMap::new(),
        origins: Vec::new(),
    };
    assert_eq!(
        empty.append(plan).unwrap_err().message,
        "canonical shared-loan endpoint names an unknown loan"
    );
}
