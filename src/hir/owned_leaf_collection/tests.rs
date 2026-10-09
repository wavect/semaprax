use super::*;
use crate::hir::{self, DeclarationId, IdentityOrigin, ResolvedProgram};
const SOURCE: &str = include_str!("../../../tests/fixtures/owned-leaf-collections.spx");
fn checked(source: &str) -> ResolvedProgram {
    let source = crate::check(source, "owned-leaf.spx").unwrap();
    let resolved = hir::resolve(&source).unwrap();
    hir::validate(&resolved).unwrap();
    resolved
}
fn entry() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("owned.leaf.entry"),
        arguments: Vec::new(),
    }
}

#[test]
fn owned_leaf_source_hir_graph_cache_and_interpreter_agree() {
    let ast = crate::check(SOURCE, "owned-leaf.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let round = crate::check(&canonical, "owned-leaf.spx").unwrap();
    assert_eq!(crate::format::canonical(&round), canonical);
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(graph.contains("semaprax.prelude.v14"));
    assert!(
        crate::graph::verify_json(
            &ast,
            &graph.replace("semaprax.prelude.v14", "semaprax.prelude.v13")
        )
        .is_err()
    );
    let resolved = checked(SOURCE);
    let wire = crate::cache_codec::encode(&resolved).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
    hir::validate_stream_owned_program(&restored, None).unwrap();
    assert!(hir::validate_stream_record_program(&restored, None).is_err());
    let observed =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&restored, "owned.leaf.main", 1_000_000)
            .unwrap();
    assert!(
        matches!(
            observed.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(0)
        ),
        "{:?}",
        observed.outcome
    );
}

#[test]
fn owned_leaf_metadata_and_source_drift_cannot_forge_admission() {
    let program = checked(SOURCE);
    let shape = layout(&program.declarations, &entry()).unwrap();
    assert_eq!(
        shape.owned_fields[0],
        Some(OwnedField {
            index: 0,
            kind: OwnedLeafKind::String
        })
    );
    assert_eq!(
        shape.owned_fields[1],
        Some(OwnedField {
            index: 2,
            kind: OwnedLeafKind::Bytes
        })
    );
    assert_eq!(shape.capacity(), 4096);
    let mut forged = program.clone();
    forged
        .declarations
        .declarations
        .get_mut(&DeclarationId::new("owned.leaf.key"))
        .unwrap()
        .identity_origin = IdentityOrigin::Automatic;
    assert!(layout(&forged.declarations, &entry()).is_none());
    assert!(hir::validate(&forged).is_err());
    let mut forged = program.clone();
    forged
        .declarations
        .record_fields
        .get_mut(&DeclarationId::new("owned.leaf.entry"))
        .unwrap()[0]
        .ty = ResolvedType::Bytes;
    assert!(hir::validate(&forged).is_err());
    let ast = crate::check(SOURCE, "owned-leaf.spx").unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    let drift = crate::check(
        &SOURCE
            .replace("count: i64", "count: i32")
            .replace("count: 20", "count: 20i32")
            .replace("count: 10", "count: 10i32")
            .replace("count: 30", "count: 30i32"),
        "owned-leaf.spx",
    );
    // Even a type-changing edit that independently refuses cannot re-use the
    // old source-bound graph. The admitted value edit exercises the exact gate.
    assert!(drift.is_err());
    let changed =
        crate::check(&SOURCE.replace("count: 20", "count: 21"), "owned-leaf.spx").unwrap();
    assert!(crate::graph::verify_json(&changed, &graph).is_err());
}

#[test]
fn old_owned_vector_operations_and_primitive_bytes_stay_closed() {
    for (ty, operation) in [
        ("string", "vec_get<string>(v,0usize)"),
        ("string", "vec_sort<string>(v)"),
        ("Bytes", "vec_clone_at<Bytes>(v,0usize)"),
        ("i64", "vec_clone_at<i64>(v,0usize)"),
    ] {
        let source = format!(
            "module t; @id(\"t.main\") fn main()->i64{{let v=vec_with_capacity<{ty}>(0usize);let bad={operation};0}}"
        );
        let errors = crate::check(&source, "old-operations.spx").unwrap_err();
        assert!(errors.iter().any(|d| d.code == "SPX-T281"), "{errors:?}");
    }
}

#[test]
fn owned_leaf_replacement_rejects_reusing_the_consumed_element() {
    let source = "module t; @id(\"t.main\") fn main()->i64 {let v=vec_with_capacity<string>(1usize);let text=\"a\";let w=vec_push<string>(v,text);string_len(text)}";
    let errors = crate::check(source, "moved-owned-leaf.spx").unwrap_err();
    assert!(errors.iter().any(|d| d.code == "SPX-O101"), "{errors:?}");
}

