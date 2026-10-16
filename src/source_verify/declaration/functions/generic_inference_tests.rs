use super::*;
use crate::ast::Program;
use std::path::Path;

fn parsed(source: &str) -> Program {
    crate::parse(source, Path::new("generic-forwarding-precheck.spx")).unwrap()
}

fn mapping_diagnostics(program: &Program) -> Vec<Diagnostic> {
    let function_cells = crate::source_verify::capacity::source_capacity_functions(program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let function_value_targets =
        crate::source_verify::function_value_inventory::function_value_targets(program, &functions);
    let call_graph = program
        .functions
        .iter()
        .map(|function| {
            (
                function.name.clone(),
                crate::source_verify::function_value_inventory::calls(
                    function,
                    &functions,
                    &function_value_targets,
                ),
            )
        })
        .collect();
    let generic_functions = program
        .functions
        .iter()
        .filter(|function| !function.type_parameters.is_empty())
        .map(|function| function.name.as_str())
        .collect();
    let mut diagnostics = Vec::new();
    check_generic_function_cycles(program, &call_graph, &generic_functions, &mut diagnostics);
    diagnostics
}

fn nested_calls(count: usize, explicit: bool) -> Program {
    let mut program = parsed(
        r#"
module test.generic_precheck;
@id("infer.id") fn id<T>(value:T)->T{value}
@id("infer.outer") fn outer<T>(value:T)->T{value}
@id("app.main") fn main()->i64{0}
"#,
    );
    let mut expression = Expr {
        kind: ExprKind::Var("value".to_owned()),
        span: crate::ast::Span::default(),
    };
    for _ in 0..count {
        expression = Expr {
            kind: ExprKind::Call {
                name: "id".to_owned(),
                type_arguments: if explicit {
                    vec![Type::Named {
                        name: "T".to_owned(),
                        arguments: Vec::new(),
                    }]
                } else {
                    Vec::new()
                },
                args: vec![expression],
            },
            span: crate::ast::Span::default(),
        };
    }
    program
        .functions
        .iter_mut()
        .find(|function| function.name == "outer")
        .unwrap()
        .body = expression;
    program
}

#[test]
fn omitted_mapping_depth_fails_closed_without_rejecting_explicit_mapping() {
    assert!(mapping_diagnostics(&nested_calls(128, false)).is_empty());
    let omitted_program = nested_calls(129, false);
    let omitted = mapping_diagnostics(&omitted_program);
    assert!(omitted.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T225"
            && diagnostic
                .message
                .contains("beyond the bounded static traversal")
    }));
    let explicit_program = nested_calls(129, true);
    assert!(mapping_diagnostics(&explicit_program).is_empty());
}

#[test]
fn unknown_let_shadow_cannot_reuse_the_outer_parameter_fact() {
    let program = parsed(
        r#"
module test.generic_shadow;
@id("infer.id") fn id<T>(value:T)->T{value}
@id("infer.outer") fn outer<T>(value:T)->T{{let value=missing;id(value)}}
@id("app.main") fn main()->i64{0}
"#,
    );
    let diagnostics = mapping_diagnostics(&program);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T225"));
}

#[test]
fn match_binding_shadow_cannot_reuse_the_outer_parameter_fact() {
    let program = parsed(
        r#"
module test.generic_match_shadow;
@id("infer.id") fn id<T>(value:T)->T{value}
@id("infer.outer") fn outer<T>(value:T)->T{match value{value=>id(value),}}
@id("app.main") fn main()->i64{0}
"#,
    );
    let diagnostics = mapping_diagnostics(&program);
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T225"));
}
