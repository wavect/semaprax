use super::*;
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedFunction {
    let source = crate::check(
        "module clone.payload; @id(\"main\") fn main()->i64 {0}",
        "clone.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap().functions.remove(0)
}
fn nested(shared: &DeclarationId) -> ResolvedType {
    ResolvedType::Function {
        parameters: vec![ResolvedType::Nominal {
            declaration: shared.clone(),
            arguments: vec![
                ResolvedType::I64,
                ResolvedType::TypeParameter {
                    owner: shared.clone(),
                    index: 0,
                },
            ],
        }],
        result: Box::new(ResolvedType::Nominal {
            declaration: DeclarationId::compiler(crate::hir::CompilerDeclaration::Vec),
            arguments: Vec::new(),
        }),
    }
}
#[test]
fn static_and_shared_clone_census_keeps_real_type_backing_without_identity_text() {
    // Original backing acquisition is outside the clone-only window. Neither
    // this census nor sharing creates source/type admission authority.
    let shared = DeclarationId::new("clone:é\0.long.shared.identity");
    let constant = DeclarationId::compiler(crate::hir::CompilerDeclaration::Vec);
    let (_, identities) = measure(|| {
        for _ in 0..1000 {
            let copy = shared.clone();
            let static_copy = constant.clone();
            assert_eq!(copy, shared);
            assert_eq!(static_copy, constant);
            assert_eq!(copy.clone_payload_bytes(), Some(0));
            assert_eq!(static_copy.clone_payload_bytes(), Some(0));
            drop((copy, static_copy));
        }
    });
    assert!(!identities.invalid);
    assert_eq!(identities.allocated, 0);
    assert_eq!(identities.live, 0);
    // Generic instance identities still own String backing; this tranche
    // cannot discount them along with DeclarationId's valid shared carrier.
    let generic = FunctionInstanceId::derive(&shared, &[ResolvedType::I64]);
    let generic_bytes = generic.as_str().len();
    assert_eq!(generic.clone_payload_bytes(), Some(generic_bytes));
    let (_, generic_copy) = measure(|| {
        let copy = generic.clone();
        assert_eq!(copy, generic);
        drop(copy);
    });
    assert!(!generic_copy.invalid);
    assert_eq!(generic_copy.allocated, generic_bytes);
    assert_eq!(generic_copy.freed, generic_bytes);
    assert_eq!(generic_copy.live, 0);
    let value = nested(&shared);
    // One parameter slot, two nominal argument slots and one result Box.
    let expected = 4 * size_of::<ResolvedType>();
    // Saved String-backed clone census, independently enumerating the two
    // shared spellings and the static result declaration's former String.
    let old = expected + 2 * shared.as_str().len() + "core.vec".len();
    assert!(old > expected);
    assert_eq!(value.clone_payload_bytes(), Some(expected));
    let (_, copies) = measure(|| {
        for _ in 0..32 {
            let copy = value.clone();
            assert_eq!(copy, value);
            drop(copy);
        }
    });
    assert!(!copies.invalid);
    assert_eq!(copies.allocations, 3 * 32);
    assert_eq!(copies.allocated, expected * 32);
    assert_eq!(copies.freed, copies.allocated);
    assert_eq!(copies.live, 0);
}
#[test]
fn real_reservation_clone_exact_short_and_zero_retry_keep_first_paid_refusal() {
    let function = fixture();
    let shared = DeclarationId::new("clone:é\0.shared");
    let value = nested(&shared);
    let payload = 4 * size_of::<ResolvedType>();
    // Actual selected setup: affine ledger Box, fixed census stack and paid
    // refusal String. The stack is charged, but makes no heap request.
    let text = function.id.as_str().len() + 512;
    // Independently enumerate the actual inline direct-anchor receiving
    // controls added to selected setup. These are stack/inline reservations,
    // so the original heap-allocation assertion below remains unchanged.
    let anchor_controls = 3 * size_of::<Option<SelectedDirectAnchor>>()
        + 2 * size_of::<SelectedDirectAnchor>()
        + size_of::<crate::bounded_output::classifier_string::ClassifierLedger>()
        + crate::bounded_output::classifier_string::ClassifierLedger::capture_into_controls()
        + 2 * size_of::<*const ResolvedFunction>()
        + 2 * size_of::<&ResolvedFunction>()
        + size_of::<&mut crate::bounded_output::classifier_string::ClassifierLedger>()
        + 2 * size_of::<Result<(), physical_storage::PhysicalRefusal>>()
        + 2 * size_of::<(&mut physical_storage::PhysicalReservation, usize)>()
        + 2 * size_of::<bool>();
    let setup = size_of::<physical_storage::PhysicalReservation>()
        + 514 * size_of::<Option<(&ResolvedType, usize)>>()
        + text
        + anchor_controls;
    let (_, baseline) = measure(|| {
        let budget = ReplayBudget::from_reservation(
            &function,
            physical_storage::PhysicalReservation::new(setup),
        )
        .unwrap();
        assert_eq!(budget.physical.as_ref().unwrap().charged(), setup);
        drop(budget);
    });
    assert!(!baseline.invalid);
    assert_eq!(
        baseline.allocated,
        size_of::<physical_storage::PhysicalReservation>() + text
    );
    assert_eq!(baseline.allocated, baseline.freed);
    let (_, exact) = measure(|| {
        let mut budget = ReplayBudget::from_reservation(
            &function,
            physical_storage::PhysicalReservation::new(setup + payload),
        )
        .unwrap();
        budget
            .reserve_clone(&function, &value, "type clone")
            .unwrap();
        let copy = value.clone();
        assert_eq!(copy, value);
        assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + payload);
        // A successful zero-payload clone still consults this same ledger.
        budget
            .reserve_clone(&function, &shared, "shared clone")
            .unwrap();
        drop(shared.clone());
        drop(copy);
        drop(budget);
    });
    assert!(!exact.invalid);
    assert_eq!(exact.allocated, baseline.allocated + payload);
    assert_eq!(exact.allocated, exact.freed);
    assert_eq!(exact.live, 0);
    let (_, short) = measure(|| {
        let mut budget = ReplayBudget::from_reservation(
            &function,
            physical_storage::PhysicalReservation::new(setup + payload - 1),
        )
        .unwrap();
        let first = budget
            .reserve_clone(&function, &value, "type clone")
            .unwrap_err();
        assert_eq!(first.code, "SPX-H006");
        assert!(first.message.contains("type clone"));
        assert_eq!(budget.physical.as_ref().unwrap().charged(), setup);
        let retry = budget
            .reserve_clone(&function, &shared, "zero clone retry")
            .unwrap_err();
        assert_eq!(retry.code, first.code);
        assert!(retry.message.is_empty());
        assert_eq!(budget.physical.as_ref().unwrap().charged(), setup);
        drop((retry, first, budget));
    });
    assert!(!short.invalid);
    assert_eq!(short.allocated, baseline.allocated);
    assert_eq!(short.allocated, short.freed);
    assert_eq!(short.live, 0);
}

