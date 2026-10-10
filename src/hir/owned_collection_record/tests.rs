use super::*;
use crate::hir::{self, ResolvedProgram};

const ROWS: &str = include_str!("../../../tests/fixtures/nested-collection-records.spx");
const STRINGS: &str = include_str!("../../../tests/fixtures/nested-collection-string-record.spx");
fn checked(source: &str) -> ResolvedProgram {
    hir::resolve(&crate::check(source, "nested-collection.spx").unwrap()).unwrap()
}
fn report() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("collection.report"),
        arguments: vec![],
    }
}

#[test]
fn nested_collection_roundtrip_graph_cache_and_profiles_are_exact() {
    for source in [ROWS, STRINGS] {
        let ast = crate::check(source, "nested-collection.spx").unwrap();
        let canonical = crate::format::canonical(&ast);
        let round = crate::check(&canonical, "nested-collection.spx").unwrap();
        assert_eq!(crate::format::canonical(&round), canonical);
        let graph = crate::graph::to_json(&ast).unwrap();
        assert!(graph.contains("semaprax.graph.v72"));
        assert!(graph.contains("semaprax.nested-collection-records.v1"));
        crate::graph::verify_json(&ast, &graph).unwrap();
        assert!(crate::graph::verify_json(&ast, &graph.replace("graph.v72", "graph.v71")).is_err());
        let changed = crate::check(
            &source.replace("{42}else{0}", "{41}else{0}"),
            "nested-collection.spx",
        )
        .unwrap();
        assert!(crate::graph::verify_json(&changed, &graph).is_err());
        let program = checked(source);
        assert!(admitted(&report(), &program.declarations));
        hir::validate(&program).unwrap();
        hir::validate_stream_collection_record_program(&program, None).unwrap();
        assert!(hir::validate_stream_owned_program(&program, None).is_err());
        assert!(hir::validate_stream_record_program(&program, None).is_err());
        let wire = crate::cache_codec::encode(&program).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        hir::validate(&restored).unwrap();
        assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
        let result =
            crate::interpreter::evaluate_resolved_zero_arg_i64(&restored, "app.main", 1_000_000)
                .unwrap();
        assert!(
            matches!(
                result.outcome,
                crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
            ),
            "{:?}",
            result.outcome
        );
    }
}

#[test]
fn nested_collection_layout_charges_the_full_target_carrier() {
    use crate::aggregate_layout::{AggregateFieldValueKind, AggregateLayout, AggregateTarget};
    let program = checked(ROWS);
    for (target, bytes) in [
        (AggregateTarget::Native64, 40),
        (AggregateTarget::Wasm32, 8),
    ] {
        let layout = AggregateLayout::for_type(&program, target, &report()).unwrap();
        layout.validate(&program).unwrap();
        let vector = layout
            .field(&DeclarationId::new("collection.report.items"))
            .unwrap();
        assert_eq!(vector.value_kind, AggregateFieldValueKind::OwnedVec);
        assert_eq!((vector.size, vector.align), (bytes, 8));
        let mut forged = layout.clone();
        forged.fields[0].size = 16;
        assert!(forged.validate(&program).is_err());
    }
}

#[test]
fn nested_collection_declaration_origins_order_and_types_replay_after_cache_decode() {
    for id in [
        "collection.report",
        "collection.report.items",
        "collection.metrics",
        "collection.metrics.base",
    ] {
        for origin in [IdentityOrigin::Automatic, IdentityOrigin::CompilerOwned] {
            let mut forged = checked(ROWS);
            forged
                .declarations
                .declarations
                .get_mut(&DeclarationId::new(id))
                .unwrap()
                .identity_origin = origin;
            assert!(!admitted(&report(), &forged.declarations));
            let wire = crate::cache_codec::encode(&forged).unwrap();
            let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
            assert!(hir::validate(&restored).is_err());
            assert!(crate::codegen::emit_hir_c(&restored).is_err());
            assert!(crate::wasm::emit_resolved_module(&restored).is_err());
        }
    }
    for mode in 0..3 {
        let mut forged = checked(ROWS);
        let fields = forged
            .declarations
            .record_fields
            .get_mut(&DeclarationId::new("collection.report"))
            .unwrap();
        match mode {
            0 => fields.swap(0, 1),
            1 => fields[0].ty = ResolvedType::Bytes,
            _ => fields[0].id = DeclarationId::new("collection.metrics.base"),
        }
        assert!(hir::validate(&forged).is_err());
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert!(hir::validate(&restored).is_err());
    }
}

#[test]
fn projected_collection_loans_reject_stale_field_paths() {
    let source = ROWS.replace(
        "let first=inspect(report);",
        "let count=vec_len<Row>(report.items);let first=inspect(report);",
    );
    let program = checked(&source);
    let inspect = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let loan = inspect
        .loan_plan
        .loans
        .iter()
        .find(|loan| !loan.origin.projections.is_empty())
        .expect("projected Vec borrow retains its field path");
    assert!(loan.origin.projections.iter().any(|projection| matches!(projection, hir::PlaceProjection::Field(id) if id.as_str() == "collection.report.items")));
    let mut forged = program.clone();
    let inspect = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let loan = inspect
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| !loan.origin.projections.is_empty())
        .unwrap();
    loan.origin.projections[0] =
        hir::PlaceProjection::Field(DeclarationId::new("collection.report.metrics"));
    assert!(crate::loan_plan::validate_program(&forged).is_err());
    assert!(hir::validate(&forged).is_err());
}

