use super::*;
use crate::hir::{self, ResolvedProgram};
const SOURCE: &str = include_str!("../../../../tests/fixtures/owned-nested-outcome.spx");
fn checked(source: &str) -> ResolvedProgram {
    hir::resolve(&crate::check(source, "nested-outcome.spx").unwrap()).unwrap()
}
fn ty() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("n.Outcome"),
        arguments: vec![],
    }
}

#[test]
fn nested_outcome_roundtrip_cache_graph_and_successor_profile_replay() {
    let ast = crate::check(SOURCE, "nested-outcome.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(&crate::check(&canonical, "nested-outcome.spx").unwrap()),
        canonical
    );
    let graph = crate::graph::to_json(&ast).unwrap();
    assert!(graph.contains("semaprax.graph.v74"));
    assert!(graph.contains("semaprax.owned-nested-outcomes.v1"));
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(crate::graph::verify_json(&ast, &graph.replace("graph.v74", "graph.v73")).is_err());
    let drift = crate::check(&SOURCE.replace("seed:7", "seed:8"), "nested-outcome.spx").unwrap();
    assert!(crate::graph::verify_json(&drift, &graph).is_err());
    let program = checked(SOURCE);
    assert!(admitted(&program.declarations, &ty()));
    hir::validate_stream_nested_outcome_program(&program, None).unwrap();
    assert!(hir::validate_stream_collection_record_program(&program, None).is_err());
    assert!(hir::validate_stream_owned_program(&program, None).is_err());
    assert!(hir::validate_stream_record_program(&program, None).is_err());
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
    crate::codegen::emit_hir_c(&restored).unwrap();
    wasmparser::Validator::new()
        .validate_all(&crate::wasm::emit_resolved_module(&restored).unwrap())
        .unwrap();
}

#[test]
fn nested_outcome_case_field_record_origins_and_types_fail_closed_after_cache_decode() {
    for id in [
        "n.Outcome",
        "n.Outcome.ready",
        "n.Outcome.value",
        "n.Outcome.error",
        "n.Outcome.offset",
        "n.Payload",
        "n.Payload.items",
        "n.Config",
        "n.Config.label",
    ] {
        for origin in [IdentityOrigin::Automatic, IdentityOrigin::CompilerOwned] {
            let mut forged = checked(SOURCE);
            forged
                .declarations
                .declarations
                .get_mut(&DeclarationId::new(id))
                .unwrap()
                .identity_origin = origin;
            assert!(!admitted(&forged.declarations, &ty()), "{id}");
            let wire = crate::cache_codec::encode(&forged).unwrap();
            let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
            assert!(hir::validate(&restored).is_err());
            assert!(crate::codegen::emit_hir_c(&restored).is_err());
            assert!(crate::wasm::emit_resolved_module(&restored).is_err());
        }
    }
    for mode in 0..4 {
        let mut forged = checked(SOURCE);
        let cases = forged
            .declarations
            .variant_cases
            .get_mut(&DeclarationId::new("n.Outcome"))
            .unwrap();
        match mode {
            0 => cases.swap(0, 1),
            1 => cases[0].fields[0].id = DeclarationId::new("n.Outcome.offset"),
            2 => cases[1].fields[1].ty = ResolvedType::I64,
            _ => cases[0].fields[0].ty = ResolvedType::Bytes,
        }
        assert!(!admitted(&forged.declarations, &ty()));
        assert!(hir::validate(&forged).is_err());
    }
}

#[test]
fn nested_outcome_cleanup_uses_exact_case_and_full_record_path() {
    let program = checked(SOURCE);
    let payload = ResolvedType::Nominal {
        declaration: DeclarationId::new("n.Payload"),
        arguments: vec![],
    };
    assert!(record_field(
        &program.declarations,
        &ty(),
        &DeclarationId::new("n.Outcome.ready"),
        &DeclarationId::new("n.Outcome.value"),
        &payload
    ));
    for (case, field) in [
        ("n.Outcome.error", "n.Outcome.value"),
        ("n.Outcome.ready", "n.Payload.items"),
        ("foreign", "n.Outcome.value"),
    ] {
        assert!(!record_field(
            &program.declarations,
            &ty(),
            &DeclarationId::new(case),
            &DeclarationId::new(field),
            &payload
        ));
    }
    let forward = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "n.forward")
        .unwrap();
    let paths = forward
        .cleanup
        .flags
        .iter()
        .map(|flag| {
            flag.place
                .projections
                .iter()
                .map(DeclarationId::as_str)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for path in [
        vec!["n.Outcome.ready", "n.Outcome.value", "n.Payload.items"],
        vec![
            "n.Outcome.ready",
            "n.Outcome.value",
            "n.Payload.config",
            "n.Config.label",
        ],
        vec!["n.Outcome.ready", "n.Outcome.value", "n.Payload.bytes"],
    ] {
        assert!(paths.contains(&path), "{paths:?}");
    }
    let mut forged = program.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "n.forward")
        .unwrap();
    function.cleanup.flags[0].place.projections[0] = DeclarationId::new("n.Outcome.error");
    assert!(hir::validate(&forged).is_err());
}

