use super::*;
const SOURCE: &str = include_str!("../../../tests/owned_data/byte_helper_renewal.spx");
#[test]
fn transitive_byte_helpers_reserve_exact_owners_inside_and_outside_loops() {
    let checked = crate::check(SOURCE, "byte-helper-renewal.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    for name in ["leaf", "relay"] {
        let source = checked.functions.iter().find(|f| f.name == name).unwrap();
        let resolved = program.functions.iter().find(|f| f.name == name).unwrap();
        assert!(source_target(&checked, source));
        assert!(resolved_target(&program, resolved));
    }
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (refusal, overflowed, _) =
        crate::bounded_output::with_limit_usage(0, || checked_bindings(&program, main));
    assert!(overflowed);
    assert_eq!(refusal.unwrap_err().code, "SPX-H006");
    let sites = bindings(&program, main);
    assert_eq!(
        sites.len(),
        2,
        "ordinary assignment and loop assignment are distinct sites"
    );
    let (at, binding) = bindings(&program, main).into_iter().next().unwrap();
    let first_backing = std::mem::size_of::<HelperBindings<'_>>()
        + std::mem::size_of::<(&crate::hir::ExpressionId, &ResolvedBinding)>();
    let ((exact, mut entry), overflowed, used) =
        crate::bounded_output::with_limit_usage(first_backing, || {
            let mut entry = HelperBindings::default();
            let result = entry.insert(at, binding);
            (result, entry)
        });
    assert!(exact.is_some());
    assert!(!overflowed);
    assert_eq!(used, first_backing);
    assert_eq!(entry.backing_capacity(), 1);
    let (duplicate, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || entry.insert(at, binding));
    assert!(duplicate.is_some());
    assert!(!overflowed);
    assert_eq!(used, 0);
    let ((short, entry), overflowed, used) =
        crate::bounded_output::with_limit_usage(first_backing - 1, || {
            let mut entry = HelperBindings::default();
            let result = entry.insert(at, binding);
            (result, entry)
        });
    assert!(short.is_none());
    assert!(overflowed);
    assert_eq!(used, 0);
    assert!(entry.is_empty());
    assert_eq!(
        entry.backing_capacity(),
        0,
        "refused insertion allocated no backing"
    );
    assert_eq!(
        main.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    );
    for (at, binding) in sites {
        assert_eq!(binding.name, "output");
        assert!(main.cleanup_plan.blocks.iter().flat_map(|block| &block.transitions).any(|transition| matches!(transition,
            crate::cleanup_plan::CleanupTransition::ReserveRenewal { at: site, binding: place }
                if site == at && place.storage == crate::cleanup_plan::StorageId::Value(binding.id.clone()) && place.projections.is_empty())));
    }
}
#[test]
fn transitive_contract_allocation_unknown_calls_and_cycles_have_no_helper_authority() {
    for source in [
        SOURCE.replace(
            "requires index<byte_len(bytes_as_slice(output))",
            "requires string_len(\"allocated\")==9",
        ),
        SOURCE.replace(
            "next=bytes_set(next,index,value);",
            "let ignored=bytes_zeroed(1usize); next=bytes_set(next,index,value);",
        ),
        SOURCE.replace(
            "next=bytes_set(next,index,value);",
            "let ignored=missing(); next=bytes_set(next,index,value);",
        ),
        SOURCE.replace(
            "next=bytes_set(next,index,value);",
            "next=leaf(next,index,value);",
        ),
    ] {
        let parsed = crate::parse(&source, "byte-helper-hostile.spx").unwrap();
        let relay = parsed.functions.iter().find(|f| f.name == "relay").unwrap();
        assert!(
            !source_target(&parsed, relay),
            "transitive refusal must inspect leaf and contracts"
        );
        assert!(crate::check(&source, "byte-helper-hostile.spx").is_err());
    }
    let checked = crate::check(SOURCE, "byte-helper-forged-contract.spx").unwrap();
    let mut program = crate::hir::resolve(&checked).unwrap();
    let leaf = program
        .functions
        .iter_mut()
        .find(|f| f.name == "leaf")
        .unwrap();
    leaf.requires[0].kind = ResolvedExprKind::String("hidden allocation".into());
    let relay = program
        .functions
        .iter()
        .find(|f| f.name == "relay")
        .unwrap();
    assert!(
        !resolved_target(&program, relay),
        "attached v19 cleanup grants no closure authority"
    );
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
}
#[test]
fn helper_renewal_replay_refuses_a_different_actual_owner() {
    let checked = crate::check(SOURCE, "byte-helper-owner-forgery.spx").unwrap();
    let mut program = crate::hir::resolve(&checked).unwrap();
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("main block")
    };
    let other = statements
        .iter()
        .find_map(|s| match s {
            crate::hir::ResolvedStatement::Let { binding, .. } if binding.name == "untouched" => {
                Some(binding.id.clone())
            }
            _ => None,
        })
        .unwrap();
    let assignment = statements
        .iter_mut()
        .find_map(|s| match s {
            crate::hir::ResolvedStatement::Assign { value, .. } => Some(value),
            _ => None,
        })
        .unwrap();
    let ResolvedExprKind::Call { args, .. } = &mut assignment.kind else {
        panic!("helper call")
    };
    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
        panic!("whole owner")
    };
    place.root = other;
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn legacy_allocating_helper_census_preserves_ordinary_depth_and_refuses_one_extra_frame() {
    let checked = crate::check(
        r#"module census.depth;
@id("census.old") fn old(input:own Bytes)->Bytes {
 let length=byte_len(bytes_as_slice(input)); bytes_zeroed(1usize)
}
@id("census.main") fn main()->i64 {0}
"#,
        "legacy-byte-census-depth.spx",
    )
    .unwrap();
    let mut program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "census.main")
        .unwrap();
    // Preserve the actual typed source body and its identities, as the existing
    // exact-depth HIR admission oracle does. This census creates no new sites.
    let template = main.body.clone();
    for _ in 0..512 {
        let mut outer = template.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut outer.kind else {
            panic!("ordinary source block");
        };
        **tail = std::mem::replace(&mut main.body, template.clone());
        main.body = outer;
    }
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "census.main")
        .unwrap();
    assert!(checked_bindings(&program, main).unwrap().is_empty());
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "census.main")
        .unwrap();
    let mut outer = template;
    let ResolvedExprKind::Block { tail, .. } = &mut outer.kind else {
        panic!("source block");
    };
    // Move the existing body under exactly one additional source-shaped frame.
    **tail = main.body.clone();
    main.body = outer;
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "census.main")
        .unwrap();
    assert_eq!(
        checked_bindings(&program, main).unwrap_err().code,
        "SPX-H006"
    );
}

mod string_slots;
