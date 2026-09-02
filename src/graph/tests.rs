use std::path::Path;

use super::{
    agent_contract_expr_json, agent_function_json, expr_json, graph_json, graph_schema,
    to_hir_json, AgentContextFilter, DeclarationId, GraphView, ResolvedExprKind, ResolvedMatchMode,
    ResolvedProgram,
};
use crate::{hir, parse};

fn resolved_program() -> ResolvedProgram {
    let source = r#"
module test.graph_hir;
@id("app.main")
fn main() -> i64 { 42 }
"#;
    hir::resolve(&parse(source, Path::new("graph-hir.spx")).unwrap()).unwrap()
}

fn resolved_record_program() -> ResolvedProgram {
    let source = r#"
module test.graph_record_hir;
@id("geometry.point")
record Point { @id("geometry.point.x") x: i64, }
@id("app.main")
fn main() -> i64 { Point { x: 42 }.x }
"#;
    hir::resolve(&parse(source, Path::new("graph-record-hir.spx")).unwrap()).unwrap()
}

fn resolved_resource_program() -> ResolvedProgram {
    let source = r#"
module test.graph_resource_hir;
@id("token.type")
resource Token { @id("token.drop") drop trivial; }
@id("token.discard")
fn discard(token: own Token) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    hir::resolve(&parse(source, Path::new("graph-resource-hir.spx")).unwrap()).unwrap()
}

fn resolved_value_match_program() -> ResolvedProgram {
    let source = r#"
module test.graph_match_mode;
@id("mode.pair")
record Pair {
@id("mode.pair.left") left: i64,
@id("mode.pair.right") right: bool,
}
@id("mode.read")
fn read(value: Pair) -> i64 {
match value { Pair { left, right: _ } => left, }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    hir::resolve(&parse(source, Path::new("graph-match-mode.spx")).unwrap()).unwrap()
}

fn match_tail_mut(program: &mut ResolvedProgram) -> &mut crate::hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "mode.read")
        .expect("mode fixture function");
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("mode fixture body is a block")
    };
    tail
}

#[test]
fn explicit_match_modes_select_v21_and_are_distinct_in_graph_and_agent_views() {
    let value = resolved_value_match_program();
    assert_eq!(graph_schema(&value), "semaprax.graph.v13");
    let value_match = value
        .functions
        .iter()
        .find(|function| function.id.as_str() == "mode.read")
        .and_then(|function| match &function.body.kind {
            ResolvedExprKind::Block { tail, .. } => Some(tail.as_ref()),
            _ => None,
        })
        .expect("mode fixture match");
    let value_graph = expr_json(&value, value_match).unwrap();
    let value_agent = agent_contract_expr_json(value_match).unwrap();
    assert!(!value_graph.contains("\"kind\":\"match\",\"ownership_mode\""));
    assert!(!value_agent.contains("\"kind\":\"match\",\"ownership_mode\""));

    let mut own = value.clone();
    let ResolvedExprKind::Match { mode, .. } = &mut match_tail_mut(&mut own).kind else {
        panic!("mode fixture tail is a match")
    };
    *mode = ResolvedMatchMode::Own;
    assert_eq!(graph_schema(&own), "semaprax.graph.v21");
    let own_match = own
        .functions
        .iter()
        .find(|function| function.id.as_str() == "mode.read")
        .and_then(|function| match &function.body.kind {
            ResolvedExprKind::Block { tail, .. } => Some(tail.as_ref()),
            _ => None,
        })
        .unwrap();
    let own_graph = expr_json(&own, own_match).unwrap();
    assert!(own_graph.contains("\"kind\":\"match\",\"ownership_mode\":\"own\""));
    let selected_functions = own
        .functions
        .iter()
        .map(|function| function.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let selected_types = own
        .types
        .iter()
        .map(|declaration| declaration.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let module = graph_json(
        &own,
        "test-source-revision",
        &selected_functions,
        &selected_types,
        &GraphView::Module,
    )
    .unwrap();
    assert!(module.starts_with("{\"schema\":\"semaprax.graph.v21\","));
    assert!(module.contains("\"kind\":\"match\",\"ownership_mode\":\"own\""));
    let root = DeclarationId::new("mode.read");
    let context = graph_json(
        &own,
        "test-source-revision",
        &selected_functions,
        &selected_types,
        &GraphView::Context {
            root: &root,
            depth: 1,
            frontier: &selected_functions,
        },
    )
    .unwrap();
    assert!(context.contains("\"kind\":\"match\",\"ownership_mode\":\"own\""));

    let mut borrow = value;
    let ResolvedExprKind::Match { mode, .. } = &mut match_tail_mut(&mut borrow).kind else {
        panic!("mode fixture tail is a match")
    };
    *mode = ResolvedMatchMode::Borrow;
    assert_eq!(graph_schema(&borrow), "semaprax.graph.v21");
    let borrow_function = borrow
        .functions
        .iter()
        .find(|function| function.id.as_str() == "mode.read")
        .unwrap();
    let ResolvedExprKind::Block {
        tail: borrow_match, ..
    } = &borrow_function.body.kind
    else {
        panic!("mode fixture body is a block")
    };
    let borrow_graph = expr_json(&borrow, borrow_match).unwrap();
    assert!(borrow_graph.contains("\"kind\":\"match\",\"ownership_mode\":\"borrow\""));
    assert_ne!(own_graph, borrow_graph);

    let filters = std::collections::BTreeSet::from([AgentContextFilter::Ownership]);
    let agent = agent_function_json(&borrow, borrow_function, &filters).unwrap();
    assert!(agent.contains("\"ownership_mode\":\"borrow\""));
    assert!(!agent.contains("\"ownership_mode\":\"own\""));
}

#[test]
fn internal_hir_renderer_revalidates_before_serializing() {
    let mut program = resolved_program();
    program.entrypoint = hir::DeclarationId::new("missing.entrypoint");
    assert_eq!(
        to_hir_json(&program, "trusted-source-revision")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}

#[test]
fn internal_hir_renderer_rejects_nul_identity_before_serializing() {
    let mut program = resolved_program();
    program.functions[0].body.ty = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("type\0forged"),
        arguments: Vec::new(),
    };
    let diagnostic = to_hir_json(&program, "trusted-source-revision").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("contains NUL"));
}

#[test]
fn internal_hir_renderer_rejects_nul_cleanup_reference_before_serializing() {
    let mut program = resolved_resource_program();
    let discard = program
        .functions
        .iter_mut()
        .find(|function| function.name == "discard")
        .unwrap();
    let finalizer = discard
        .cleanup_plan
        .exits
        .iter_mut()
        .find_map(|exit| exit.finalize_in_order.first_mut())
        .expect("discard must finalize its parameter");
    finalizer.lifecycle_id = hir::DeclarationId::new("token.drop\0forged");

    let diagnostic = to_hir_json(&program, "trusted-source-revision").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("contains NUL"));
}

