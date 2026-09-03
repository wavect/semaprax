use std::path::Path;

use semaprax::cleanup_plan::{EdgeCondition, CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V3};
use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedType,
};
use semaprax::{format, parse, verify};

const SOURCE: &str = r#"
module test.record_patterns;

@id("test.inner")
record Inner {
    @id("test.inner.value") value: i64,
    @id("test.inner.flag") flag: bool,
}

@id("test.outer")
record Outer {
    @id("test.outer.inner") inner: Inner,
    @id("test.outer.other") other: i64,
}

@id("test.box")
record Box<T> { @id("test.box.value") value: T, }

@id("test.nested")
fn nested(input: Outer) -> i64 {
    match input {
        Outer { inner: Inner { value: renamed, flag: _ }, other } => renamed + other,
    }
}

@id("test.box_i64")
fn box_i64(input: Box<i64>) -> i64 {
    match input { Box { value } => value, }
}

@id("test.whole_inner")
fn whole_inner(input: Outer) -> i64 {
    match input { Outer { inner, other: _ } => if inner.flag { inner.value } else { 0 }, }
}

@id("test.box_bool")
fn box_bool(input: Box<bool>) -> bool {
    match input { Box { value: truth } => truth, }
}

@id("test.wildcard")
fn wildcard(input: Outer) -> i64 { match input { _ => 7, } }

@id("test.explicit_constant")
fn explicit_constant(input: Outer) -> i64 {
    match input { Outer { inner: _, other: _ } => 7, }
}

@id("app.main")
fn main() -> i64 {
    nested(Outer { inner: Inner { value: 20, flag: false }, other: 22 })
}
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("record-patterns.spx")).unwrap()
}

fn error_codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("record-pattern-errors.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn tail_pattern_mut<'a>(
    program: &'a mut hir::ResolvedProgram,
    function_id: &str,
) -> &'a mut ResolvedMatchPattern {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == function_id)
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("function body must be a block");
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("function tail must be a match");
    };
    &mut arms[0].pattern
}

#[test]
fn record_patterns_round_trip_and_resolve_recursive_exact_instances() {
    let program = program();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical
        .contains("Outer { inner: Inner { value: renamed, flag: _ }, other } => renamed + other"));
    assert!(canonical.contains("match input { Box { value } => value, }"));
    let reparsed = parse(&canonical, Path::new("record-patterns-canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let nested = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.nested")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &nested.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        unreachable!();
    };
    let ResolvedMatchPattern::Record {
        record,
        instance,
        fields,
    } = &arms[0].pattern
    else {
        panic!("nested must resolve to an explicit record pattern");
    };
    assert_eq!(record.as_str(), "test.outer");
    assert_eq!(
        instance,
        &ResolvedType::Nominal {
            declaration: DeclarationId::new("test.outer"),
            arguments: Vec::new(),
        }
    );
    assert_eq!(fields[0].field.as_str(), "test.outer.inner");
    let ResolvedRecordMatchFieldPattern::Record {
        record,
        instance,
        fields: inner_fields,
    } = &fields[0].pattern
    else {
        panic!("inner field must retain its nested record pattern");
    };
    assert_eq!(record.as_str(), "test.inner");
    assert_eq!(
        instance,
        &ResolvedType::Nominal {
            declaration: DeclarationId::new("test.inner"),
            arguments: Vec::new(),
        }
    );
    assert_eq!(inner_fields[0].field.as_str(), "test.inner.value");
    let ResolvedRecordMatchFieldPattern::Binding(binding) = &inner_fields[0].pattern else {
        unreachable!();
    };
    assert_eq!(binding.name, "renamed");
    assert_eq!(binding.ty, ResolvedType::I64);
    assert!(matches!(
        inner_fields[1].pattern,
        ResolvedRecordMatchFieldPattern::Wildcard
    ));

    let whole = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.whole_inner")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &whole.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        unreachable!();
    };
    let ResolvedMatchPattern::Record { fields, .. } = &arms[0].pattern else {
        unreachable!();
    };
    let ResolvedRecordMatchFieldPattern::Binding(inner) = &fields[0].pattern else {
        panic!("whole Inner field must remain an aggregate Copy binding");
    };
    assert_eq!(
        inner.ty,
        ResolvedType::Nominal {
            declaration: DeclarationId::new("test.inner"),
            arguments: Vec::new(),
        }
    );
}

