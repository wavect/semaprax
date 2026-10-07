//! Hostile HIR must rederive catalogue signatures, ownership and lifecycle.
use super::*;
fn resolved()->crate::hir::ResolvedProgram {
    let source=r#"module test.map_replay;
@id("map.read") fn read(borrow map:Map<i64,string>)->i64 {string_len(map_get_or<i64,string>(map,3,"missing"))}
@id("map.main") fn main()->i64 {let mut map=map_new<i64,string>(2usize);map=map_set<i64,string>(map,3,"abc");read(map)}"#;
    let source=crate::check(source,"map-replay.spx").unwrap();crate::hir::resolve(&source).unwrap()
}
#[test]
fn checked_collection_declarations_cannot_be_relabelled_or_relaid_out() {
    let baseline=resolved();
    for mutate in 0..3 {
        let mut program=baseline.clone();let decl=program.types.iter_mut().find(|d|d.id.as_str()==MAP_ID).unwrap();
        match mutate {0=>decl.name="AuthoredMap".into(),1=>decl.type_parameters[0].index=9,_=>decl.type_parameters.pop().map(|_|()).unwrap()};
        assert!(crate::hir::validate(&program).is_err());
        assert!(crate::wasm::emit_resolved_module(&program).is_err());
    }
}
#[test]
fn changed_collection_type_arguments_cannot_reuse_attached_ownership_proof() {
    let mut program=resolved();let main=program.functions.iter_mut().find(|f|f.name=="main").unwrap();
    let crate::hir::ResolvedExprKind::Block{statements,..}=&mut main.body.kind else{panic!("block")};
    let crate::hir::ResolvedStatement::Assign{value,..}=&mut statements[1] else{panic!("assignment")};
    let crate::hir::ResolvedExprKind::Call{type_arguments,..}=&mut value.kind else{panic!("call")};
    type_arguments[1]=ResolvedType::Bytes;
    assert!(crate::hir::validate(&program).is_err());
}
#[test]
fn independent_generic_signatures_preserve_owner_and_copied_string_modes() {
    for op in MapOp::ALL {
        let types=if op.is_set(){vec![ResolvedType::String]}else if op==MapOp::Add{vec![ResolvedType::String,ResolvedType::I64]}else{vec![ResolvedType::I64,ResolvedType::String]};
        let (params,result)=op.resolved_signature(&types).unwrap();assert_eq!(params.len(),op.arity());
        if !matches!(op,MapOp::New|MapOp::SetNew){assert_eq!(params[0].ownership,if op.reopens(){OwnershipMode::Own}else{OwnershipMode::Borrow});}
        if op.returns_collection(){assert!(is_collection(&result));}
    }
    assert!(MapOp::Set.resolved_signature(&[ResolvedType::F64,ResolvedType::String]).is_none());
    assert!(MapOp::Add.resolved_signature(&[ResolvedType::I64,ResolvedType::F64]).is_none());
}

#[test]
fn canonical_cleanup_leaf_cannot_be_replayed_as_a_string_or_legacy_map() {
    let baseline=resolved();
    for forged in [crate::cleanup::STRING_DROP_LIFECYCLE_ID,crate::string_ops::MAP_DROP_LIFECYCLE_ID] {
        let mut program=baseline.clone();
        let main=program.functions.iter_mut().find(|f|f.name=="main").unwrap();
        let slot=main.cleanup_plan.slots.iter_mut().find(|s|is_typed_collection(&s.ty)).unwrap();
        let crate::cleanup::FieldLivenessShape::Leaf{lifecycle,..}=&mut slot.field_liveness_shape else{panic!("direct map leaf")};
        *lifecycle=DeclarationId::new(forged);
        assert!(crate::hir::validate(&program).is_err());
        assert!(crate::wasm::emit_resolved_module(&program).is_err());
    }
}

#[test]
fn authored_declarations_cannot_reuse_collection_operation_or_lifecycle_ids() {
    for id in reserved_ids() {
        let source=format!("module test.map_reserved; @id({id:?}) fn main()->i64 {{0}}");
        assert!(crate::check(&source,"map-reserved.spx").is_err(),"reserved identity {id}");
    }
    for id in [MapOp::Set.id(),DROP_ID,"core.collection.wasm.checked.v2"] {
        let mut program=resolved();program.functions.iter_mut().find(|f|f.name=="main").unwrap().id=DeclarationId::new(id);
        assert!(crate::hir::validate(&program).is_err());
        assert!(crate::wasm::emit_resolved_module(&program).is_err());
    }
}

#[test]
fn additive_transport_and_removal_select_the_collection_prelude() {
    for body in [
        "@id(\"map.borrow\") fn read(borrow map:Map<string,i64>)->usize {map_len(map)} @id(\"map.main\") fn main()->i64 {0}",
        "@id(\"map.carrier\") record Carrier {@id(\"map.field\") words:Map<string,i64>,} @id(\"map.main\") fn main()->i64 {0}",
        "@id(\"map.main\") fn main()->i64 {let mut map=map_new(1usize);map=map_remove(map,\"missing\");0}",
    ] {
        let source=format!("module test.map_selection; {body}");let program=crate::check(&source,"map-selection.spx").unwrap();assert!(program_uses(&program));
    }
    let old=crate::check("module test.old_map; @id(\"map.main\") fn main()->i64 {let map=map_new(1usize);0}","old-map.spx").unwrap();assert!(!program_uses(&old));
}
