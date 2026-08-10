use std::path::Path;

use semaprax::cleanup_plan::{
    CleanupTransition, StagedCopyResultSource, CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V3,
};
use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::hir::{self, ResolvedExprKind, ResolvedType};
use semaprax::{format, parse, verify};

const SOURCE: &str = r#"
module test.option_try;

@id("option.source_i64")
fn source_i64(absent: bool, value: i64) -> Option<i64> {
    if absent {
        Option<i64>::None {}
    } else {
        Option<i64>::Some { value: value }
    }
}

@id("option.propagate")
fn propagate(absent: bool, value: i64, divisor: i64) -> Option<bool>
    ensures divisor != 13
{
    let number = source_i64(absent, value)?;
    Option<bool>::Some { value: (number + 1) / divisor > 0 }
}

@id("option.legacy")
fn legacy(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("option-try.spx")).unwrap()
}

fn errors(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("option-try-error.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn try_option(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "option.propagate")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("propagate must remain a block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0];
    value
}

fn option_none_source(program: &mut hir::ResolvedProgram) -> &mut StagedCopyResultSource {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "option.propagate")
        .unwrap();
    function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::StageCopyResult { source }
                if matches!(source, StagedCopyResultSource::TryOptionNone { .. }) =>
            {
                Some(source)
            }
            _ => None,
        })
        .expect("Option `?` must stage an authenticated None result")
}