#[test]
fn record_pattern_diagnostics_are_stable_and_fail_closed() {
    let missing = SOURCE.replace(
        "Outer { inner: Inner { value: renamed, flag: _ }, other } => renamed + other,",
        "Outer { inner: Inner { value: renamed, flag: _ } } => renamed,",
    );
    assert_eq!(error_codes(&missing), ["SPX-M104"]);

    let duplicate = SOURCE.replace(
        "Inner { value: renamed, flag: _ }",
        "Inner { value: renamed, value: duplicate, flag: _ }",
    );
    assert_eq!(error_codes(&duplicate), ["SPX-M104"]);

    let foreign_nested = SOURCE.replace(
        "Outer { inner: Inner { value: renamed, flag: _ }, other }",
        "Outer { inner: Outer { inner: _, other: _ }, other }",
    );
    assert!(error_codes(&foreign_nested).contains(&"SPX-M103"));

    let extra_arm = SOURCE.replace(
        "Outer { inner: Inner { value: renamed, flag: _ }, other } => renamed + other,",
        "Outer { inner: Inner { value: renamed, flag: _ }, other } => renamed + other,\n        _ => 0,",
    );
    assert_eq!(error_codes(&extra_arm), ["SPX-M102"]);

    let aggregate_result = SOURCE.replace(
        "Outer { inner: Inner { value: renamed, flag: _ }, other } => renamed + other,",
        "Outer { inner: Inner { value: renamed, flag: _ }, other } => input,",
    );
    assert_eq!(error_codes(&aggregate_result), ["SPX-T216"]);

    let resource = r#"
module test.resource_record_pattern;
@id("test.handle") resource Handle { @id("test.handle.drop") drop trivial; }
@id("test.holder") record Holder { @id("test.holder.handle") handle: Handle, }
@id("test.inspect") fn inspect(value: own Holder) -> i64 {
    match value { Holder { handle: _ } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let codes = error_codes(resource);
    assert!(codes.contains(&"SPX-O111"));
    assert!(codes.contains(&"SPX-M103"));
}

#[test]
fn graph_v13_is_program_wide_exact_and_wildcard_patterns_are_schema_neutral() {
    let program = program();
    let json = graph::to_json(&program).unwrap();
    let accepts_v13 = |candidate: &str| {
        candidate.starts_with("{\"schema\":\"semaprax.graph.v13\",")
            && candidate.contains("\"kind\":\"record_pattern\"")
    };
    assert!(accepts_v13(&json));
    for lower in [
        "semaprax.graph.v12",
        "semaprax.graph.v11",
        "semaprax.graph.v10",
    ] {
        assert!(!accepts_v13(&json.replacen("semaprax.graph.v13", lower, 1)));
    }
    assert!(json.contains("\"kind\":\"record_pattern\""));
    assert!(json.contains("\"record\":\"test.outer\""));
    assert!(json.contains("\"record_type_id\":\"nominal:10:test.outer:0:\""));
    assert!(json.contains("\"field\":\"test.outer.inner\""));
    assert!(json.contains("\"field\":\"test.inner.value\""));
    assert!(json.contains("\"name\":\"renamed\",\"type_id\":\"i64\""));
    assert!(json.contains("\"kind\":\"wildcard_pattern\""));

    let context = graph::context_json(&program, "test.wildcard", 0)
        .unwrap()
        .unwrap();
    assert!(context.starts_with("{\"schema\":\"semaprax.graph.v13\","));
    let pattern_context = graph::context_json(&program, "test.nested", 0)
        .unwrap()
        .unwrap();
    assert!(pattern_context.starts_with("{\"schema\":\"semaprax.graph.v13\","));
    assert!(pattern_context.contains("\"kind\":\"record_pattern\""));
    assert!(pattern_context.contains("\"record_type_id\":\"nominal:10:test.outer:0:\""));

    let options = AgentContextOptions::new(0, 64 * 1024, 32, [AgentContextFilter::Types]).unwrap();
    let agent = graph::agent_context_json(&program, "test.nested", &options)
        .unwrap()
        .unwrap();
    assert!(agent.contains("\"source_graph_schema\":\"semaprax.graph.v13\""));

    let wildcard_only = parse(
        r#"
module test.wildcard_only;
@id("test.plain") record Plain { @id("test.plain.value") value: i64, }
@id("test.use") fn use(value: Plain) -> i64 { match value { _ => 7, } }
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("record-wildcard-only.spx"),
    )
    .unwrap();
    let wildcard_json = graph::to_json(&wildcard_only).unwrap();
    assert!(wildcard_json.starts_with("{\"schema\":\"semaprax.graph.v10\","));
    assert!(!wildcard_json.contains("record_pattern"));

    let generic_wildcard = parse(
        r#"
module test.generic_wildcard;
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.use") fn use(value: Box<i64>) -> i64 { match value { _ => 7, } }
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("generic-record-wildcard.spx"),
    )
    .unwrap();
    let generic_json = graph::to_json(&generic_wildcard).unwrap();
    assert!(generic_json.starts_with("{\"schema\":\"semaprax.graph.v12\","));
    assert!(!generic_json.contains("record_pattern"));

    let mixed_option = parse(
        r#"
module test.record_pattern_option;
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.read") fn read(input: Box<i64>) -> i64 {
    match input { Box { value } => value, }
}
@id("test.source") fn source(absent: bool) -> Option<i64> {
    if absent { Option<i64>::None {} } else { Option<i64>::Some { value: 1 } }
}
@id("test.propagate") fn propagate(absent: bool) -> Option<bool> {
    let value = source(absent)?;
    Option<bool>::Some { value: value > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("record-pattern-option.spx"),
    )
    .unwrap();
    let mixed_json = graph::to_json(&mixed_option).unwrap();
    assert!(mixed_json.starts_with("{\"schema\":\"semaprax.graph.v13\","));
    assert!(mixed_json.contains("\"kind\":\"record_pattern\""));
    assert!(mixed_json.contains("\"kind\":\"try_option\""));

    let wildcard_option = parse(
        r#"
module test.record_wildcard_option;
@id("test.plain") record Plain { @id("test.plain.value") value: i64, }
@id("test.ignore") fn ignore(value: Plain) -> i64 { match value { _ => 7, } }
@id("test.source") fn source(absent: bool) -> Option<i64> {
    if absent { Option<i64>::None {} } else { Option<i64>::Some { value: 1 } }
}
@id("test.propagate") fn propagate(absent: bool) -> Option<bool> {
    let value = source(absent)?;
    Option<bool>::Some { value: value > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("record-wildcard-option.spx"),
    )
    .unwrap();
    let wildcard_option_json = graph::to_json(&wildcard_option).unwrap();
    assert!(wildcard_option_json.starts_with("{\"schema\":\"semaprax.graph.v11\","));
    assert!(!wildcard_option_json.contains("record_pattern"));

    let legacy = parse(
        r#"
module examples.record_graph;
@id("geometry.point") record Point {
    @id("geometry.point.x") x: i64,
    @id("geometry.point.y") y: i64,
}
@id("app.main") fn main() -> i64 { Point { x: 20, y: 22 }.x }
"#,
        Path::new("record-graph.spx"),
    )
    .unwrap();
    assert_eq!(
        format!("{}\n", graph::to_json(&legacy).unwrap()),
        include_str!("../snapshots/records.graph.json")
    );
}

#[test]
fn hir_and_cleanup_replay_reject_record_pattern_identity_confusion() {
    let resolved = hir::resolve(&program()).unwrap();
    for function in &resolved.functions {
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V2);
    }
    hir::validate(&resolved).unwrap();
    let wildcard = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.wildcard")
        .unwrap();
    let explicit = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.explicit_constant")
        .unwrap();
    for plan in [&wildcard.cleanup_plan, &explicit.cleanup_plan] {
        assert!(plan.slots.is_empty());
        assert!(plan.status_sources.is_empty());
        assert!(plan
            .edges
            .iter()
            .all(|edge| matches!(edge.condition, EdgeCondition::Always)));
        assert!(plan.blocks.iter().all(|block| block.transitions.is_empty()));
    }
    assert_eq!(
        (
            wildcard.cleanup_plan.blocks.len(),
            wildcard.cleanup_plan.edges.len(),
            wildcard.cleanup_plan.regions.len(),
            wildcard.cleanup_plan.exits.len(),
        ),
        (
            explicit.cleanup_plan.blocks.len(),
            explicit.cleanup_plan.edges.len(),
            explicit.cleanup_plan.regions.len(),
            explicit.cleanup_plan.exits.len(),
        )
    );

    let mut wrong_instance = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { instance, .. } =
        tail_pattern_mut(&mut wrong_instance, "test.box_i64")
    else {
        unreachable!();
    };
    *instance = ResolvedType::Nominal {
        declaration: DeclarationId::new("test.box"),
        arguments: vec![ResolvedType::Bool],
    };
    assert_eq!(hir::validate(&wrong_instance).unwrap_err().code, "SPX-H006");

    let mut wrong_record = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { record, .. } =
        tail_pattern_mut(&mut wrong_record, "test.box_i64")
    else {
        unreachable!();
    };
    *record = DeclarationId::new("test.outer");
    assert_eq!(hir::validate(&wrong_record).unwrap_err().code, "SPX-H006");

    let mut wrong_field = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { fields, .. } =
        tail_pattern_mut(&mut wrong_field, "test.box_i64")
    else {
        unreachable!();
    };
    fields[0].field = DeclarationId::new("test.outer.other");
    assert_eq!(hir::validate(&wrong_field).unwrap_err().code, "SPX-H006");

    let mut wrong_nested_instance = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { fields, .. } =
        tail_pattern_mut(&mut wrong_nested_instance, "test.nested")
    else {
        unreachable!();
    };
    let ResolvedRecordMatchFieldPattern::Record { instance, .. } = &mut fields[0].pattern else {
        unreachable!();
    };
    *instance = ResolvedType::Nominal {
        declaration: DeclarationId::new("test.outer"),
        arguments: Vec::new(),
    };
    assert_eq!(
        hir::validate(&wrong_nested_instance).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_order = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { fields, .. } =
        tail_pattern_mut(&mut wrong_order, "test.nested")
    else {
        unreachable!();
    };
    fields.swap(0, 1);
    assert_eq!(hir::validate(&wrong_order).unwrap_err().code, "SPX-H006");

    let mut missing_field = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { fields, .. } =
        tail_pattern_mut(&mut missing_field, "test.box_i64")
    else {
        unreachable!();
    };
    fields.clear();
    assert_eq!(hir::validate(&missing_field).unwrap_err().code, "SPX-H006");

    let mut wrong_binding = hir::resolve(&program()).unwrap();
    let forged_id = wrong_binding
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.box_i64")
        .unwrap()
        .params[0]
        .id
        .clone();
    let ResolvedMatchPattern::Record { fields, .. } =
        tail_pattern_mut(&mut wrong_binding, "test.box_i64")
    else {
        unreachable!();
    };
    let ResolvedRecordMatchFieldPattern::Binding(binding) = &mut fields[0].pattern else {
        unreachable!();
    };
    binding.id = forged_id;
    assert_eq!(hir::validate(&wrong_binding).unwrap_err().code, "SPX-H006");

    let mut wrong_binding_type = hir::resolve(&program()).unwrap();
    let ResolvedMatchPattern::Record { fields, .. } =
        tail_pattern_mut(&mut wrong_binding_type, "test.box_i64")
    else {
        unreachable!();
    };
    let ResolvedRecordMatchFieldPattern::Binding(binding) = &mut fields[0].pattern else {
        unreachable!();
    };
    binding.ty = ResolvedType::Bool;
    binding.ownership = OwnershipMode::Own;
    assert_eq!(
        hir::validate(&wrong_binding_type).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_cleanup_schema = hir::resolve(&program()).unwrap();
    wrong_cleanup_schema
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.nested")
        .unwrap()
        .cleanup_plan
        .schema = CLEANUP_PLAN_SCHEMA_V3;
    assert_eq!(
        hir::validate(&wrong_cleanup_schema).unwrap_err().code,
        "SPX-H006"
    );
}
