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
        complete_inventory_predecessor(include_str!("../inventory_builder.rs")),
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

// Keep the original whole-source assertion and independently compiled old
// receiver intact. Only these exact later import/module edits are reversed.
fn complete_inventory_predecessor(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = |source: &str| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes())));
    assert_eq!(digest(current), "74761ad25d83020ed34e245643d95a62fe29bdaa6f144d71720f2b777ba6881f");
    let mut restored = current.to_owned();
    {
        let new = "use crate::cleanup_plan::{ReplayClonePayload, ReplayPhysicalReservation, replay_reserve_vec};";
        let old = "use crate::cleanup_plan::{replay_reserve_vec, ReplayClonePayload, ReplayPhysicalReservation};";
        assert_eq!(restored.matches(new).count(), 1, "unique complete inventory inverse");
        restored = restored.replacen(new, old, 1);
    }
    {
        let new = "mod selected;\npub(crate) mod selected_inline;";
        let old = "mod selected;";
        assert_eq!(restored.matches(new).count(), 1, "unique complete inventory inverse");
        restored = restored.replacen(new, old, 1);
    }
    {
        let new = "\n\npub(crate) mod typed;\n";
        let old = "\n";
        assert_eq!(restored.matches(new).count(), 1, "unique complete inventory inverse");
        restored = restored.replacen(new, old, 1);
    }
    assert_eq!(digest(&restored), "a00caa48b822a64862c176eb74acf2be1387b59b5af2298577167099c75ec356");
    restored
}

#[test]
fn inventory_source_inverse_retains_whole_modules_and_rejects_drift() {
    let current = include_str!("../inventory_builder.rs");
    let original = include_str!("../../../docs/evidence/schema-summary/inventory_builder.rs.before");
    assert_eq!(complete_inventory_predecessor(current), original);
    for hostile in [format!("{current} "), current.replacen("pub(crate)", "pub(super)", 1), original.to_owned()] {
        assert!(std::panic::catch_unwind(|| complete_inventory_predecessor(&hostile)).is_err());
    }
}
