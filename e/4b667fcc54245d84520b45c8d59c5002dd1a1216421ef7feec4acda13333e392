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
    let setup = size_of::<physical_storage::PhysicalReservation>()
        + 514 * size_of::<Option<(&ResolvedType, usize)>>()
        + text;
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