#[test]
fn unused_new_operation_is_rejected_by_old_profile_before_reachability() {
    let source = "module t; @id(\"t.unused\") fn unused()->i64 {let v=vec_with_capacity<string>(0usize);0} @id(\"t.main\") fn main()->i64 {0}";
    let program = checked(source);
    let unused = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "t.unused")
        .unwrap();
    assert!(function_requires_profile_by(unused, |id| program
        .types
        .iter()
        .find(|ty| &ty.id == id)));
    assert!(hir::validate_stream_record_program(&program, None).is_err());
    hir::validate_stream_owned_program(&program, None).unwrap();
}

#[test]
fn owned_leaf_string_vectors_can_clone_and_replace_in_bounded_loops() {
    let source = "module t; @id(\"t.main\") fn main()->i64 {let mut v=vec_with_capacity<string>(2usize);v=vec_push<string>(v,\"a\");v=vec_push<string>(v,\"b\");let mut index=0usize;while index<vec_len<string>(v){let item=vec_clone_at<string>(v,index);v=vec_replace<string>(v,index,string_concat(item,\"x\"));index=index+1usize;0}let a=vec_clone_at<string>(v,0usize);let b=vec_clone_at<string>(v,1usize);if a==\"ax\"&&b==\"bx\"{0}else{1}}";
    let program = checked(source);
    let outcome =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&program, "t.main", 1_000_000).unwrap();
    assert!(matches!(
        outcome.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(0)
    ));
}

#[test]
fn every_owned_position_and_mixed_kind_uses_independent_descriptor_charges() {
    for fields in [
        vec!["string"],
        vec!["Bytes"],
        vec!["string", "i64", "Bytes"],
        vec![
            "bool", "string", "i32", "string", "u8", "usize", "f32", "f64",
        ],
    ] {
        let declarations = fields
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("@id(\"r.{index}\") f{index}:{ty},"))
            .collect::<String>();
        let source = format!(
            "module t;@id(\"r\") record R{{{declarations}}}@id(\"unused\") fn unused(v:own Vec<R>)->i64{{0}}@id(\"main\") fn main()->i64{{0}}"
        );
        let program = checked(&source);
        let ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("r"),
            arguments: Vec::new(),
        };
        let descriptor = layout(&program.declarations, &ty).unwrap();
        let owned = fields
            .iter()
            .filter(|ty| matches!(**ty, "string" | "Bytes"))
            .count();
        assert_eq!(descriptor.owned_count, owned);
        assert_eq!(descriptor.scalar_count, fields.len() - owned);
        for field in descriptor.owned_fields.iter().flatten() {
            assert_eq!(
                field.kind,
                if fields[field.index] == "string" {
                    OwnedLeafKind::String
                } else {
                    OwnedLeafKind::Bytes
                }
            );
        }
        assert!(
            descriptor.capacity() * owned as u64 * 16 <= crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
        );
        assert!(
            descriptor.capacity() * descriptor.scalar_count as u64 <= crate::vec_ops::MAX_CAPACITY
        );
    }
}

#[test]
fn new_operations_keep_three_owned_fields_and_automatic_fields_refused() {
    for fields in [
        "@id(\"r.a\") a:string,@id(\"r.b\") b:string,@id(\"r.c\") c:string,",
        "a:string,",
    ] {
        let source = format!(
            "module t;@id(\"r\") record R{{{fields}}}@id(\"main\") fn main()->i64{{let v=vec_with_capacity<R>(0usize);let w=vec_sort_owned<R>(v);0}}"
        );
        let errors = crate::check(&source, "closed-shape.spx").unwrap_err();
        assert!(errors.iter().any(|d| d.code == "SPX-T281"), "{errors:?}");
    }
}

#[test]
fn owned_leaf_loop_helpers_and_field_views_replay_ordinary_loans() {
    let source = r#"module row.loop;
@id("r") record R { @id("r.text") text:string,@id("r.count") count:i64, }
@id("observe") fn observe(value:borrow R)->i64 { value.count }
@id("entry") fn main()->i64 {
 let mut rows=vec_with_capacity<R>(2usize);
 let mut i=0usize;
 while i<2usize { rows=vec_push<R>(rows,R{text:"key",count:4});i=i+1usize;0 }
 let mut total=0;
 i=0usize;
 while i<vec_len<R>(rows) {
   let row=vec_clone_at<R>(rows,i);
   let n={let text=string_as_str(row.text);str_len_bytes(text)};
   total=total+observe(row)+n;
   i=i+1usize;
   0
 }
 if total==14 {0}else{1}
}"#;
    let program = checked(source);
    let observed =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&program, "entry", 1_000_000).unwrap();
    assert!(
        matches!(
            observed.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(0)
        ),
        "{:?}",
        observed.outcome
    );
}
