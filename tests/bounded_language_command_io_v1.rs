use std::path::Path;

use semaprax::cleanup_plan::{
    execute_for_conformance, CleanupScenario, CleanupTransition, StatusProducer,
};
use semaprax::conformance::{
    NormalizedStatus, OperationOutcome, Retryability, StatusClass, TraceEventKind, TraceOutcome,
    TraceResult,
};
use semaprax::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedHostCommandOperation,
};
use semaprax::{graph, parse, verify};

const SOURCE: &str = r#"
module test.language_command_io;

permit {
    process.args.read,
    process.stderr.write,
    process.stdin.read,
    process.stdout.write
}

@id("command.probe")
fn probe() -> usize uses {
    process.args.read,
    process.stderr.write,
    process.stdin.read
} {
    let count = args_len();
    let argument = arg_utf8(0usize);
    let argument_bytes = str_as_bytes(argument);
    let input = stdin_read();
    let input_bytes = bytes_as_slice(input);
    let written = stderr_write(argument_bytes);
    count + written + byte_len(input_bytes)
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved() -> hir::ResolvedProgram {
    let ast = parse(SOURCE, Path::new("bounded-language-command-io.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

fn command_expression_mut(
    program: &mut hir::ResolvedProgram,
    operation: ResolvedHostCommandOperation,
) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|item| item.id.as_str() == "command.probe")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("probe body is a block");
    };
    statements
        .iter_mut()
        .find_map(|statement| {
            let semaprax::hir::ResolvedStatement::Let { value, .. } = statement else {
                return None;
            };
            matches!(
                &value.kind,
                ResolvedExprKind::HostCommandCall(call) if call.operation == operation
            )
            .then_some(value)
        })
        .unwrap()
}

fn collect_operations(
    expression: &ResolvedExpr,
    operations: &mut Vec<ResolvedHostCommandOperation>,
) {
    if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
        operations.push(call.operation);
    }
    match &expression.kind {
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_operations(argument, operations);
            }
        }
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_operations(argument, operations);
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collect_operations(child, operations);
                    }
                }
            }
            collect_operations(tail, operations);
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => collect_operations(value, operations),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_operations(left, operations);
            collect_operations(right, operations);
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_operations(source, operations);
            collect_operations(start, operations);
            collect_operations(end, operations);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_operations(condition, operations);
            collect_operations(then_branch, operations);
            collect_operations(else_branch, operations);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_operations(&field.value, operations);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_operations(scrutinee, operations);
            for arm in arms {
                collect_operations(&arm.value, operations);
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_operations(base, operations);
            for field in fields {
                collect_operations(&field.value, operations);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_operations(argument, operations);
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

#[test]
fn hir_cleanup_and_graph_authenticate_the_closed_command_io_contract() {
    let program = resolved();
    hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|item| item.id.as_str() == "command.probe")
        .unwrap();
    let mut operations = Vec::new();
    collect_operations(&function.body, &mut operations);
    assert_eq!(
        operations,
        [
            ResolvedHostCommandOperation::ArgsLen,
            ResolvedHostCommandOperation::ArgUtf8,
            ResolvedHostCommandOperation::StdinRead,
            ResolvedHostCommandOperation::StderrWrite,
        ]
    );

    let statuses = function
        .cleanup_plan
        .status_sources
        .iter()
        .filter(|source| matches!(source.producer, StatusProducer::PropagatedCall { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        statuses.len(),
        2,
        "only arg_utf8 and stdin_read are fallible"
    );
    let status_text = format!("{statuses:?}");
    assert!(status_text.contains("core.host.arg-utf8"));
    assert!(status_text.contains("core.host.stdin-read"));
    assert!(!status_text.contains("core.host.args-len"));
    assert!(!status_text.contains("core.host.stderr-write"));

    let ast = parse(SOURCE, "bounded-language-command-io-graph.spx").unwrap();
    let json = graph::to_json(&ast).unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v19\""));
    assert!(json.contains("\"status_domain\":\"semaprax.command-input.v1\""));
    assert!(json.contains("\"root_kind\":\"command_arguments\""));
    assert!(json.contains("\"max_arguments\":16"));
    assert!(json.contains("\"max_input_bytes\":65536"));
    assert!(json.contains("\"bytes_copy_sites\":0,\"stdin_read_sites\":1"));
}

#[test]
fn effects_contracts_loops_and_authored_aliases_fail_closed() {
    let missing = SOURCE.replace("    process.args.read,\n", "");
    let ast = parse(&missing, "missing-command-effect.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-E102"));

    let contract = r#"
module test.command_contract;
permit { process.args.read }
@id("app.main")
fn main() -> i64 uses { process.args.read }
    requires args_len() == 0usize
{ 0 }
"#;
    let ast = parse(contract, "contract-command-io.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T270"));

    let alias = SOURCE.replace("fn probe()", "fn args_len()");
    let ast = parse(&alias, "command-name-alias.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-S113"));
}

