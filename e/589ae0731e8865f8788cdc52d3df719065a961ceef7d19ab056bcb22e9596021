//! Actual cursor storage, original traversal, refusal and allocation controls.
use super::*;
use crate::test_allocation_observer::measure;

fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module types.funding; @id(\"types.main\") fn main() -> i64 { 0 }",
            "types-funding.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn paid(owner: &FactCapacity<'_, '_>) -> usize {
    match &owner.physical {
        Payer::Selected(payer) => payer.charged(),
        _ => unreachable!(),
    }
}
fn growth(slots: usize) -> usize {
    2 * size_of::<TypeCursor<'_>>()
        + size_of::<TypeFrame<'_>>()
        + slots * size_of::<TypeFrame<'_>>()
}
const SIZES: [usize; 10] = [2, 4, 8, 16, 32, 64, 128, 256, 512, 514];

#[test]
fn type_cursor_all_growth_reuses_one_owner_and_releases_complete_backing() {
    let program = program();
    let function = &program.functions[0];
    let mut physical = PhysicalReservation::new(usize::MAX);
    let (_, metrics) = measure(|| {
        let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
        let initial = paid(&owner);
        assert!(owner.types.is_empty());
        let mut expected = initial;
        let mut work = 0;
        for slots in SIZES {
            owner.grow_types(slots).unwrap();
            expected += growth(slots);
            work += slots;
            assert_eq!(paid(&owner), expected);
            assert_eq!(owner.work, MAX_REPLAY_WORK_UNITS - work);
            assert_eq!(owner.types.len(), slots);
            owner.types[0] = Some((&function.return_type, 7));
            let before = paid(&owner);
            owner.grow_types(slots).unwrap();
            assert_eq!(paid(&owner), before);
            assert_eq!(owner.types[0].unwrap().1, 7);
            assert!(std::ptr::eq(
                owner.types[0].unwrap().0,
                &function.return_type
            ));
        }
        let before = paid(&owner);
        let work = owner.work;
        assert_eq!(
            owner.grow_types(515).unwrap_err(),
            Failure {
                detail: "expression-fact type depth exceeds limit",
                physical: false
            }
        );
        assert_eq!(paid(&owner), before);
        assert_eq!(owner.work, work);
        drop(owner);
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(
        metrics.allocated,
        SIZES.iter().sum::<usize>() * size_of::<TypeFrame<'_>>()
    );
    assert_eq!(metrics.highwater, (512 + 514) * size_of::<TypeFrame<'_>>());
}

#[test]
fn type_cursor_each_growth_exact_and_one_short_are_sticky_and_do_not_allocate_on_refusal() {
    let program = program();
    let function = &program.functions[0];
    let header =
        size_of::<ArenaCapacity<'_, '_>>() + 2 * size_of::<FactFrame<'_>>() + size_of::<usize>();
    for boundary in 0..SIZES.len() {
        let exact = header
            + SIZES[..=boundary]
                .iter()
                .copied()
                .map(growth)
                .sum::<usize>();
        for allowance in [exact - 1, exact] {
            let mut physical = PhysicalReservation::new(allowance);
            let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
            for slots in &SIZES[..boundary] {
                owner.grow_types(*slots).unwrap();
            }
            let previous = owner.types.as_ptr();
            let length = owner.types.len();
            let (result, observation) = measure(|| owner.grow_types(SIZES[boundary]));
            if allowance == exact {
                result.unwrap();
                assert_eq!(paid(&owner), exact);
            } else {
                assert!(result.unwrap_err().physical);
                assert_eq!(observation.allocated, 0);
                let work = owner.work;
                assert!(owner.grow_types(length).unwrap_err().physical);
                assert_eq!(owner.work, work);
                assert_eq!(owner.types.as_ptr(), previous);
                assert_eq!(owner.types.len(), length);
            }
        }
    }
}

#[test]
fn type_cursor_matches_complete_original_for_primitives_and_exact_depth_boundary() {
    let program = program();
    let function = &program.functions[0];
    let nominal = |argument| ResolvedType::Nominal {
        declaration: crate::hir::DeclarationId::new("types.node"),
        arguments: vec![argument],
    };
    let mut cases = vec![
        ResolvedType::I64,
        ResolvedType::Bool,
        ResolvedType::Bytes,
        ResolvedType::TypeParameter {
            owner: crate::hir::DeclarationId::new("types.owner"),
            index: 0,
        },
    ];
    for depth in [1, 2, 3, 31, 513, 514] {
        let mut ty = ResolvedType::I64;
        for _ in 0..depth {
            ty = nominal(ty);
        }
        cases.push(ty);
    }
    for ty in &cases {
        let (old, original_work) = before_types::type_control(function, ty);
        let mut physical = PhysicalReservation::new(usize::MAX);
        let mut owner = ArenaCapacity::new(function, Payer::Selected(&mut physical)).unwrap();
        let current = owner
            .types(ty)
            .map_err(|error| (error.detail, error.physical));
        assert_eq!(current, old);
        assert!(MAX_REPLAY_WORK_UNITS - owner.work >= original_work);
        if current.is_ok() {
            assert!(owner.types.iter().all(Option::is_none));
            let paid = paid(&owner);
            owner.types(ty).unwrap();
            // Type clone payload is still reserved on each traversal; backing
            // slots alone are reused, never another function's allowance.
            assert!(self::paid(&owner) >= paid);
        }
    }
}

#[test]
fn type_cursor_live_budget_zero_short_exact_preserves_sticky_refusal() {
    let program = program();
    let function = &program.functions[0];
    let exact = size_of::<ArenaCapacity<'_, '_>>()
        + 2 * size_of::<FactFrame<'_>>()
        + size_of::<usize>()
        + FactCapacity::TYPE_SELECTOR
        + FactCapacity::TYPE_GROW_CALL
        + growth(2);
    for limit in [0, exact - 1, exact] {
        let ((accepted, metrics), refused, consumed) =
            crate::bounded_output::with_limit_usage(limit, || {
                measure(|| {
                    let Ok(mut owner) = ArenaCapacity::new(function, Payer::ActiveBuilder) else {
                        return false;
                    };
                    if owner.types(&function.return_type).is_err() {
                        assert!(owner.grow_types(0).unwrap_err().physical);
                        return false;
                    }
                    true
                })
            });
        assert_eq!(accepted, limit == exact);
        assert_eq!(refused, limit != exact);
        assert!(consumed <= limit);
        assert_eq!(metrics.live, 0);
        assert!(!metrics.invalid);
    }
}

// The old source guards still run against the complete immediate predecessor.
// Authenticate the entire new source before applying only its exact inverse.
pub(in crate::cleanup_plan::replay::expression_facts) fn immediate_before(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let spec: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/evidence/expression-fact-type-cursor-v1/source-inverse.json"
    ))
    .unwrap();
    let digest = |value: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
        )
    };
    assert_eq!(
        digest(current),
        spec["current_sha256"].as_str().unwrap(),
        "complete current type cursor source"
    );
    let mut restored = current.to_owned();
    for change in spec["inverse"].as_array().unwrap().iter().rev() {
        let begin = change["begin"].as_u64().unwrap() as usize;
        let end = change["end"].as_u64().unwrap() as usize;
        assert_eq!(&restored[begin..end], change["current"].as_str().unwrap());
        restored.replace_range(begin..end, change["previous"].as_str().unwrap());
    }
    assert_eq!(digest(&restored), spec["previous_sha256"].as_str().unwrap());
    assert_eq!(
        restored,
        include_str!(
            "../../../../../docs/evidence/expression-fact-type-cursor-v1/capacity.before.txt"
        )
    );
    restored
}