#[test]
fn nested_outcome_layout_is_record_sized_not_a_vec_slot() {
    use crate::variant_layout::{VariantFieldValueKind, VariantLayout, VariantTarget};
    let program = checked(SOURCE);
    for target in [VariantTarget::Native64, VariantTarget::Wasm32] {
        let layout = VariantLayout::for_type(&program, target, &ty()).unwrap();
        layout.validate(&program).unwrap();
        let field = layout
            .case(&DeclarationId::new("n.Outcome.ready"))
            .unwrap()
            .field(&DeclarationId::new("n.Outcome.value"))
            .unwrap();
        assert_eq!(field.value_kind, VariantFieldValueKind::OwnedRecord);
        let record =
            crate::aggregate_layout::AggregateLayout::for_type(&program, target, &field.ty)
                .unwrap();
        assert_eq!((field.size, field.align), (record.size, record.align));
        assert!(
            field.size
                > if target == VariantTarget::Native64 {
                    40
                } else {
                    8
                }
        );
    }
}

#[test]
fn nested_outcome_error_only_and_unused_helpers_still_require_v32() {
    let prefix = SOURCE.split("@id(\"app.main\")").next().unwrap();
    let source=format!("{prefix}@id(\"app.main\") fn main()->i64 {{let error=Outcome::Error{{code:42,offset:0usize,field:0}};finish(error)}}");
    let program = checked(&source);
    hir::validate_stream_nested_outcome_program(&program, None).unwrap();
    assert!(program_requires_profile(&program));
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "n.forward")
        .unwrap();
    assert!(function_requires_profile_by(function, |id| program
        .types
        .iter()
        .find(|item| &item.id == id)));
    for validator in [
        hir::validate_stream_collection_record_program,
        hir::validate_stream_owned_program,
        hir::validate_stream_record_program,
    ] {
        assert!(validator(&program, None).is_err());
    }
    crate::codegen::emit_hir_c(&program).unwrap();
    wasmparser::Validator::new()
        .validate_all(&crate::wasm::emit_resolved_module(&program).unwrap())
        .unwrap();
}

#[test]
fn nested_outcome_preserves_variant_inclusive_depth_and_leaf_bounds() {
    fn bounded(depth: usize, leaves: usize) -> String {
        let mut source = String::from("module bounded.outcome;\n");
        for n in 0..depth {
            source.push_str(&format!("@id(\"r{n}\") record R{n} {{"));
            if n + 1 < depth {
                source.push_str(&format!("@id(\"r{n}.child\") child:R{},", n + 1));
            } else {
                for k in 0..leaves {
                    source.push_str(&format!("@id(\"r{n}.s{k}\") s{k}:string,"));
                }
            }
            source.push_str("}\n");
        }
        source.push_str("@id(\"o\") variant O {@id(\"o.ok\") Ready{@id(\"o.value\") value:R0,},@id(\"o.err\") Error{@id(\"o.code\") code:i64,@id(\"o.offset\") offset:usize,@id(\"o.field\") field:i64,},}\n@id(\"forward\") fn forward(value:own O)->O{value}\n@id(\"app.main\") fn main()->i64{let value=forward(O::Error{code:42,offset:0usize,field:0});match own value {O::Ready{value}=>0,O::Error{code,offset,field}=>code,}}\n");
        source
    }
    for source in [bounded(63, 1), bounded(1, 256)] {
        let program = checked(&source);
        hir::validate_stream_nested_outcome_program(&program, None).unwrap();
    }
    for source in [bounded(64, 1), bounded(1, 257)] {
        let errors = crate::check(&source, "bounded-outcome.spx").unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| matches!(d.code.as_str(), "SPX-T215" | "SPX-T309" | "SPX-T268")),
            "{errors:?}"
        );
    }
    let missing = SOURCE.replace("@id(\"n.Outcome.value\") ", "");
    assert!(crate::check(&missing, "missing.spx").is_err());
    let wrong = SOURCE
        .replace("offset:usize", "offset:i64")
        .replace("offset:3usize", "offset:3");
    assert!(crate::check(&wrong, "wrong.spx").is_err());
}
