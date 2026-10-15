//! Option metadata admits only replayed monomorphic flat-record entries.
use super::super::flat_record::tests::{append, program, row, row_type};
use super::*;
fn option_u8() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::OPTION_ID),
        arguments: vec![ResolvedType::U8],
    }
}
fn replayed(program: &crate::hir::ResolvedProgram) -> DeclarationIndex {
    let mut index = DeclarationIndex::default();
    let mut physical = ReplayPhysicalReservation::new(1 << 20);
    append(&mut index, row(program), &mut physical, &mut |_| Ok(())).unwrap();
    index
}
fn run(
    index: &DeclarationIndex,
    work: usize,
    bytes: usize,
) -> (Result<(usize, usize), Refused>, DeclarationIndex) {
    let mut index = index.clone();
    let mut remaining = work;
    let mut physical = ReplayPhysicalReservation::new(bytes);
    let result = index.append_option_u8_replay_metered(&mut physical, &mut |units| {
        remaining = remaining
            .checked_sub(units)
            .ok_or(Refused::constructor("exact Option work exhausted"))?;
        Ok(())
    });
    (
        result.map(|()| (work - remaining, physical.charged())),
        index,
    )
}
#[test]
fn option_u8_replay_admits_replayed_flat_records_at_exact_and_one_short_work() {
    let program = program();
    let base = replayed(&program);
    let (empty, _) = run(&DeclarationIndex::default(), 1 << 20, 1 << 20);
    let (empty_work, _) = empty.unwrap();
    let (wide, index) = run(&base, 1 << 20, 1 << 20);
    let (work, bytes) = wide.unwrap();
    // Every retained record key, field key and parameter entry is charged.
    assert!(work > empty_work, "{work} <= {empty_work}");
    assert!(crate::hir::borrowed_scalar::option_u8::authenticated(
        &index,
        &option_u8()
    ));
    assert!(crate::hir::copy_record_collection::admitted(
        &index,
        &row_type()
    ));
    assert_eq!(
        index.type_parameters(&DeclarationId::new("app.Row")),
        Some(&[][..])
    );
    let (exact, exact_index) = run(&base, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert_eq!(exact_index, index);
    assert!(run(&base, work - 1, bytes).0.is_err());
    assert!(run(&base, work, bytes - 1).0.is_err());
}
/// A retained identity is a fixed compiler identity equal to its owned text.
fn compiler(id: &DeclarationId, oracle: &str) {
    assert!(id.is_compiler_static(), "{id:?}");
    assert_eq!(id.owned_allocation_bytes(), 0);
    assert_eq!(*id, DeclarationId::new(oracle.to_owned()));
}
#[test]
fn option_u8_replay_builds_only_compiler_identities_within_its_reservation_through_drop() {
    use crate::prelude::{OPTION_ID, OPTION_NONE_ID, OPTION_SOME_ID, OPTION_SOME_VALUE_ID};
    let (result, index) = run(&DeclarationIndex::default(), 1 << 20, 1 << 20);
    let (_, charged) = result.unwrap();
    let (variant, cases) = index.variant_cases.get_key_value(OPTION_ID).unwrap();
    compiler(variant, OPTION_ID);
    compiler(&cases[0].id, OPTION_NONE_ID);
    compiler(&cases[1].id, OPTION_SOME_ID);
    let field = &cases[1].fields[0];
    compiler(&field.id, OPTION_SOME_VALUE_ID);
    let ResolvedType::TypeParameter { owner, .. } = &field.ty else {
        panic!("Option field type")
    };
    compiler(owner, OPTION_ID);
    let case_fields = index.case_fields.iter().collect::<Vec<_>>();
    let [(none, no_fields), (some, some_fields)] = case_fields.as_slice() else {
        panic!("two Option cases")
    };
    compiler(none, OPTION_NONE_ID);
    compiler(some, OPTION_SOME_ID);
    assert!(no_fields.is_empty());
    compiler(&some_fields[0].id, OPTION_SOME_VALUE_ID);
    let oracle = [
        (OPTION_ID, None),
        (OPTION_NONE_ID, Some(OPTION_ID)),
        (OPTION_SOME_ID, Some(OPTION_ID)),
        (OPTION_SOME_VALUE_ID, Some(OPTION_SOME_ID)),
    ];
    assert_eq!(index.declarations.len(), oracle.len());
    for (text, owner) in oracle {
        let (key, declaration) = index.declarations.get_key_value(text).unwrap();
        compiler(key, text);
        compiler(&declaration.id, text);
        match (&declaration.owner, owner) {
            (Some(actual), Some(expected)) => compiler(actual, expected),
            (None, None) => {}
            _ => panic!("Option declaration owner"),
        }
    }
    compiler(
        index.type_parameters.get_key_value(OPTION_ID).unwrap().0,
        OPTION_ID,
    );
    drop(index);
    // Saved DeclarationIndex Drop drains exactly two field types here: the
    // Some field in variant_cases and its separate case_fields inventory.
    // Its Vec frontier is real temporary storage, not an identity clone.
    // Observe that independent two-element frontier through Drop rather than
    // assuming a private Vec minimum capacity or hiding its allocation.
    let ((), drop_frontier) = crate::test_allocation_observer::measure(|| {
        let mut pending = Vec::new();
        for _ in 0..2 {
            pending.push(ResolvedType::TypeParameter {
                owner: DeclarationId::compiler(crate::hir::CompilerDeclaration::Option),
                index: 0,
            });
        }
        while let Some(ty) = pending.pop() {
            assert!(matches!(ty, ResolvedType::TypeParameter { .. }));
        }
        drop(pending);
    });
    assert!(!drop_frontier.invalid);
    assert_eq!(drop_frontier.allocations, 1);
    assert_eq!(drop_frontier.reallocations, 0);
    assert_eq!(drop_frontier.live, 0);
    assert_eq!(drop_frontier.allocated, drop_frontier.freed);
    // The current replay owns prepaid nominal disposal backing. The complete
    // former frontier above remains independent; current Drop requests no new
    // allocation and every final release stays within the original reservation.
    let ((), observed) = crate::test_allocation_observer::measure(|| {
        let (result, index) = run(&DeclarationIndex::default(), 1 << 20, 1 << 20);
        assert_eq!(result.unwrap().1, charged);
        let retained = crate::test_allocation_observer::current_metrics();
        drop(index);
        let settled = crate::test_allocation_observer::current_metrics();
        assert_eq!(settled.allocations, retained.allocations);
        assert_eq!(settled.allocated, retained.allocated);
        assert_eq!(settled.live, 0);
    });
    assert!(!observed.invalid);
    assert!(
        observed.allocated <= charged,
        "{} > {charged}",
        observed.allocated
    );
    assert_eq!(observed.live, 0);
}
#[test]
fn vec_replay_builds_one_compiler_identity_within_its_reservation_through_drop() {
    let replay = |physical: &mut ReplayPhysicalReservation| {
        let mut index = DeclarationIndex::default();
        index
            .append_vec_replay_metered(physical, &mut |_| Ok(()))
            .unwrap();
        index
    };
    let index = replay(&mut ReplayPhysicalReservation::new(1 << 20));
    let vec = crate::prelude::VEC_ID;
    let (key, declaration) = index.declarations.get_key_value(vec).unwrap();
    compiler(key, vec);
    compiler(&declaration.id, vec);
    compiler(&index.types_by_name["Vec"], vec);
    compiler(index.type_parameters.get_key_value(vec).unwrap().0, vec);
    compiler(index.record_fields.get_key_value(vec).unwrap().0, vec);
    drop(index);
    let mut physical = ReplayPhysicalReservation::new(1 << 20);
    let ((), observed) = crate::test_allocation_observer::measure(|| {
        let index = replay(&mut physical);
        let retained = crate::test_allocation_observer::current_metrics();
        drop(index);
        let settled = crate::test_allocation_observer::current_metrics();
        assert_eq!(settled.allocations, retained.allocations);
        assert_eq!(settled.live, 0);
    });
    assert!(!observed.invalid);
    assert!(observed.allocated <= physical.charged());
    assert_eq!(observed.live, 0);
}
#[test]
fn option_u8_replay_refuses_generic_compiler_owned_orphan_and_unfielded_entries() {
    let program = program();
    let row_id = DeclarationId::new("app.Row");
    for attack in 0..10 {
        let mut index = replayed(&program);
        match attack {
            0 => index.type_parameters.get_mut(&row_id).unwrap().push(
                ResolvedTypeParameterDeclaration {
                    name: "T".into(),
                    index: 0,
                    span: crate::ast::Span::default(),
                },
            ),
            1 => {
                index.declarations.get_mut(&row_id).unwrap().identity_origin =
                    IdentityOrigin::CompilerOwned
            }
            2 => {
                index.record_fields.remove(&row_id).unwrap();
            }
            3 => {
                index
                    .type_parameters
                    .insert(DeclarationId::new("foreign.Row"), Vec::new());
            }
            4 => {
                index
                    .declarations
                    .get_mut(&DeclarationId::new("app.Row.n"))
                    .unwrap()
                    .owner = None
            }
            5 => {
                // Compiler-owned generic Vec metadata is not a record entry.
                let mut physical = ReplayPhysicalReservation::new(1 << 20);
                index
                    .append_vec_replay_metered(&mut physical, &mut |_| Ok(()))
                    .unwrap();
            }
            6 => index.declarations.get_mut(&row_id).unwrap().kind = DeclarationKind::Variant,
            7 => *index.record_fields.get_mut(&row_id).unwrap() = Vec::new().into(),
            8 => {
                index
                    .declarations
                    .get_mut(&DeclarationId::new("app.Row.flag"))
                    .unwrap()
                    .identity_origin = IdentityOrigin::CompilerOwned
            }
            _ => {
                index.variant_cases.insert(row_id.clone(), Vec::new());
            }
        }
        let (result, after) = run(&index, 1 << 20, 1 << 20);
        assert_eq!(
            result.unwrap_err().detail(),
            "selected Option metadata reconstruction refused",
            "attack {attack}"
        );
        // Admission is decided before any Option metadata is inserted.
        assert_eq!(after, index, "attack {attack}");
        assert!(!crate::hir::borrowed_scalar::option_u8::authenticated(
            &after,
            &option_u8()
        ));
    }
}
