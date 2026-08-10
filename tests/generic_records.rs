use std::path::Path;

use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::hir::{self, ResolvedExprKind, ResolvedType, ResolvedTypeDeclarationKind};
use semaprax::{format, parse, verify};

const SOURCE: &str = r#"
module test.generic_records;

@id("test.box")
record Box<T> {
    @id("test.box.value") value: T,
}

@id("test.pair")
record Pair<T> {
    @id("test.pair.left") left: T,
    @id("test.pair.right") right: T,
}

@id("test.duo")
record Duo<T, U> {
    @id("test.duo.left") left: T,
    @id("test.duo.right") right: U,
}

@id("test.phantom")
record Phantom<T> {
    @id("test.phantom.marker") marker: bool,
}

@id("test.box_i64")
fn box_i64(value: i64) -> Box<i64>
    requires (Box<i64> { value: value }).value == value
{
    Box<i64> { value: value }
}

@id("test.bump")
fn bump(boxed: Box<i64>) -> Box<i64> {
    boxed with { value: boxed.value + 1 }
}

@id("test.read_bool")
fn read_bool(boxed: Box<bool>) -> i64 {
    if boxed.value { 1 } else { 0 }
}

@id("test.sum_pair")
fn sum_pair(pair: Pair<i64>) -> i64 { pair.left + pair.right }

@id("test.duo_value")
fn duo_value(value: i64, flag: bool) -> Duo<i64, bool> {
    Duo<i64, bool> { left: value, right: flag }
}

@id("test.legacy")
fn legacy(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 {
    let first = bump(box_i64(19));
    let flag = Box<bool> { value: true };
    let pair = Pair<i64> { left: first.value, right: 21 };
    let duo = duo_value(1, true);
    sum_pair(pair) + read_bool(flag) + if duo.right { duo.left - 1 } else { 0 }
}
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("generic-records.spx")).unwrap()
}

fn errors(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("generic-record-error.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn nominal(id: &str, argument: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: hir::DeclarationId::new(id),
        arguments: vec![argument],
    }
}

#[test]
fn generic_records_round_trip_and_resolve_exact_concrete_instances() {
    let program = program();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("record Box<T> {"));
    assert!(canonical.contains("Box<i64> { value: value }"));
    assert!(canonical.contains("Box<bool> { value: true }"));
    let reparsed = parse(&canonical, Path::new("generic-records-canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let box_declaration = resolved
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "test.box")
        .unwrap();
    assert_eq!(box_declaration.type_parameters.len(), 1);
    let ResolvedTypeDeclarationKind::Record { fields } = &box_declaration.kind else {
        panic!("Box must remain a record template");
    };
    assert_eq!(
        fields[0].ty,
        ResolvedType::TypeParameter {
            owner: box_declaration.id.clone(),
            index: 0,
        }
    );

    let box_i64 = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.box_i64")
        .unwrap();
    assert_eq!(box_i64.return_type, nominal("test.box", ResolvedType::I64));
    let ResolvedExprKind::Block { tail, .. } = &box_i64.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::ConstructRecord { fields, .. } = &tail.kind else {
        panic!("box_i64 must construct its exact instance");
    };
    assert_eq!(tail.ty, nominal("test.box", ResolvedType::I64));
    assert_eq!(fields[0].value.ty, ResolvedType::I64);

    let read_bool = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.read_bool")
        .unwrap();
    assert_eq!(
        read_bool.params[0].ty,
        nominal("test.box", ResolvedType::Bool)
    );

    let duo_declaration = resolved
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "test.duo")
        .unwrap();
    assert_eq!(
        duo_declaration
            .type_parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter.index))
            .collect::<Vec<_>>(),
        [("T", 0), ("U", 1)]
    );
    let duo_function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.duo_value")
        .unwrap();
    assert_eq!(
        duo_function.return_type,
        ResolvedType::Nominal {
            declaration: duo_declaration.id.clone(),
            arguments: vec![ResolvedType::I64, ResolvedType::Bool],
        }
    );
    let ResolvedExprKind::Block { tail, .. } = &duo_function.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::ConstructRecord { fields, .. } = &tail.kind else {
        unreachable!();
    };
    assert_eq!(fields[0].value.ty, ResolvedType::I64);
    assert_eq!(fields[1].value.ty, ResolvedType::Bool);
}

