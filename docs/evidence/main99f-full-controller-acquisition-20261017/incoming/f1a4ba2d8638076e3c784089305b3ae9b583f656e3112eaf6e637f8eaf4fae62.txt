//! Primitive slot views compose with the ordinary nonallocating closure proof.
use super::*;

const SOURCE: &str = r#"module helper.slots;
@id("helper.read") fn read(words:borrow Vec<string>,index:usize,output:own Bytes)->Bytes
requires index<vec_len<string>(words) && byte_len(bytes_as_slice(output))>=1usize {
let selected=vec_str_at(words,index);let bytes=str_as_bytes(selected);
let byte=match byte_get(bytes,0usize){Option::Some{value:item}=>item,Option::None{}=>0u8,};
let mut next=output;next=bytes_set(next,0usize,byte);next
}
@id("helper.fused") fn fused(words:borrow Vec<string>,index:usize,output:own Bytes)->Bytes
requires index<vec_len<string>(words) && byte_len(bytes_as_slice(output))>=1usize {
let bytes=str_as_bytes(vec_str_at(words,index));
let byte=match byte_get(bytes,0usize){Option::Some{value:item}=>item,Option::None{}=>0u8,};
let mut next=output;next=bytes_set(next,0usize,byte);next
}
@id("app.main") fn main()->i64 {
let initial=vec_with_capacity<string>(2usize);let one=vec_push<string>(initial,"A");let words=vec_push<string>(one,"B");
let mut output=bytes_zeroed(1usize);let mut index=0usize;
while index<2usize{output=read(words,index,output);output=fused(words,index,output);index=index+1usize;0}
match byte_get(bytes_as_slice(output),0usize){Option::Some{value:byte}=>i64_from_u8(byte),Option::None{}=>0,}
}
"#;

#[test]
fn named_and_fused_slot_reads_keep_both_complete_closure_proofs_and_graphs() {
    let checked = crate::check(SOURCE, "slot-byte-helper.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "slot-byte-helper-canonical.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&checked));
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    for name in ["read", "fused"] {
        let source = checked.functions.iter().find(|f| f.name == name).unwrap();
        let resolved = program.functions.iter().find(|f| f.name == name).unwrap();
        assert!(source_target(&checked, source));
        assert!(resolved_target(&program, resolved));
    }
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert_eq!(bindings(&program, main).len(), 2);
    assert_eq!(
        main.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    );
    let graph = crate::graph::to_json(&checked).unwrap();
    crate::graph::verify_json(&checked, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(
        document["byte_helper_renewal"]["schema"],
        "semaprax.byte-helper-renewal.v1"
    );
}

#[test]
fn slot_read_admission_keeps_allocating_clones_and_non_named_sources_closed() {
    for source in [
        SOURCE.replace("let selected=vec_str_at(words,index);let bytes=str_as_bytes(selected);", "let selected=vec_clone_at<string>(words,index);let bytes=str_as_bytes(string_as_str(selected));"),
        SOURCE.replace("vec_str_at(words,index)", "vec_str_at<string>(words,index)"),
        SOURCE.replace("vec_str_at(words,index)", "vec_str_at(words)"),
        SOURCE.replace("vec_str_at(words,index)", "vec_str_at(vec_with_capacity<string>(1usize),index)"),
    ] {
        let parsed = crate::parse(&source, "slot-byte-helper-hostile.spx").unwrap();
        let helper = parsed.functions.iter().find(|f| f.name == "read").unwrap();
        assert!(!source_target(&parsed, helper));
        let errors = crate::check(&source, "slot-byte-helper-hostile.spx").unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-U105"));
    }
}

#[test]
fn complete_fixed_slot_shape_is_rechecked_before_helper_renewal_authority() {
    let checked = crate::check(SOURCE, "slot-byte-helper-forgery.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    for name in ["read", "fused"] {
        for mutation in 0..9 {
            let mut forged = program.clone();
            let helper = forged
                .functions
                .iter_mut()
                .find(|f| f.name == name)
                .unwrap();
            let ResolvedExprKind::Block { statements, .. } = &mut helper.body.kind else {
                panic!("helper block")
            };
            let crate::hir::ResolvedStatement::Let { value: slot, .. } = &mut statements[0] else {
                panic!("slot binding")
            };
            let original_kind = slot.kind.clone();
            if mutation == 0 {
                slot.ty = ResolvedType::Bytes;
            }
            if mutation == 1 {
                slot.ownership = OwnershipMode::Own;
            }
            let ResolvedExprKind::Call {
                type_arguments,
                args,
                ..
            } = &mut slot.kind
            else {
                panic!("fixed slot call")
            };
            match mutation {
                2 => type_arguments.clear(),
                3 => type_arguments[0] = ResolvedType::U8,
                4 => {
                    args.pop().unwrap();
                }
                5 => args[0].ownership = OwnershipMode::Value,
                6 => args[0].ty = ResolvedType::Bytes,
                7 => args[0].kind = original_kind,
                8 => args[1].ownership = OwnershipMode::Borrow,
                _ => {}
            }
            let helper = forged.functions.iter().find(|f| f.name == name).unwrap();
            assert!(
                !resolved_target(&forged, helper),
                "{name}, mutation {mutation}"
            );
            assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
        }
    }
}