#[test]
fn internal_hir_renderer_preserves_its_trusted_source_revision() {
    let graph = to_hir_json(&resolved_program(), "trusted-source-revision").unwrap();
    assert!(graph.contains("\"revision\":\"trusted-source-revision\""));
}

#[test]
fn loan_bearing_generic_instance_defensively_selects_v23() {
    let source = r#"
module test.graph_instance_loan;
@id("generic.marker") fn marker<T>() -> bool { true }
@id("app.main") fn main() -> i64 { if marker<bool>() { 1 } else { 0 } }
"#;
    let mut program = hir::resolve(&parse(source, Path::new("graph-instance-loan.spx")).unwrap())
        .expect("generic instance resolves");
    assert_eq!(program.function_instances.len(), 1);
    assert_ne!(graph_schema(&program), "semaprax.graph.v23");

    // Generic source admission is intentionally scalar-only today, so an
    // authenticated instance cannot yet acquire a real loan. Exercise the
    // schema selector defensively without invoking the validating renderer:
    // if that boundary widens later, an instance attachment must never be
    // silently serialized under an older schema.
    let function = &mut program.function_instances[0].function;
    let site = function.body.id.clone();
    function.loan_plan.schema = crate::loan_plan::LOAN_PLAN_SCHEMA_V1;
    function.loan_plan.loans.push(crate::loan_plan::Loan {
        id: crate::loan_plan::LoanId(0),
        site: site.clone(),
        origin: hir::Place {
            root: function.result_id.clone(),
            projections: Vec::new(),
        },
        parent: None,
        start: crate::loan_plan::LoanProgramPoint {
            expression: site.clone(),
            phase: crate::loan_plan::LoanPointPhase::Before,
        },
        ends: vec![crate::loan_plan::LoanProgramPoint {
            expression: site,
            phase: crate::loan_plan::LoanPointPhase::After,
        }],
        end_edges: vec![0],
        cause: crate::loan_plan::LoanCause::SliceView,
    });
    assert_eq!(graph_schema(&program), "semaprax.graph.v23");
}

