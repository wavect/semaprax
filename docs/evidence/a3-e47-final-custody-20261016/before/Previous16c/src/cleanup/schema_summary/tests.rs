use super::*;

fn program() -> ResolvedProgram {
    let source = crate::check(
        r#"module summary.inventory;
@id("summary.read") fn read(input:borrow Bytes)->usize {
 let view=bytes_as_slice(input); byte_len(view)
}
@id("summary.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); let mut at=0;
 while at<2 {data=bytes_set(data,0usize,65u8);at=at+1;if at==1 {continue;};0}
 if read(data)==1usize {42}else{0}
}"#,
        "summary-inventory.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn full(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<Summary, Diagnostic> {
    let inventory = build_inventory(program, function)?;
    let nested = inventory.slots.iter().try_fold(false, |nested, slot| {
        cleanup_shape_profile(&slot.shape).map(|p| nested || p.has_nested_owned_bytes)
    })?;
    Ok(Summary {
        has_nested_owned_bytes: nested,
        has_variant: inventory.schema == CLEANUP_INVENTORY_SCHEMA_V2,
        has_iterator: inventory
            .flags
            .iter()
            .any(|f| f.lifecycle.as_str() == ITER_DROP_LIFECYCLE_ID),
        storage_count: inventory.slots.len(),
        flag_count: inventory.flags.len(),
    })
}
#[test]
fn complete_ordinary_summary_matches_full_receiver_and_receiving_drop() {
    let program = program();
    for function in &program.functions {
        assert_eq!(collect(&program, function), full(&program, function));
        let (_, measured) = crate::test_allocation_observer::measure(|| {
            let result = collect(&program, function);
            assert!(result.is_ok());
            drop(result);
        });
        assert!(!measured.invalid);
        assert_eq!(measured.live, 0);
        assert_eq!(measured.allocated, measured.freed);
    }
}
#[test]
fn late_missing_type_preserves_full_first_diagnostic_after_earlier_owners() {
    let program = program();
    let mut function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "summary.main")
        .unwrap()
        .clone();
    function.return_type = ResolvedType::Nominal {
        declaration: DeclarationId::new("forged.missing.return"),
        arguments: Vec::new(),
    };
    assert_eq!(collect(&program, &function), full(&program, &function));
    assert!(collect(&program, &function).is_err());
}
#[test]
fn discovery_indices_survive_discarded_directories_and_empty_flags() {
    // A real checked result identity; no private unscoped identity constructor.
    let checked = program();
    let function = checked
        .functions
        .iter()
        .find(|f| f.id.as_str() == "summary.main")
        .unwrap();
    let result = ValueId::result(&crate::hir::FunctionExecutionId::Monomorphic(
        function.id.clone(),
    ));
    let mut slots = Slots::default();
    for n in 0..17 {
        slots.push(CleanupStorageSlot {
            id: CleanupStorageId(n),
            discovery_index: n,
            origin: CleanupStorageOrigin::ProvisionalResult {
                value: result.clone(),
            },
            ty: ResolvedType::Unit,
            shape: FieldLivenessShape::NoDrop,
        });
        assert_eq!(slots.len(), n as usize + 1);
        assert_eq!(slots.iter().count(), 1);
        assert_eq!(slots.iter().next().unwrap().id, CleanupStorageId(n));
    }
    let mut flags = Flags::default();
    for n in 0..17 {
        flags.push(CleanupFlag {
            id: LivenessFlagId(n),
            place: CleanupPlace {
                storage: CleanupStorageId(n),
                projections: Vec::new(),
            },
            lifecycle: DeclarationId::new(ITER_DROP_LIFECYCLE_ID),
        });
    }
    assert_eq!(flags.len(), 17);
    assert!(flags.iterator);
}
#[test]
fn independently_compiled_full_producer_sources_remain_byte_exact() {
    assert_eq!(
        include_str!("../inventory_builder.rs"),
        include_str!("../../../docs/evidence/schema-summary/inventory_builder.rs.before")
    );
    assert_eq!(
        include_str!("../generic_record.rs"),
        include_str!("../../../docs/evidence/schema-summary/generic_record.rs.before")
    );
    assert_eq!(
        include_str!("generic_record.rs"),
        include_str!("../generic_record.rs")
    );
}
