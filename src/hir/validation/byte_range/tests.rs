use super::*;
const SOURCE: &str = r#"module test.inline_owned_range;
@id("inspect") fn inspect(view:borrow Slice<u8>)->usize { byte_len(view) }
@id("app.main") fn main()->i64 {
 let bytes=bytes_zeroed(4usize);
 let view=byte_range(bytes_as_slice(bytes),1usize,3usize);
 let short=byte_range(view,0usize,1usize);
 i64_from_usize(inspect(short)+inspect(byte_range(bytes_as_slice(bytes),0usize,4usize)))
}
"#;
#[test]
fn inline_owned_bytes_ranges_retain_named_alias_call_and_graph_origins() {
    let checked = crate::check(SOURCE, "inline-owned-range.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let roundtrip = crate::check(&canonical, "inline-owned-range-roundtrip.spx").unwrap();
    assert_eq!(crate::format::canonical(&roundtrip), canonical);
    let graph = crate::graph::to_json(&checked).unwrap();
    crate::graph::verify_json(&checked, &graph).unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    assert!(main
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.cause == crate::loan_plan::LoanCause::SliceView));
    let ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("main block")
    };
    let named = statements
        .iter()
        .filter_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. }
                if binding.name == "view" || binding.name == "short" =>
            {
                Some((binding, value))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (index, (binding, value)) in named.iter().enumerate() {
        let provenance = program
            .declarations
            .byte_slice_provenance(&binding.id)
            .unwrap();
        assert_eq!(
            provenance.root_kind,
            crate::hir::ByteSliceRootKind::OwnedBytes
        );
        assert_eq!(provenance.projected_type, ResolvedType::Bytes);
        assert_eq!(
            provenance.root_length,
            crate::hir::ByteSliceExtent::ValueLength
        );
        assert_eq!(provenance.ranges.len(), index + 1);
        assert_eq!(provenance.producer.as_ref(), Some(&value.id));
        if index == 0 {
            assert_eq!(provenance.ranges[0].source, provenance.root);
        } else {
            assert_eq!(provenance.ranges[1].source, named[0].0.id);
        }
    }
    assert_eq!(named.len(), 2);
    crate::codegen::emit_hir_c(&program).unwrap();
    let wasm = crate::wasm::emit_resolved_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();
}
#[test]
fn inline_range_refuses_forged_view_operation_root_and_cached_slice_facts() {
    let checked = crate::check(SOURCE, "inline-owned-range-hostile.spx").unwrap();
    let original = crate::hir::resolve(&checked).unwrap();
    for mode in 0..6 {
        let mut forged = original.clone();
        let main = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("main block")
        };
        let ResolvedStatement::Let { value, binding, .. } = &mut statements[1] else {
            panic!("view local")
        };
        let alias_id = binding.id.clone();
        let ResolvedExprKind::ByteRange { source, .. } = &mut value.kind else {
            panic!("range")
        };
        if mode == 3 {
            source.ty = ResolvedType::Str;
        } else if mode == 4 {
            source.ownership = OwnershipMode::Value;
        } else if mode == 5 {
            source.kind = ResolvedExprKind::Int(0);
        } else {
            let ResolvedExprKind::BorrowPlace { operation, place } = &mut source.kind else {
                panic!("inline view")
            };
            match mode {
                0 => *operation = DeclarationId::new(crate::byte_ops::ARRAY_AS_SLICE_ID),
                1 => place.root = alias_id,
                2 => place.root = main.result_id.clone(),
                _ => unreachable!(),
            }
        }
        let error = crate::hir::validate(&forged).unwrap_err();
        assert_eq!(error.code, "SPX-H006", "mode {mode}: {error:?}");
    }
}
#[test]
fn inline_range_keeps_temporary_and_other_view_sources_closed() {
    for source in [
        "module bad.range;@id(\"app.main\") fn main()->i64 {let raw=[0u8];let view=byte_range(array_as_slice(raw),0usize,1usize);0}",
        "module bad.range;@id(\"app.main\") fn main()->i64 {let view=byte_range(bytes_as_slice(bytes_zeroed(1usize)),0usize,1usize);0}",
    ] {
        let errors = crate::check(source,"closed-inline-range.spx").unwrap_err();
        assert!(errors.iter().any(|error| error.code=="SPX-T266"),"{errors:?}");
    }
}

#[test]
fn named_inline_range_keeps_source_owner_frozen_until_scope_end() {
    let invalid_write = SOURCE.replace(
        "let short=byte_range(view,0usize,1usize);",
        "let moved=bytes_set(bytes,0usize,1u8);let short=byte_range(view,0usize,1usize);",
    );
    assert!(
        crate::check(&invalid_write, "inline-range-invalid-write.spx")
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-T271")
    );
    // A real, otherwise admitted ownership transfer must reach the loan check.
    let changed = SOURCE.replace(
        "let short=byte_range(view,0usize,1usize);",
        "let moved=consume(bytes);let short=byte_range(view,0usize,1usize);",
    ) + "\n@id(\"consume\") fn consume(value:own Bytes)->usize {byte_len(bytes_as_slice(value))}\n";
    assert_ne!(changed, SOURCE);
    let errors = crate::check(&changed, "inline-range-owner-move.spx").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.code == "SPX-T265" || error.code == "SPX-O101"),
        "{errors:?}"
    );
}