#[test]
fn dynamic_byte_ranges_select_v20_and_publish_exact_contract() {
    let source = r#"
module test.graph_byte_range;
@id("range.length")
fn range_length(input: borrow Slice<u8>) -> usize {
let outer = byte_range(input, 1usize, 4usize);
let inner = byte_range(outer, 0usize, 2usize);
byte_len(inner)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(&parse(source, Path::new("graph-byte-range.spx")).unwrap())
        .expect("range program resolves");
    assert_eq!(graph_schema(&program), "semaprax.graph.v20");
    let graph = to_hir_json(&program, "trusted-source-revision").unwrap();
    assert!(graph.contains("\"kind\":\"byte_range\""));
    assert!(graph.contains("\"status_domain\":\"semaprax.byte-range.v1\""));
    assert!(graph.contains("\"ranges\":[{"));
    assert!(!graph.contains("\"bounded_line_command_io\""));
}

#[test]
fn graph_v20_publishes_exact_line_command_append_contract_and_v19_does_not() {
    let v19_source = r#"
module test.graph_command_v19;
permit { process.args.read }
@id("command.count")
fn count() -> usize uses { process.args.read } { args_len() }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let v19 = hir::resolve(&parse(v19_source, Path::new("graph-command-v19.spx")).unwrap())
        .expect("legacy command program resolves");
    assert_eq!(graph_schema(&v19), "semaprax.graph.v19");
    let v19_graph = to_hir_json(&v19, "trusted-source-revision").unwrap();
    assert!(v19_graph.contains("\"bounded_language_command_io\""));
    assert!(!v19_graph.contains("\"bounded_line_command_io\""));
    assert!(!v19_graph.contains("core.host.stdout-append"));
    assert!(!v19_graph.contains("semaprax.command-output.v1"));
    assert!(!v19_graph.contains("__spx_command_output_status_v1"));

    let v20_source = r#"
module test.graph_line_command_v20;
permit { process.stderr.write, process.stdout.write }
@id("command.append")
fn append(value: borrow Slice<u8>) -> usize
uses { process.stderr.write, process.stdout.write }
{
let selected = byte_range(value, 0usize, byte_len(value));
let stdout = stdout_append(selected);
stdout + stderr_append(selected)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let v20 = hir::resolve(&parse(v20_source, Path::new("graph-line-command-v20.spx")).unwrap())
        .expect("line command program resolves");
    assert_eq!(graph_schema(&v20), "semaprax.graph.v20");
    let v20_graph = to_hir_json(&v20, "trusted-source-revision").unwrap();
    let exact = "\"bounded_line_command_io\":{\"profile\":\"line-command-io.v1\",\"operations\":[{\"name\":\"stdout_append\",\"id\":\"core.host.stdout-append\",\"effect\":\"process.stdout.write\",\"return\":\"usize\",\"failure\":\"status\"},{\"name\":\"stderr_append\",\"id\":\"core.host.stderr-append\",\"effect\":\"process.stderr.write\",\"return\":\"usize\",\"failure\":\"status\"}],\"status_domain\":\"semaprax.command-output.v1\",\"status_codes\":{\"output_capacity_exceeded\":1},\"status_marker\":\"__spx_command_output_status_v1\",\"write_mode\":\"cumulative-append.v1\",\"max_combined_output_bytes\":65536,\"publication\":\"terminal-success-only\",\"failure\":\"discard-staged-transcripts\"}";
    assert!(v20_graph.contains(exact), "{v20_graph}");
}

#[test]
fn internal_hir_renderer_rejects_forged_byte_range_operation() {
    let source = r#"
module test.graph_forged_byte_range;
@id("range.length")
fn range_length(input: borrow Slice<u8>) -> usize {
let selected = byte_range(input, 0usize, 1usize);
byte_len(selected)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let mut program =
        hir::resolve(&parse(source, Path::new("graph-forged-byte-range.spx")).unwrap())
            .expect("range program resolves");
    let ResolvedExprKind::Block { statements, .. } = &mut program.functions[0].body.kind else {
        panic!("range function body is a block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("first statement binds the range");
    };
    let ResolvedExprKind::ByteRange { operation, .. } = &mut value.kind else {
        panic!("initializer is an explicit byte range");
    };
    *operation = DeclarationId::new("foreign.byte.range");
    assert_eq!(
        to_hir_json(&program, "trusted-source-revision")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}

#[test]
fn internal_hir_renderer_rejects_a_foreign_record_field_reference() {
    let mut program = resolved_record_program();
    let ResolvedExprKind::Block { tail, .. } = &mut program.functions[0].body.kind else {
        panic!("function body should be a block");
    };
    let ResolvedExprKind::Project { base, .. } = &mut tail.kind else {
        panic!("function tail should be a temporary projection");
    };
    let ResolvedExprKind::ConstructRecord { fields, .. } = &mut base.kind else {
        panic!("projection base should be a record constructor");
    };
    fields[0].field = DeclarationId::new("foreign.field");

    assert_eq!(
        to_hir_json(&program, "trusted-source-revision")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}