#[test]
fn hostile_hir_operation_ownership_and_identity_mutations_fail_closed() {
    let mut wrong_operation = resolved();
    let expression = command_expression_mut(
        &mut wrong_operation,
        ResolvedHostCommandOperation::StdinRead,
    );
    let ResolvedExprKind::HostCommandCall(call) = &mut expression.kind else {
        unreachable!()
    };
    call.operation = ResolvedHostCommandOperation::ArgsLen;
    assert_eq!(
        hir::validate(&wrong_operation).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_ownership = resolved();
    command_expression_mut(&mut wrong_ownership, ResolvedHostCommandOperation::ArgUtf8).ownership =
        OwnershipMode::Value;
    assert_eq!(
        hir::validate(&wrong_ownership).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_identity = resolved();
    let replacement = {
        let function = wrong_identity
            .functions
            .iter()
            .find(|item| item.id.as_str() == "command.probe")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
            unreachable!()
        };
        let semaprax::hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
            unreachable!()
        };
        value.id.clone()
    };
    let expression =
        command_expression_mut(&mut wrong_identity, ResolvedHostCommandOperation::ArgUtf8);
    let ResolvedExprKind::HostCommandCall(call) = &mut expression.kind else {
        unreachable!()
    };
    call.expression = replacement;
    assert_eq!(hir::validate(&wrong_identity).unwrap_err().code, "SPX-H006");
}