use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::current_metrics;
fn cursor_program() -> ResolvedProgram {
    let parsed = crate::check(
        "module cursor.test; @id(\"main\") fn main()->i64 {0}",
        "cursor.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed).unwrap()
}
#[test]
fn staged_copy_sources_keep_all_fields_and_pay_recursive_cursor_only_when_used() {
    let program = cursor_program();
    let function = &program.functions[0];
    let expression = function.body.id.clone();
    let declaration = DeclarationId::new("cursor.stage.identity");
    let compound = ResolvedType::Function {
        parameters: vec![ResolvedType::I64; 3],
        result: Box::new(ResolvedType::I64),
    };
    let values = [
        StagedCopyResultSource::Body {
            expression: expression.clone(),
            instance: ResolvedType::I64,
        },
        StagedCopyResultSource::TryResidual {
            expression: expression.clone(),
            operand: expression.clone(),
            source_instance: ResolvedType::I64,
            target_instance: ResolvedType::I64,
            result: declaration.clone(),
            ok_case: declaration.clone(),
            ok_field: declaration.clone(),
            err_case: declaration.clone(),
            err_field: declaration.clone(),
        },
        StagedCopyResultSource::TryOptionNone {
            expression: expression.clone(),
            operand: expression.clone(),
            source_instance: ResolvedType::I64,
            target_instance: ResolvedType::I64,
            option: declaration.clone(),
            some_case: declaration.clone(),
            some_field: declaration.clone(),
            none_case: declaration.clone(),
        },
        StagedCopyResultSource::Body {
            expression,
            instance: compound,
        },
    ];
    const {
        assert!(
            <StagedCopyResultSource as ClonePayload>::NEEDS_TYPE_CURSOR,
            "future payloads remain conservative"
        );
    }
    let text = function.id.as_str().len() + 512;
    let cursor = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    for (index, value) in values.iter().enumerate() {
        let recursive = index == 3;
        assert_eq!(value.clone_needs_type_cursor(), recursive);
        let payload = value.clone_payload_bytes().unwrap();
        if !recursive {
            assert_eq!(
                value.clone_payload_bytes_without_type_cursor(),
                Some(payload)
            );
        }
        let exact = text + usize::from(recursive) * cursor + payload;
        for floor in [0, 37] {
            for available in [0, text - 1, exact - 1, exact, exact + 1] {
                let (((), metrics), refused, used, receipt) =
                    with_limit_usage_refusal(available + floor, || {
                        assert!(set_active_floor(floor));
                        measure(|| {
                            let mut budget = ReplayBudget::new();
                            let result =
                                budget.reserve_clone(function, value, "actual staged copy source");
                            if available >= exact {
                                result.unwrap();
                                assert_eq!(budget.builder_clone_cursor_paid, recursive);
                                let copy = value.clone();
                                assert_eq!(&copy, value);
                                drop(copy);
                            } else {
                                let error = result.unwrap_err();
                                let before = current_metrics();
                                assert!(budget
                                    .reserve_clone(function, value, "sticky staged retry")
                                    .is_err());
                                assert_eq!(current_metrics(), before);
                                drop(error);
                            }
                            drop(budget);
                        })
                    });
                assert_eq!(refused, available < exact);
                assert!(!metrics.invalid);
                assert_eq!(metrics.live, 0);
                assert_eq!(metrics.allocated, metrics.freed);
                if !refused {
                    assert!(receipt.is_none());
                    assert_eq!(used, exact);
                    assert_eq!(metrics.allocated, text + payload);
                } else {
                    assert!(receipt.is_some());
                    assert!(used <= available);
                }
            }
        }
    }
}

#[test]
fn ordinary_hir_setup_leaves_unused_cursor_unpaid_then_funds_real_recursive_clone() {
    let program = cursor_program();
    let function = &program.functions[0];
    let compound = ResolvedType::Function {
        parameters: vec![ResolvedType::I64],
        result: Box::new(ResolvedType::I64),
    };
    let (((), metrics), refused, _, receipt) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let paths =
                hir_skeleton_paths_with_index(&program, function, &mut budget, None).unwrap();
            assert!(
                !budget.builder_clone_cursor_paid,
                "flat actual HIR receiver constructs no recursive cursor"
            );
            drop(paths);
            budget
                .reserve_clone(function, &compound, "later real recursive type")
                .unwrap();
            assert!(budget.builder_clone_cursor_paid);
            let copy = compound.clone();
            assert_eq!(copy, compound);
            drop((copy, budget));
        })
    });
    assert!(!refused && receipt.is_none() && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