#[test]
fn option_try_round_trips_and_rejects_cross_carrier_and_contract_use() {
    let program = program();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("let number = source_i64(absent, value)?;"));
    let reparsed = parse(&canonical, Path::new("option-try-canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));

    let result_outer = SOURCE
        .replace(
            "-> Option<bool>\n    ensures",
            "-> Result<bool, bool>\n    ensures",
        )
        .replace(
            "Option<bool>::Some { value: (number + 1) / divisor > 0 }",
            "Result<bool, bool>::Ok { value: (number + 1) / divisor > 0 }",
        );
    assert_eq!(errors(&result_outer), ["SPX-T218"]);

    let contract = SOURCE.replace(
        "ensures divisor != 13",
        "requires source_i64(false, value)? > 0\n    ensures divisor != 13",
    );
    assert_eq!(errors(&contract), ["SPX-T218"]);
}

#[test]
fn option_try_rejects_nested_custom_and_live_resource_payloads() {
    let nested = r#"
module test.option_try_nested;
@id("test.nested")
fn nested(value: Option<Option<i64>>) -> Option<bool> {
    let inner = value?;
    Option<bool>::None {}
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(errors(nested).contains(&"SPX-T218"));

    let nested_outer = r#"
module test.option_try_nested_outer;
@id("test.nested_outer")
fn nested_outer(value: Option<i64>) -> Option<Option<bool>> {
    let inner = value?;
    Option<Option<bool>>::None {}
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(errors(nested_outer).contains(&"SPX-T218"));

    let custom = r#"
module test.option_try_custom;
@id("test.maybe")
variant Maybe<T> {
    @id("test.maybe.none") None,
    @id("test.maybe.some") Some { @id("test.maybe.some.value") value: T, },
}
@id("test.custom")
fn custom(value: Maybe<i64>) -> Option<bool> {
    let inner = value?;
    Option<bool>::Some { value: inner > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(errors(custom).contains(&"SPX-T218"));

    let resource = r#"
module test.option_try_resource;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("test.resource")
fn resource(token: own Token, value: Option<i64>) -> Option<bool> {
    let inner = value?;
    Option<bool>::Some { value: inner > 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(errors(resource).contains(&"SPX-T218"));
}

#[test]
fn resolved_option_try_authenticates_carrier_members_instances_and_schema() {
    let mut resolved = hir::resolve(&program()).unwrap();
    hir::validate(&resolved).unwrap();
    let expression = try_option(&mut resolved);
    let ResolvedExprKind::TryOption {
        operand,
        option,
        some_case,
        some_field,
        none_case,
        residual_type,
    } = &expression.kind
    else {
        panic!("initializer must be TryOption");
    };
    assert_eq!(option.as_str(), "core.option");
    assert_eq!(some_case.as_str(), "core.option.some");
    assert_eq!(some_field.as_str(), "core.option.some.value");
    assert_eq!(none_case.as_str(), "core.option.none");
    assert_eq!(expression.ty, ResolvedType::I64);
    assert_ne!(operand.ty, *residual_type);
    assert_eq!(
        resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == "option.propagate")
            .unwrap()
            .cleanup_plan
            .schema,
        CLEANUP_PLAN_SCHEMA_V3
    );
    assert!(resolved
        .functions
        .iter()
        .filter(|function| function.id.as_str() != "option.propagate")
        .all(|function| function.cleanup_plan.schema == CLEANUP_PLAN_SCHEMA_V2));

    for identity in 0..4 {
        let mut hostile = hir::resolve(&program()).unwrap();
        let ResolvedExprKind::TryOption {
            option,
            some_case,
            some_field,
            none_case,
            ..
        } = &mut try_option(&mut hostile).kind
        else {
            unreachable!();
        };
        *match identity {
            0 => option,
            1 => some_case,
            2 => some_field,
            _ => none_case,
        } = hir::DeclarationId::new("core.result");
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }

    let mut wrong_schema = hir::resolve(&program()).unwrap();
    wrong_schema
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "option.propagate")
        .unwrap()
        .cleanup_plan
        .schema = CLEANUP_PLAN_SCHEMA_V2;
    assert_eq!(hir::validate(&wrong_schema).unwrap_err().code, "SPX-H006");

    let legacy_source = parse(
        "module test.legacy_schema; @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("legacy-schema.spx"),
    )
    .unwrap();
    let mut wrong_legacy_schema = hir::resolve(&legacy_source).unwrap();
    wrong_legacy_schema.functions[0].cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V3;
    assert_eq!(
        hir::validate(&wrong_legacy_schema).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn cleanup_replay_rejects_option_none_source_tampering_and_stage_count_changes() {
    for mutation in 0..8 {
        let mut hostile = hir::resolve(&program()).unwrap();
        let foreign_expression = hostile
            .functions
            .iter()
            .find(|function| function.id.as_str() == "option.legacy")
            .unwrap()
            .body
            .id
            .clone();
        let source = option_none_source(&mut hostile);
        let StagedCopyResultSource::TryOptionNone {
            expression,
            operand,
            source_instance,
            target_instance,
            option,
            some_case,
            some_field,
            none_case,
        } = source
        else {
            unreachable!();
        };
        match mutation {
            0 => *expression = foreign_expression,
            1 => *operand = foreign_expression,
            2 => *source_instance = ResolvedType::Bool,
            3 => *target_instance = ResolvedType::I64,
            4 => *option = hir::DeclarationId::new("core.result"),
            5 => *some_case = hir::DeclarationId::new("core.option.none"),
            6 => *some_field = hir::DeclarationId::new("core.result.ok.value"),
            _ => *none_case = hir::DeclarationId::new("core.result.err"),
        }
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }

    let mut missing = hir::resolve(&program()).unwrap();
    let function = missing
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "option.propagate")
        .unwrap();
    let (block, position) = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .find_map(|block| {
            block
                .transitions
                .iter()
                .position(|transition| {
                    matches!(
                        transition,
                        CleanupTransition::StageCopyResult {
                            source: StagedCopyResultSource::TryOptionNone { .. }
                        }
                    )
                })
                .map(|position| (block, position))
        })
        .unwrap();
    let staged = block.transitions.remove(position);
    assert_eq!(hir::validate(&missing).unwrap_err().code, "SPX-H006");

    let mut duplicate = hir::resolve(&program()).unwrap();
    let function = duplicate
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "option.propagate")
        .unwrap();
    let block = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block.transitions.iter().any(|transition| {
                matches!(
                    transition,
                    CleanupTransition::StageCopyResult {
                        source: StagedCopyResultSource::TryOptionNone { .. }
                    }
                )
            })
        })
        .unwrap();
    block.transitions.push(staged);
    assert_eq!(hir::validate(&duplicate).unwrap_err().code, "SPX-H006");
}

#[test]
fn graph_v11_and_context_are_program_bound_while_result_free_legacy_stays_v10() {
    let program = program();
    let graph = graph::to_json(&program).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v11\","));
    assert!(graph.contains("\"kind\":\"try_option\",\"evaluation\":\"once\""));
    assert!(graph.contains("\"none_exit\":\"normal_result\""));
    assert!(graph.contains("\"schema\":\"semaprax.cleanup-plan.v3\""));

    let options = AgentContextOptions::new(
        0,
        32 * 1024,
        8,
        [AgentContextFilter::Types, AgentContextFilter::Contracts],
    )
    .unwrap();
    let context = graph::agent_context_json(&program, "option.legacy", &options)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"source_graph_schema\":\"semaprax.graph.v11\""));

    let legacy = parse(
        "module test.legacy; @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("legacy.spx"),
    )
    .unwrap();
    assert!(graph::to_json(&legacy)
        .unwrap()
        .starts_with("{\"schema\":\"semaprax.graph.v10\","));
}