#[test]
fn hostile_cleanup_status_and_stdin_initialization_mutations_fail_replay() {
    let mut wrong_status = resolved();
    let function = wrong_status
        .functions
        .iter_mut()
        .find(|item| item.id.as_str() == "command.probe")
        .unwrap();
    let source = function
        .cleanup_plan
        .status_sources
        .iter_mut()
        .find(|source| {
            matches!(
                &source.producer,
                StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == "core.host.stdin-read"
            )
        })
        .unwrap();
    let StatusProducer::PropagatedCall { callee } = &mut source.producer else {
        unreachable!()
    };
    *callee = hir::DeclarationId::new("core.host.args-len");
    assert_eq!(hir::validate(&wrong_status).unwrap_err().code, "SPX-H006");

    let mut missing_initialize = resolved();
    let stdin_id = command_expression_mut(
        &mut missing_initialize,
        ResolvedHostCommandOperation::StdinRead,
    )
    .id
    .clone();
    let function = missing_initialize
        .functions
        .iter_mut()
        .find(|item| item.id.as_str() == "command.probe")
        .unwrap();
    let before = function
        .cleanup_plan
        .blocks
        .iter()
        .map(|block| block.transitions.len())
        .sum::<usize>();
    for block in &mut function.cleanup_plan.blocks {
        block.transitions.retain(|transition| {
            !matches!(transition, CleanupTransition::Initialize { at, .. } if at == &stdin_id)
        });
    }
    let after = function
        .cleanup_plan
        .blocks
        .iter()
        .map(|block| block.transitions.len())
        .sum::<usize>();
    assert_eq!(
        before,
        after + 1,
        "fixture removes exactly stdin success init"
    );
    assert_eq!(
        hir::validate(&missing_initialize).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn cleanup_executor_commits_all_command_ops_and_settles_stdin_exactly() {
    let source = r#"
module test.command_cleanup_execution;
permit { process.args.read, process.stderr.write, process.stdin.read }
@id("command.run")
fn run() -> bool uses { process.args.read, process.stderr.write, process.stdin.read } {
    let count = args_len();
    let argument = arg_utf8(0usize);
    let input = stdin_read();
    let input_view = bytes_as_slice(input);
    let written = stderr_write(input_view);
    count == written
}

@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(source, "command-cleanup-execution.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "command.run")
        .unwrap();
    let sources = function
        .cleanup_plan
        .status_sources
        .iter()
        .filter_map(|source| match &source.producer {
            StatusProducer::PropagatedCall { callee }
                if callee.as_str() == "core.host.arg-utf8"
                    || callee.as_str() == "core.host.stdin-read" =>
            {
                Some((callee.as_str(), source.id.clone()))
            }
            _ => None,
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let arg_source = sources["core.host.arg-utf8"].clone();
    let stdin_source = sources["core.host.stdin-read"].clone();
    let stdin_initializations = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|transition| {
            matches!(
                transition,
                CleanupTransition::Initialize { at, .. } if *at == stdin_source.expression
            )
        })
        .count();
    assert_eq!(stdin_initializations, 1);

    let mut success = CleanupScenario::new("command-success", Some(TraceResult::Bool(true)));
    success
        .operations
        .insert(arg_source.clone(), OperationOutcome::Success);
    success
        .operations
        .insert(stdin_source.clone(), OperationOutcome::Success);
    let trace = execute_for_conformance(&program, &function.id, success).unwrap();
    assert_eq!(
        trace
            .events
            .iter()
            .filter_map(|event| match &event.event {
                TraceEventKind::CallCommit { callee, .. }
                    if callee.as_str().starts_with("core.host.") =>
                {
                    Some(callee.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            "core.host.args-len",
            "core.host.arg-utf8",
            "core.host.stdin-read",
            "core.host.stderr-write",
        ]
    );
    assert_eq!(
        trace.outcome,
        TraceOutcome::Success {
            result: TraceResult::Bool(true)
        }
    );
    assert_eq!(
        trace
            .events
            .iter()
            .filter(|event| matches!(
                &event.event,
                TraceEventKind::FinalizeEnd { lifecycle_id, .. }
                    if lifecycle_id.as_str() == "core.bytes.drop"
            ))
            .count(),
        1,
        "the one successful stdin-owned value is finalized exactly once"
    );

    for code in [1, 2] {
        let status = NormalizedStatus::try_new(
            "semaprax.command-input.v1",
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .unwrap();
        let mut failure = CleanupScenario::new(format!("arg-failure-{code}"), None);
        failure.operations.insert(
            arg_source.clone(),
            OperationOutcome::Failure(status.clone()),
        );
        let trace = execute_for_conformance(&program, &function.id, failure).unwrap();
        assert_eq!(
            trace.outcome,
            TraceOutcome::Failure {
                selected_source: arg_source.clone(),
                status,
            }
        );
        assert!(!trace.events.iter().any(|event| matches!(
            &event.event,
            TraceEventKind::CallCommit { callee, .. }
                if callee.as_str() == "core.host.stdin-read"
        )));
    }

    for code in [3, 4] {
        let status = NormalizedStatus::try_new(
            "semaprax.command-input.v1",
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .unwrap();
        let mut failure = CleanupScenario::new(format!("stdin-failure-{code}"), None);
        failure
            .operations
            .insert(arg_source.clone(), OperationOutcome::Success);
        failure.operations.insert(
            stdin_source.clone(),
            OperationOutcome::Failure(status.clone()),
        );
        let trace = execute_for_conformance(&program, &function.id, failure).unwrap();
        assert_eq!(
            trace.outcome,
            TraceOutcome::Failure {
                selected_source: stdin_source.clone(),
                status,
            }
        );
        assert!(!trace.events.iter().any(|event| matches!(
            &event.event,
            TraceEventKind::Initialize { at, .. } if *at == stdin_source.expression
        )));
    }
}

#[test]
fn cleanup_executor_rejects_hostile_command_status_shapes_exactly() {
    let source = r#"
module test.hostile_command_status;
permit { process.args.read, process.stdin.read }
@id("command.run")
fn run() -> bool uses { process.args.read, process.stdin.read } {
    let argument = arg_utf8(0usize);
    let input = stdin_read();
    byte_len(str_as_bytes(argument)) == byte_len(bytes_as_slice(input))
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(source, "hostile-command-status.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "command.run")
        .unwrap();
    let sources = function
        .cleanup_plan
        .status_sources
        .iter()
        .filter_map(|source| match &source.producer {
            StatusProducer::PropagatedCall { callee }
                if matches!(
                    callee.as_str(),
                    "core.host.arg-utf8" | "core.host.stdin-read"
                ) =>
            {
                Some((callee.as_str(), source.id.clone()))
            }
            _ => None,
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let arg = sources["core.host.arg-utf8"].clone();
    let stdin = sources["core.host.stdin-read"].clone();
    for (operation, source, valid_code, other_code) in
        [("arg", &arg, 1, 3), ("stdin", &stdin, 3, 2)]
    {
        for (name, domain, code, class, retryability) in [
            (
                "foreign-domain",
                "foreign.command.v1",
                valid_code,
                StatusClass::Adapter,
                Retryability::Known(false),
            ),
            (
                "wrong-code",
                "semaprax.command-input.v1",
                other_code,
                StatusClass::Adapter,
                Retryability::Known(false),
            ),
            (
                "wrong-class",
                "semaprax.command-input.v1",
                valid_code,
                StatusClass::Import,
                Retryability::Known(false),
            ),
            (
                "retryable",
                "semaprax.command-input.v1",
                valid_code,
                StatusClass::Adapter,
                Retryability::Known(true),
            ),
            (
                "unknown-retry",
                "semaprax.command-input.v1",
                valid_code,
                StatusClass::Adapter,
                Retryability::Unknown,
            ),
        ] {
            let status = NormalizedStatus::try_new(domain, code, class, retryability).unwrap();
            let mut scenario = CleanupScenario::new(format!("hostile-{operation}-{name}"), None);
            if operation == "stdin" {
                scenario
                    .operations
                    .insert(arg.clone(), OperationOutcome::Success);
            }
            scenario
                .operations
                .insert(source.clone(), OperationOutcome::Failure(status));
            let error = execute_for_conformance(&program, &function.id, scenario).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("outside its exact normalized failure domain"),
                "{operation}/{name}: {error}"
            );
        }
    }
}

#[test]
fn stdout_only_graph_remains_v18_and_has_no_v19_facts() {
    let source = r#"
module test.legacy_stdout;
permit { process.stdout.write }
@id("app.main")
fn main() -> i64 uses { process.stdout.write } {
    let data = [65u8];
    let view = array_as_slice(data);
    let written = stdout_write(view);
    if written == 1usize { 1 } else { 0 }
}
"#;
    let first = graph::to_json(&parse(source, "legacy-v18-a.spx").unwrap()).unwrap();
    let second = graph::to_json(&parse(source, "legacy-v18-b.spx").unwrap()).unwrap();
    assert_eq!(first, second, "legacy Graph bytes remain deterministic");
    assert!(first.starts_with("{\"schema\":\"semaprax.graph.v18\""));
    assert!(first.contains("\"bounded_stdout_transcript\""));
    assert!(!first.contains("bounded_language_command_io"));
    assert!(!first.contains("host_command_call"));
    assert!(!first.contains("command_arguments"));
}

#[test]
fn stderr_write_capacity_is_path_sensitive_and_cycle_closed() {
    let sequential = SOURCE.replace(
        "let written = stderr_write(argument_bytes);",
        "let first = stderr_write(argument_bytes);\n    let written = stderr_write(argument_bytes);",
    );
    let ast = parse(&sequential, "two-stderr-writes.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T269"));

    let alternatives = SOURCE.replace(
        "let written = stderr_write(argument_bytes);",
        "let written = if count == 0usize { stderr_write(argument_bytes) } else { stderr_write(argument_bytes) };",
    );
    let ast = parse(&alternatives, "alternative-stderr-writes.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let looped = SOURCE.replace(
        "let written = stderr_write(argument_bytes);",
        "let mut written = 0usize;\n    while written == 0usize { written = stderr_write(argument_bytes); false }",
    );
    let ast = parse(&looped, "looped-stderr-write.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| matches!(item.code, "SPX-T269" | "SPX-T270")));

    let cyclic = r#"
module test.cyclic_stderr;
permit { process.stderr.write }
@id("cycle.a")
fn a(view: borrow Slice<u8>) -> usize uses { process.stderr.write } {
    let written = stderr_write(view);
    written + b(view)
}
@id("cycle.b")
fn b(view: borrow Slice<u8>) -> usize uses { process.stderr.write } { a(view) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(cyclic, "cyclic-stderr-write.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(
        diagnostics.iter().any(|item| item.code == "SPX-T269"),
        "{diagnostics:?}"
    );
}

#[test]
fn stdin_read_has_its_own_path_counter_across_branches_loops_and_calls() {
    let sequential = SOURCE.replace(
        "let input = stdin_read();",
        "let first_input = stdin_read();\n    let input = stdin_read();",
    );
    let ast = parse(&sequential, "two-stdin-reads.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T267"));

    let cyclic = r#"
module test.stdin_cycle;
permit { process.stdin.read }
@id("stdin.cycle.a")
fn a() -> usize uses { process.stdin.read } {
    let input = stdin_read();
    let view = bytes_as_slice(input);
    byte_len(view) + b()
}
@id("stdin.cycle.b")
fn b() -> usize uses { process.stdin.read } { a() }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(cyclic, "stdin-cycle.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T267"));

    let alternatives = SOURCE.replace(
        "let input = stdin_read();",
        "let input = if count == 0usize { stdin_read() } else { stdin_read() };",
    );
    let ast = parse(&alternatives, "alternative-stdin-reads.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();

    let looped = SOURCE.replace(
        "let input = stdin_read();",
        "let mut input = stdin_read();\n    while count == 0usize { input = stdin_read(); false }",
    );
    let ast = parse(&looped, "looped-stdin-read.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| matches!(item.code, "SPX-T267" | "SPX-T270")));

    let through_call = r#"
module test.stdin_call_path;
permit { process.stdin.read }
@id("stdin.read.one")
fn read_one() -> Bytes uses { process.stdin.read } { stdin_read() }
@id("stdin.read.two")
fn read_two() -> usize uses { process.stdin.read } {
    let first = read_one();
    let second = read_one();
    let first_view = bytes_as_slice(first);
    let second_view = bytes_as_slice(second);
    byte_len(first_view) + byte_len(second_view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(through_call, "stdin-call-path.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T267"));
}

#[test]
fn combined_transcript_bytes_are_path_sensitive_and_source_hir_agree() {
    let invocation_bounded = r#"
module test.invocation_bounded_dual_transcript;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("command.run")
fn run() -> bool uses { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write } {
    if args_len() == 1usize {
        let argument = arg_utf8(0usize);
        let argument_bytes = str_as_bytes(argument);
        let input = stdin_read();
        let input_bytes = bytes_as_slice(input);
        let stderr_count = stderr_write(argument_bytes);
        let stdout_count = stdout_write(input_bytes);
        stderr_count == byte_len(argument_bytes) && stdout_count == byte_len(input_bytes)
    } else { false }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(invocation_bounded, "invocation-bounded-transcript.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();

    let bounded = r#"
module test.bounded_dual_transcript;
permit { process.stderr.write, process.stdout.write }
@id("app.main")
fn main() -> i64 uses { process.stderr.write, process.stdout.write } {
    let stderr_data = [1u8; 32768];
    let stdout_data = [2u8; 32768];
    let stderr_count = stderr_write(array_as_slice(stderr_data));
    let stdout_count = stdout_write(array_as_slice(stdout_data));
    if stderr_count + stdout_count == 65536usize { 1 } else { 0 }
}
"#;
    let ast = parse(bounded, "bounded-dual-transcript.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();

    let unbounded = r#"
module test.unbounded_dual_transcript;
permit { process.stderr.write, process.stdout.write }
@id("transcript.copy")
fn copy(view: borrow Slice<u8>) -> usize uses { process.stderr.write, process.stdout.write } {
    let stderr_count = stderr_write(view);
    let stdout_count = stdout_write(view);
    stderr_count + stdout_count
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(unbounded, "unbounded-dual-transcript.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(
        diagnostics.iter().any(|item| item.code == "SPX-T269"),
        "{diagnostics:?}"
    );
    let hostile_hir = hir::resolve(&ast).unwrap_err();
    assert!(
        hostile_hir.iter().any(|item| item.code == "SPX-T269"),
        "{hostile_hir:?}"
    );

    for (name, setup, first, second) in [
        (
            "duplicate-stdin",
            "let input = stdin_read();\n    let view = bytes_as_slice(input);",
            "stderr_write(view)",
            "stdout_write(view)",
        ),
        (
            "duplicate-args",
            "let argument = arg_utf8(0usize);\n    let view = str_as_bytes(argument);",
            "stderr_write(view)",
            "stdout_write(view)",
        ),
    ] {
        let module = name.replace('-', "_");
        let source = format!(
            r#"
module test.{module};
permit {{ process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }}
@id("command.run")
fn run() -> bool uses {{ process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }} {{
    {setup}
    let first = {first};
    let second = {second};
    first == second
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let ast = parse(&source, format!("{name}.spx")).unwrap();
        assert!(verify::verify(&ast)
            .iter()
            .any(|item| item.code == "SPX-T269"));
    }

    let fixed_plus_unbounded = r#"
module test.fixed_plus_unbounded;
permit { process.stderr.write, process.stdout.write }
@id("transcript.mixed")
fn mixed(view: borrow Slice<u8>) -> usize uses { process.stderr.write, process.stdout.write } {
    let fixed = [1u8];
    let first = stderr_write(view);
    let second = stdout_write(array_as_slice(fixed));
    first + second
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(fixed_plus_unbounded, "fixed-plus-unbounded.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T269"));

    let through_calls = r#"
module test.called_dual_transcript;
permit { process.stderr.write, process.stdout.write }
@id("transcript.stderr")
fn err(view: borrow Slice<u8>) -> usize uses { process.stderr.write } { stderr_write(view) }
@id("transcript.stdout")
fn out(view: borrow Slice<u8>) -> usize uses { process.stdout.write } { stdout_write(view) }
@id("transcript.both")
fn both(view: borrow Slice<u8>) -> usize uses { process.stderr.write, process.stdout.write } {
    err(view) + out(view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(through_calls, "called-dual-transcript.spx").unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-T269"));

    let alternatives = r#"
module test.alternative_dual_transcript;
permit { process.stderr.write, process.stdout.write }
@id("transcript.one")
fn one(view: borrow Slice<u8>, choose_stdout: bool) -> usize uses { process.stderr.write, process.stdout.write } {
    if choose_stdout { stdout_write(view) } else { stderr_write(view) }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(alternatives, "alternative-dual-transcript.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
}

#[test]
fn transcript_provenance_is_lexically_scoped_across_sibling_branches() {
    let source = r#"
module test.lexical_transcript_roots;
permit { process.args.read, process.stdin.read, process.stdout.write }
@id("command.run")
fn run() -> bool uses { process.args.read, process.stdin.read, process.stdout.write } {
    if args_len() == 0usize {
        let payload = stdin_read();
        let view = bytes_as_slice(payload);
        stdout_write(view) == byte_len(view)
    } else {
        let payload = arg_utf8(0usize);
        let view = str_as_bytes(payload);
        stdout_write(view) == byte_len(view)
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(source, "lexical-transcript-roots.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();

    let duplicate = source.replace(
        "let view = bytes_as_slice(payload);",
        "let view = bytes_as_slice(payload);\n        let view = bytes_as_slice(payload);",
    );
    let ast = parse(&duplicate, "duplicate-lexical-transcript-root.spx").unwrap();
    assert!(
        verify::verify(&ast)
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T209"),
        "same-scope duplicates must remain rejected"
    );
}