#[test]
fn constructor_free_helpers_select_runtime_and_old_profiles_refuse_unused_helpers() {
    let source = STRINGS
        .split("@id(\"app.main\")")
        .next()
        .unwrap()
        .to_owned()
        + "@id(\"app.main\") fn main()->i64 {42}";
    assert!(!source.contains("vec_with_capacity"));
    let program = checked(&source);
    let inspect = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "collection.inspect")
        .unwrap();
    assert!(function_requires_profile_by(inspect, |id| program
        .types
        .iter()
        .find(|ty| &ty.id == id)));
    hir::validate_stream_collection_record_program(&program, None).unwrap();
    assert!(hir::validate_stream_owned_program(&program, None).is_err());
    let native = crate::codegen::emit_hir_c(&program).unwrap();
    assert!(native.contains("spx_leaf_drop_storage"));
    let wasm = crate::wasm::emit_resolved_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();
}

#[test]
fn updates_temporary_borrows_and_nested_vector_elements_remain_refused() {
    let updated = STRINGS.replace(
        "let count=inspect(report);",
        "let report=report with {metrics:Metrics{selected:11}};let count=inspect(report);",
    );
    assert!(crate::check(&updated, "collection-update.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T268"));
    let temporary = STRINGS.replace("let count=inspect(report);", "let count=i64_from_usize(vec_len<string>(Report{items:vec_with_capacity<string>(0usize),metrics:Metrics{selected:0}}.items));");
    assert!(crate::check(&temporary, "collection-temporary.spx").is_err());
    for ty in ["Vec<Report>", "Vec<Vec<string>>"] {
        let source = format!("module refused; @id(\"report\") record Report {{ @id(\"report.items\") items:{ty}, }} @id(\"take\") fn take(value:own Report)->i64 {{0}} @id(\"app.main\") fn main()->i64 {{0}}");
        assert!(crate::check(&source, "nested-elements.spx").is_err());
    }
}

#[test]
fn collection_records_keep_the_existing_depth_leaf_and_field_limits() {
    fn source(depth: usize, owners: usize, scalars: usize) -> String {
        let mut source = String::from("module collection.bounds;\n");
        for level in 0..depth {
            source.push_str(&format!("@id(\"r{level}\") record R{level} {{"));
            if level + 1 < depth {
                source.push_str(&format!("@id(\"r{level}.next\") next:R{},", level + 1));
            } else {
                for field in 0..owners {
                    source.push_str(&format!("@id(\"r{level}.v{field}\") v{field}:Vec<i64>,"));
                }
                for field in 0..scalars {
                    source.push_str(&format!("@id(\"r{level}.n{field}\") n{field}:i64,"));
                }
            }
            source.push_str("}\n");
        }
        source.push_str("@id(\"inspect\") fn inspect(value:borrow R0)->i64 {0} @id(\"app.main\") fn main()->i64 {42}");
        source
    }
    for (depth, owners, scalars) in [(64, 1, 0), (1, 256, 0), (1, 1, 4095)] {
        let program = checked(&source(depth, owners, scalars));
        let root = ResolvedType::Nominal {
            declaration: DeclarationId::new("r0"),
            arguments: vec![],
        };
        assert!(admitted(&root, &program.declarations));
    }
    for (depth, owners, scalars) in [(65, 1, 0), (1, 257, 0), (1, 1, 4096)] {
        assert!(crate::check(&source(depth, owners, scalars), "excessive-collection.spx").is_err());
    }
}

#[test]
fn direct_native_stream_apis_keep_nested_scalar_vec_helpers_out_of_frozen_profiles() {
    fn checked(helper: &str, command_type: &str, result: &str) -> ResolvedProgram {
        let source = format!(
            r#"module nested.native.profile;
permit {{process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}}
@id("metrics") record Metrics {{@id("metrics.total") total:i64,}}
@id("report") record Report {{@id("report.items") items:Vec<i64>,@id("report.metrics") metrics:Metrics,}}
{helper}
@id("command") fn command()->{command_type} {{{result}}}
@id("app.main") fn main()->i64 {{0}}
"#
        );
        hir::resolve(&crate::check(&source, "nested-native-profile.spx").unwrap()).unwrap()
    }
    // Both unused signatures and body-only carriers require the successor.
    for helper in [
        "@id(\"unused\") fn unused(value:borrow Report)->i64 {i64_from_usize(vec_len<i64>(value.items))}",
        "@id(\"unused\") fn unused()->i64 {let value=Report{items:vec_with_capacity<i64>(0usize),metrics:Metrics{total:0}};i64_from_usize(vec_len<i64>(value.items))}",
    ] {
        let program = checked(helper, "i64", "0");
        crate::codegen::emit_hir_c_with_stdin_stream_collection_records(&program, "command")
            .expect("explicit v31 admits the independently authenticated closure");
        for error in [
            crate::codegen::emit_hir_c_with_stdin_stream_exit_status(&program, "command"),
            crate::codegen::emit_hir_c_with_stdin_stream_text(&program, "command"),
            crate::codegen::emit_hir_c_with_stdin_stream_data(&program, "command"),
        ] {
            assert!(error.unwrap_err().message.contains("nested collection records"));
        }
        assert!(crate::codegen::emit_hir_c_with_stdin_stream_records(&program, "command").is_err());
        assert!(crate::codegen::emit_hir_c_with_stdin_stream_owned_data(&program, "command").unwrap_err().message.contains("nested collection records"));
        let boolean = checked(helper, "bool", "true");
        assert!(crate::codegen::emit_hir_c_with_stdin_stream(&boolean, "command").unwrap_err().message.contains("nested collection records"));
    }
    let ordinary = checked("", "i64", "0");
    crate::codegen::emit_hir_c_with_stdin_stream_data(&ordinary, "command")
        .expect("logical declarations alone do not require the successor");
    let boolean = checked("", "bool", "true");
    crate::codegen::emit_hir_c_with_stdin_stream(&boolean, "command")
        .expect("the exact old Boolean root and permit inventory remain admitted");
}
