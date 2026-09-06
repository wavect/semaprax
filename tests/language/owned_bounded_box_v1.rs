use semaprax::hir::{self, OwnershipMode, ResolvedExprKind, ResolvedStatement, ResolvedType};
use semaprax::{format, graph, parse, verify};
use std::path::Path;
const SOURCE: &str = r#"module test.owned_box;
@id("app.main") fn main()->i64 { let value = box_new<i64>(7); let seen = box_get<i64>(value); let inner = box_into_inner<i64>(value); if seen == inner { inner } else { 0 } }
"#;
fn program(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("owned-box.spx")).unwrap()
}
fn errors(source: &str) -> Vec<&'static str> {
    verify::verify(&program(source))
        .into_iter()
        .filter(|d| d.severity.is_error())
        .map(|d| d.code)
        .collect()
}
#[test]
fn exact_source_hir_graph_and_legacy_selection() {
    let parsed = program(SOURCE);
    assert!(verify::verify(&parsed).is_empty());
    let canonical = format::canonical(&parsed);
    assert!(canonical.contains("box_into_inner<i64>(value)"));
    let resolved = hir::resolve(&program(&canonical)).unwrap();
    let ty = ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("core.box"),
        arguments: vec![ResolvedType::I64],
    };
    let facts = resolved.declarations.type_facts(&ty).unwrap();
    assert!(!facts.copy && facts.needs_drop && facts.sized && !facts.contains_resource);
    let main = resolved
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!()
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!()
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        ..
    } = &value.kind
    else {
        panic!()
    };
    assert_eq!(callee.as_str(), "core.box.new");
    assert_eq!(type_arguments, &[ResolvedType::I64]);
    assert!(instance.is_none());
    assert_eq!(value.ownership, OwnershipMode::Own);
    let json = graph::to_json(&parsed).unwrap();
    assert!(json.contains("semaprax.prelude.v4") && json.contains("core.box.new"));
    let legacy=program("module legacy.box; @id(\"legacy.box\") record Box<T>{@id(\"legacy.box.value\") value:T,}@id(\"app.main\") fn main()->i64{0}");
    assert!(verify::verify(&legacy).is_empty());
    assert!(graph::to_json(&legacy)
        .unwrap()
        .contains("semaprax.prelude.v1"));
}
#[test]
fn source_and_hir_hostiles_fail_closed() {
    assert!(errors(&SOURCE.replace("box_new<i64>", "box_new")).contains(&"SPX-T285"));
    assert!(errors(&SOURCE.replace("<i64>", "<Bytes>")).contains(&"SPX-T285"));
    assert!(errors(&SOURCE.replace("box_new<i64>(7)", "box_new<i64>(7, 8)")).contains(&"SPX-T285"));
    let collision = SOURCE.replace(
        "@id(\"app.main\")",
        "@id(\"bad.box\") record Box<T>{@id(\"bad.box.value\") value:T,}\n@id(\"app.main\")",
    );
    assert!(errors(&collision).contains(&"SPX-S113"));
    let mut resolved = hir::resolve(&program(SOURCE)).unwrap();
    let main = resolved
        .functions
        .iter_mut()
        .find(|f| f.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!()
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!()
    };
    let ResolvedExprKind::Call { type_arguments, .. } = &mut value.kind else {
        panic!()
    };
    type_arguments[0] = ResolvedType::Bytes;
    assert_eq!(hir::validate(&resolved).unwrap_err().code, "SPX-H006");

    let reused = r#"module hostile.box.reuse;
@id("hostile.main") fn main()->i64 { let value=box_new<i64>(1); let inner=box_into_inner<i64>(value); inner + box_get<i64>(value) }
"#;
    assert!(!errors(reused).is_empty());

    let escaping = r#"module hostile.box.escape;
@id("hostile.leak") fn leak(value: borrow Box<i64>)->Box<i64> { value }
@id("hostile.main") fn main()->i64 { let value=box_new<i64>(1); box_into_inner<i64>(leak(value)) }
"#;
    assert!(!errors(escaping).is_empty());

    for forged_wrapper in [
        r#"module std.mem;
@id("std.mem.box.lookalike") fn new<T>(value:T)->Box<T>{box_new<T>(value)}
"#,
        r#"module std.mem;
@id("std.mem.box.new") fn new<T>(value:T)->Box<T>{value}
"#,
    ] {
        assert!(errors(forged_wrapper).contains(&"SPX-T286"));
    }

    let wrapper = r#"module std.mem;
@id("std.mem.box.new") fn new<T>(value:T)->Box<T>{box_new<T>(value)}
@id("app.main") fn main()->i64{0}
"#;
    let mut forged_wrapper_hir = hir::resolve(&program(wrapper)).unwrap();
    let template = forged_wrapper_hir.function_templates.first_mut().unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!()
    };
    let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!()
    };
    *callee = hir::DeclarationId::new("core.box.get");
    assert_eq!(
        hir::validate(&forged_wrapper_hir).unwrap_err().code,
        "SPX-H006"
    );

    let mut forged_cleanup = hir::resolve(&program(SOURCE)).unwrap();
    let main = forged_cleanup
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    let slot = main
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|slot| {
            matches!(&slot.field_liveness_shape, semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } if lifecycle.as_str() == "core.box.drop")
        })
        .unwrap();
    let semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } =
        &mut slot.field_liveness_shape
    else {
        panic!()
    };
    *lifecycle = hir::DeclarationId::new("core.bytes.drop");
    assert_eq!(hir::validate(&forged_cleanup).unwrap_err().code, "SPX-H006");
}