#[test]
fn generic_record_admission_is_explicit_arity_checked_and_direct_copy_only() {
    let declaration = r#"
@id("test.box")
record Box<T> { @id("test.box.value") value: T, }
"#;
    let wrong_arity = format!(
        "module test.wrong_arity;\n{declaration}\n@id(\"test.take\") fn take(value: Box<i64, bool>) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert_eq!(errors(&wrong_arity), ["SPX-T221"]);

    let missing_constructor_argument = format!(
        "module test.missing_argument;\n{declaration}\n@id(\"app.main\") fn main() -> i64 {{ Box {{ value: 1 }}.value }}"
    );
    assert!(errors(&missing_constructor_argument).contains(&"SPX-T221"));

    let nested = format!(
        "module test.nested;\n{declaration}\n@id(\"test.take\") fn take(value: Box<Option<i64>>) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert_eq!(errors(&nested), ["SPX-T223"]);

    let unknown_parameter = format!(
        "module test.unknown;\n{}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}",
        declaration.replace("value: T", "value: U")
    );
    assert_eq!(errors(&unknown_parameter), ["SPX-T220"]);

    let duplicate_parameter = format!(
        "module test.duplicate;\n{}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}",
        declaration.replace("Box<T>", "Box<T, T>")
    );
    assert_eq!(errors(&duplicate_parameter), ["SPX-T220"]);

    let nested_template = format!(
        "module test.nested_template;\n{}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}",
        declaration.replace("value: T", "value: Option<T>")
    );
    assert!(errors(&nested_template).contains(&"SPX-T223"));

    let nested_field = format!(
        "module test.nested_field;\n{declaration}\n@id(\"test.outer\") record Outer {{ @id(\"test.outer.box\") box: Box<i64>, }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert!(errors(&nested_field).contains(&"SPX-T223"));

    let generic_resource = r#"
module test.generic_resource;
@id("test.resource") resource Resource<T>;
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(errors(generic_resource).contains(&"SPX-T223"));

    let record_shadow = r#"
module test.record_shadow;
@id("test.t") record T { @id("test.t.value") value: i64, }
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.unwrap") fn unwrap(value: Box<i64>) -> i64 { value.value }
@id("app.main") fn main() -> i64 { unwrap(Box<i64> { value: 42 }) }
"#;
    assert!(errors(record_shadow).is_empty());

    let resource_shadow = r#"
module test.resource_shadow;
@id("test.t") resource T { @id("test.t.drop") drop trivial; }
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.unwrap") fn unwrap(value: Box<i64>) -> i64 { value.value }
@id("app.main") fn main() -> i64 { unwrap(Box<i64> { value: 42 }) }
"#;
    assert!(errors(resource_shadow).is_empty());
}

#[test]
fn independent_hir_rejects_cross_instance_constructor_and_field_substitution() {
    let mut wrong_instance = hir::resolve(&program()).unwrap();
    let function = wrong_instance
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.box_i64")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        unreachable!();
    };
    tail.ty = nominal("test.box", ResolvedType::Bool);
    assert_eq!(hir::validate(&wrong_instance).unwrap_err().code, "SPX-H006");

    let mut wrong_field_value = hir::resolve(&program()).unwrap();
    let function = wrong_field_value
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.box_i64")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::ConstructRecord { fields, .. } = &mut tail.kind else {
        unreachable!();
    };
    fields[0].value.ty = ResolvedType::Bool;
    assert_eq!(
        hir::validate(&wrong_field_value).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_parameter_instance = hir::resolve(&program()).unwrap();
    let function = wrong_parameter_instance
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.read_bool")
        .unwrap();
    function.params[0].ty = nominal("test.box", ResolvedType::I64);
    assert_eq!(
        hir::validate(&wrong_parameter_instance).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_parameter_owner = hir::resolve(&program()).unwrap();
    let declaration = wrong_parameter_owner
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "test.box")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut declaration.kind else {
        unreachable!();
    };
    fields[0].ty = ResolvedType::TypeParameter {
        owner: hir::DeclarationId::new("test.pair"),
        index: 0,
    };
    assert_eq!(
        hir::validate(&wrong_parameter_owner).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_parameter_index = hir::resolve(&program()).unwrap();
    let declaration = wrong_parameter_index
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "test.box")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut declaration.kind else {
        unreachable!();
    };
    fields[0].ty = ResolvedType::TypeParameter {
        owner: declaration.id.clone(),
        index: 1,
    };
    assert_eq!(
        hir::validate(&wrong_parameter_index).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_field_order = hir::resolve(&program()).unwrap();
    let declaration = wrong_field_order
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "test.pair")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut declaration.kind else {
        unreachable!();
    };
    fields.swap(0, 1);
    assert_eq!(
        hir::validate(&wrong_field_order).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_argument_order = hir::resolve(&program()).unwrap();
    let function = wrong_argument_order
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.duo_value")
        .unwrap();
    let ResolvedType::Nominal { arguments, .. } = &mut function.return_type else {
        unreachable!();
    };
    arguments.swap(0, 1);
    assert_eq!(
        hir::validate(&wrong_argument_order).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn graph_v12_is_program_wide_and_serializes_template_and_instance_identity() {
    fn accepts_v12(bytes: &str) -> bool {
        bytes.starts_with("{\"schema\":\"semaprax.graph.v12\",")
            && bytes.contains("\"type_parameters\":[")
            && bytes.contains("\"record_type\":{\"kind\":\"nominal\"")
            && bytes.ends_with('}')
    }

    let program = program();
    let graph = graph::to_json(&program).unwrap();
    assert!(accepts_v12(&graph));
    assert!(!accepts_v12(&graph.replacen(
        "semaprax.graph.v12",
        "semaprax.graph.v11",
        1
    )));
    assert!(!accepts_v12(&graph.replacen(
        "semaprax.graph.v12",
        "semaprax.graph.v10",
        1
    )));
    assert!(graph.contains(
        "\"id\":\"test.box\",\"kind\":\"record\",\"name\":\"Box\",\"identity_origin\":\"explicit\",\"persistent\":true,\"type_parameters\":["
    ));
    assert!(graph.contains("\"type_id\":null,\"fields\":[\"test.box.value\"]"));
    assert!(graph.contains(
        "\"kind\":\"construct_record\",\"record\":\"test.box\",\"record_type\":{\"kind\":\"nominal\",\"declaration\":\"test.box\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"i64\"}]}"
    ));
    assert!(graph.contains("nominal:8:test.box:1:3:i64"));
    assert!(graph.contains("nominal:8:test.box:1:4:bool"));
    assert!(graph.contains("nominal:8:test.duo:2:3:i644:bool"));
    assert!(graph.contains(
        "\"record_type\":{\"kind\":\"nominal\",\"declaration\":\"test.duo\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"i64\"},{\"kind\":\"primitive\",\"name\":\"bool\"}]}"
    ));

    let options = AgentContextOptions::new(0, 32 * 1024, 16, [AgentContextFilter::Types]).unwrap();
    let context = graph::agent_context_json(&program, "test.legacy", &options)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"source_graph_schema\":\"semaprax.graph.v12\""));

    let contract_options =
        AgentContextOptions::new(0, 32 * 1024, 16, [AgentContextFilter::Contracts]).unwrap();
    let constructor_context =
        graph::agent_context_json(&program, "test.box_i64", &contract_options)
            .unwrap()
            .unwrap();
    assert!(constructor_context.contains(
        "\"record_type\":{\"kind\":\"nominal\",\"declaration\":\"test.box\",\"arguments\":[{\"kind\":\"primitive\",\"name\":\"i64\"}]}"
    ));

    let legacy = parse(
        "module test.legacy; @id(\"test.plain\") record Plain { @id(\"test.plain.value\") value: i64, } @id(\"app.main\") fn main() -> i64 { Plain { value: 1 }.value }",
        Path::new("legacy-records.spx"),
    )
    .unwrap();
    assert!(graph::to_json(&legacy)
        .unwrap()
        .starts_with("{\"schema\":\"semaprax.graph.v10\","));
}

#[test]
fn graph_v12_precedes_try_option_and_unused_generic_declarations_still_select_v12() {
    let mixed = r#"
module test.generic_record_option;
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.source") fn source(absent: bool) -> Option<i64> {
    if absent { Option<i64>::None {} } else { Option<i64>::Some { value: 1 } }
}
@id("test.propagate") fn propagate(absent: bool) -> Option<bool> {
    let value = source(absent)?;
    Option<bool>::Some { value: value > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let mixed = parse(mixed, Path::new("generic-record-option.spx")).unwrap();
    let mixed_graph = graph::to_json(&mixed).unwrap();
    assert!(mixed_graph.starts_with("{\"schema\":\"semaprax.graph.v12\","));
    assert!(mixed_graph.contains("\"kind\":\"try_option\""));

    let option_only = r#"
module test.option_only;
@id("test.source") fn source(absent: bool) -> Option<i64> {
    if absent { Option<i64>::None {} } else { Option<i64>::Some { value: 1 } }
}
@id("test.propagate") fn propagate(absent: bool) -> Option<bool> {
    let value = source(absent)?;
    Option<bool>::Some { value: value > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let option_only = parse(option_only, Path::new("option-only.spx")).unwrap();
    assert!(graph::to_json(&option_only)
        .unwrap()
        .starts_with("{\"schema\":\"semaprax.graph.v11\","));

    let unused = r#"
module test.unused_generic_record;
@id("test.phantom") record Phantom<T> { @id("test.phantom.marker") marker: bool, }
@id("test.legacy") fn legacy(value: i64) -> i64 { value }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let unused = parse(unused, Path::new("unused-generic-record.spx")).unwrap();
    let unused_graph = graph::to_json(&unused).unwrap();
    assert!(unused_graph.starts_with("{\"schema\":\"semaprax.graph.v12\","));
    let options = AgentContextOptions::new(0, 32 * 1024, 8, [AgentContextFilter::Types]).unwrap();
    let context = graph::agent_context_json(&unused, "test.legacy", &options)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"source_graph_schema\":\"semaprax.graph.v12\""));
}

#[test]
fn cleanup_replay_keeps_generic_copy_records_resource_free_and_canonical() {
    let resolved = hir::resolve(&program()).unwrap();
    for function in &resolved.functions {
        assert!(function.cleanup_plan.slots.is_empty());
        assert_eq!(
            function.cleanup_plan.schema,
            semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2
        );
    }
    hir::validate(&resolved).unwrap();
}
