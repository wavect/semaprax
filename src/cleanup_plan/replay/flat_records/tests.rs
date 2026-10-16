use super::*;
const SOURCE: &str = r#"module replay.flat_records;
@id("flat.Packet") record Packet {
 @id("flat.Packet.n") n:i64,@id("flat.Packet.text") text:string,@id("flat.Packet.data") data:Bytes,
}
@id("flat.identity") fn identity(value:own Packet,flag:bool)->Packet {
 let mut next=0;while next<1 {next=next+1;if flag {return value;} else {0} 0} value
}
@id("flat.make") fn make(input:borrow Slice<u8>)->Packet {
 let key=str"x";let value=Packet{n:7,text:"label",data:bytes_copy(input)};
 let mut next=0;while next<1 {next=next+1;if byte_len(input)>0usize {return value;} else {0} 0} value
}
@id("flat.read") fn read(value:borrow Packet)->i64 {value.n}
@id("flat.probe") fn probe(flag:bool)->i64 {
 let input=[1u8,2u8];let packet=identity(make(array_as_slice(input)),flag);
 let mut next=0;while next<2 {next=next+1;if flag {continue;} else {0} if next==2 {return read(packet);} else {0} 0} read(packet)
}
@id("app.main") fn main()->i64 {probe(false)}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "selected-flat-records.spx").unwrap()).unwrap()
}
fn run(
    fixture: &crate::workspace_graph::SourceRecordReplayFixture,
    id: &str,
    bytes: usize,
) -> Result<PhysicalReservation, Diagnostic> {
    // Authenticate held source independently before this replay-only reservation.
    // Context construction has its own charged ledger; it is not included in this phase.
    fixture.with_context(|context| {
        let function = context
            .functions()
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        super::super::selected_entry::validate_retained_lexical_with_reservation(
            context,
            function,
            PhysicalReservation::new(bytes),
        )
    })
}
#[test]
fn actual_nominal_make_identity_read_replay_matches_legacy_exact_and_one_short() {
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(SOURCE);
    let standalone = program();
    assert!(standalone
        .types
        .iter()
        .any(|ty| ty.id.as_str() == "flat.Packet"
            && matches!(&ty.kind, crate::hir::ResolvedTypeDeclarationKind::Record { fields }
                if fields.len() == 3)));
    let standalone_function = standalone
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.identity")
        .unwrap();
    assert_eq!(
        super::super::selected_entry::validate_retained_lexical_with_reservation(
            &standalone,
            standalone_function,
            PhysicalReservation::new(usize::MAX),
        )
        .err()
        .unwrap()
        .code,
        "SPX-H006"
    );
    for id in ["flat.make", "flat.identity", "flat.probe"] {
        let function = fixture
            .functions()
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        assert!(crate::hir::lexical_exit::requires(function).unwrap());
        // The legacy reference requires the independently resolved program's
        // genuine nominal TypeFacts. The retained primitive context intentionally
        // contains direct field facts rather than generic nominal cleanup facts.
        let legacy_function = standalone
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        let mut budget = ReplayBudget::new();
        super::super::reserve_program_skeleton_work(
            &standalone,
            std::iter::once(legacy_function),
            &mut budget,
        )
        .unwrap();
        super::super::lexical_replay::validate_with_index(
            &standalone,
            legacy_function,
            &mut budget,
            None,
        )
        .unwrap();
        let measured = run(&fixture, id, usize::MAX).unwrap();
        assert!(measured.charged() > 0);
        let exact = run(&fixture, id, measured.charged()).unwrap();
        assert_eq!(exact.charged(), measured.charged());
        assert_eq!(
            run(&fixture, id, measured.charged() - 1)
                .err()
                .unwrap()
                .code,
            "SPX-H006"
        );
    }
    let identity = fixture
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "flat.identity")
        .unwrap();
    assert!(matches!(identity.return_type, ResolvedType::Nominal { .. }));
    assert!(identity
        .cleanup_plan
        .slots
        .iter()
        .any(|slot| slot.storage == StorageId::ProvisionalResult
            && matches!(slot.field_liveness_shape, FieldLivenessShape::Record { .. })));
    let probe = fixture
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "flat.probe")
        .unwrap();
    assert!(probe.cleanup_plan.slots.iter().any(|slot| matches!(
        slot.storage,
        StorageId::CallArgument { .. }
    ) && matches!(
        slot.ty,
        ResolvedType::Nominal { .. }
    ) && matches!(
        slot.field_liveness_shape,
        FieldLivenessShape::Record { .. }
    )));
}
#[test]
fn flags_projection_epoch_and_runtime_finalizer_order_forgery_refuse() {
    for attack in 0..4 {
        let mut fixture = crate::workspace_graph::SourceRecordReplayFixture::new(SOURCE);
        let function = fixture.function_mut("flat.probe");
        match attack {
            0 => {
                let slot = function
                    .cleanup_plan
                    .slots
                    .iter_mut()
                    .find(|slot| {
                        matches!(slot.field_liveness_shape, FieldLivenessShape::Record { .. })
                    })
                    .unwrap();
                let FieldLivenessShape::Record { fields, .. } = &mut slot.field_liveness_shape
                else {
                    unreachable!()
                };
                let field = fields
                    .iter_mut()
                    .find(|field| matches!(field.shape, FieldLivenessShape::Leaf { .. }))
                    .unwrap();
                let FieldLivenessShape::Leaf { flag, .. } = &mut field.shape else {
                    unreachable!()
                };
                flag.0 += 1;
            }
            1 => {
                let action = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .flat_map(|exit| &mut exit.finalize_in_order)
                    .find(|action| !action.source.projections.is_empty())
                    .unwrap();
                action.source.projections[0] = DeclarationId::new("forged.field");
            }
            2 => {
                let slot = function
                    .cleanup_plan
                    .slots
                    .iter_mut()
                    .find(|slot| {
                        matches!(slot.storage, StorageId::CallArgument { .. })
                            && matches!(slot.ty, ResolvedType::Nominal { .. })
                    })
                    .unwrap();
                let StorageId::CallArgument {
                    parameter_index, ..
                } = &mut slot.storage
                else {
                    unreachable!()
                };
                *parameter_index += 1;
            }
            _ => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| exit.finalize_in_order.len() >= 2)
                    .unwrap();
                exit.finalize_in_order.swap(0, 1);
            }
        }
        assert_eq!(
            run(&fixture, "flat.probe", usize::MAX).err().unwrap().code,
            "SPX-H006",
            "attack {attack}"
        );
    }
}
#[test]
fn current_direct_fields_reject_unknown_argument_and_recursive_layout_without_type_facts() {
    let program = program();
    let record = program
        .types
        .iter()
        .find(|record| record.id.as_str() == "flat.Packet")
        .unwrap();
    let ty = ResolvedType::Nominal {
        declaration: record.id.clone(),
        arguments: Vec::new(),
    };
    let (_, fields) = crate::cleanup::selected_flat_record_fields(&program, &ty)
        .unwrap()
        .unwrap();
    assert_eq!(fields.len(), 3);
    let invalid = ResolvedType::Nominal {
        declaration: record.id.clone(),
        arguments: vec![ResolvedType::I64],
    };
    assert!(crate::cleanup::selected_flat_record_fields(&program, &invalid).is_err());
    let mut changed = program.clone();
    let record = changed
        .types
        .iter_mut()
        .find(|record| record.id.as_str() == "flat.Packet")
        .unwrap();
    let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
        unreachable!()
    };
    fields[0].ty = ty.clone();
    assert!(crate::cleanup::selected_flat_record_fields(&changed, &ty).is_err());
}
#[test]
fn whole_record_argument_shape_preserves_contiguous_fields_and_partial_failure_leaves() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.probe")
        .unwrap();
    let ty = &program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.identity")
        .unwrap()
        .return_type;
    let mut next = 0;
    let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    let shape = super::shape(&program, function, ty, &mut next, &mut budget).unwrap();
    assert_eq!(next, 2);
    let FieldLivenessShape::Record { fields, .. } = shape else {
        panic!("whole argument shape")
    };
    assert!(matches!(fields[0].shape, FieldLivenessShape::NoDrop));
    let oracle = [
        crate::cleanup::STRING_DROP_LIFECYCLE_ID,
        crate::cleanup::BYTES_DROP_LIFECYCLE_ID,
    ];
    for ((field, expected), text) in fields[1..].iter().zip([0, 1]).zip(oracle) {
        let FieldLivenessShape::Leaf { flag, lifecycle } = &field.shape else {
            panic!("owned direct leaf")
        };
        assert_eq!(flag.0, expected);
        // A fixed compiler lifecycle that owns no bytes, equal to its text.
        assert!(lifecycle.is_compiler_static());
        assert_eq!(lifecycle.owned_allocation_bytes(), 0);
        assert_eq!(*lifecycle, DeclarationId::new(text.to_owned()));
    }
    let make = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.make")
        .unwrap();
    assert!(
        make.cleanup_plan.exits.iter().any(|exit| matches!(
            exit.continuation,
            ExitContinuation::ReturnFailure { .. }
        ) && !exit.finalize_in_order.is_empty()),
        "ordinary partial-constructor failure retains real cleanup actions"
    );
}