#[test]
fn type_cursor_source_inverse_rejects_whole_source_drift_and_retains_complete_old_module() {
    let current = include_str!("../capacity.rs");
    let old = immediate_before(current);
    let retained = include_str!("before_types.rs");
    let start = retained
        .find("// BEGIN IMMEDIATE TYPE TEST RECEIVER\n")
        .unwrap();
    let end = retained
        .find("// END IMMEDIATE TYPE TEST RECEIVER\n\n")
        .unwrap()
        + "// END IMMEDIATE TYPE TEST RECEIVER\n\n".len();
    let retained = format!("{}{}", &retained[..start], &retained[end..]);
    let retained = retained
        .replace("use super::super::*;", "use super::*;")
        .replace(
            "#[path = \"cursor_tests.rs\"]",
            "#[path = \"capacity/cursor_tests.rs\"]",
        );
    assert_eq!(retained.trim_end(), old.trim_end());
    let tests = include_str!("cursor_tests.rs");
    let annotation = "#![allow(\n    clippy::duplicate_mod,\n    reason = \"Current and complete immediate-before owners independently compile the original cursor tests.\"\n)]\n";
    assert_eq!(
        tests.strip_prefix(annotation).unwrap(),
        include_str!(
            "../../../../../docs/evidence/expression-fact-type-cursor-v1/cursor-tests.before.txt"
        ),
        "all original cursor test bytes survive the narrow duplicate-module annotation"
    );
    for changed in [
        current[..current.len() - 1].to_owned(),
        format!("{current} "),
        current.replacen("saturating_mul(2)", "saturating_mul(3)", 1),
    ] {
        assert_ne!(changed, current);
        assert!(std::panic::catch_unwind(|| immediate_before(&changed)).is_err());
    }
}