#[test]
fn whole_record_shape_backing_is_cumulative_across_work_reset_and_refusal_sticky() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.probe")
        .unwrap();
    let ty = &program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.identity")
        .unwrap()
        .return_type;
    let mut measured = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    let mut next = 0;
    let first = super::shape(&program, function, ty, &mut next, &mut measured).unwrap();
    let charged = measured.physical.as_ref().unwrap().charged();
    measured.remaining = MAX_REPLAY_WORK_UNITS;
    let second = super::shape(&program, function, ty, &mut next, &mut measured).unwrap();
    let total = measured.physical.as_ref().unwrap().charged();
    assert!(total > charged);
    assert_eq!(next, 4);
    drop((first, second));
    let mut short = ReplayBudget::with_physical(function, total - 1).unwrap();
    let mut next = 0;
    super::shape(&program, function, ty, &mut next, &mut short).unwrap();
    short.remaining = MAX_REPLAY_WORK_UNITS;
    assert_eq!(
        super::shape(&program, function, ty, &mut next, &mut short)
            .err()
            .unwrap()
            .code,
        "SPX-H006"
    );
    assert!(short.physical.as_mut().unwrap().reserve(0).is_err());
}

#[path = "tests/receiving_families.rs"]
mod receiving_families;

#[path = "tests/receiving_status_scalar.rs"]
mod receiving_status_scalar;

#[path = "tests/receiving_certificate.rs"]
mod receiving_certificate;

#[path="tests/receiving_boundary.rs"]
mod receiving_boundary;

#[path="tests/receiving_type_drop.rs"]
mod receiving_type_drop;

#[path="tests/receiving_path_boundary.rs"]
mod receiving_path_boundary;
