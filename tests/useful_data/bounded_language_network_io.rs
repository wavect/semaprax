//! Bounded Language Network I/O v1: the core semantics between the resolver
//! and the backends. A canonical network module is admitted, checked, planned,
//! and projected; every hostile shape fails closed with its stable diagnostic.

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
use semaprax::{format, graph, parse, verify};

const NETWORK_IDS: [&str; 6] = [
    "core.host.net-connect",
    "core.host.net-send",
    "core.host.net-recv",
    "core.host.net-stream-stdout",
    "core.host.net-wait",
    "core.host.net-close",
];

/// Canonical formatting: `format::canonical` must reproduce these bytes.
const SOURCE: &str = r#"module test.language_network_io;

permit { network.connect, network.read, network.write, process.stdout.write }

@id("net.stream")
fn stream() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
{
    let host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let handle = net_connect(array_as_slice(host), 8080usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let mut streamed = 1usize;
    while streamed > 0usize {
        let ready = net_wait(handle, 1000usize);
        streamed = net_stream_stdout(handle, 4096usize);
        ready != 0usize && streamed > 0usize
    }
    net_close(handle) == 0usize && sent == 5usize
}

@id("net.receive")
fn receive(handle: usize) -> usize
    uses { network.read }
{
    let chunk = net_recv(handle, 65536usize);
    let view = bytes_as_slice(chunk);
    byte_len(view)
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

/// Straight-line use of every operation, for cleanup execution traces.
const STRAIGHT_SOURCE: &str = r#"
module test.network_cleanup_execution;
permit { network.connect, network.read, network.write, process.stdout.write }
@id("net.run")
fn run() -> bool uses { network.connect, network.read, network.write, process.stdout.write } {
    let host = [49u8, 50u8, 55u8, 46u8, 48u8, 46u8, 48u8, 46u8, 49u8];
    let handle = net_connect(array_as_slice(host), 8080usize);
    let request = [80u8, 73u8, 78u8, 71u8, 10u8];
    let sent = net_send(handle, array_as_slice(request));
    let chunk = net_recv(handle, 4096usize);
    let streamed = net_stream_stdout(handle, 4096usize);
    let ready = net_wait(handle, 10usize);
    let closed = net_close(handle);
    let view = bytes_as_slice(chunk);
    closed == byte_len(view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn verified(source: &str, name: &str) -> semaprax::ast::Program {
    let ast = parse(source, Path::new(name)).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
    ast
}

fn resolved(source: &str, name: &str) -> hir::ResolvedProgram {
    hir::resolve(&verified(source, name)).unwrap()
}

fn codes(source: &str, name: &str) -> Vec<String> {
    let ast = parse(source, Path::new(name)).unwrap();
    verify::verify(&ast)
        .into_iter()
        .map(|item| item.code.to_owned())
        .collect()
}

fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|item| item.id.as_str() == id)
        .unwrap()
}

fn function_mut<'a>(
    program: &'a mut hir::ResolvedProgram,
    id: &str,
) -> &'a mut hir::ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|item| item.id.as_str() == id)
        .unwrap()
}

/// The `let` initializer in `id` whose host-command operation is `operation`.
fn command_expression_mut<'a>(
    program: &'a mut hir::ResolvedProgram,
    id: &str,
    operation: ResolvedHostCommandOperation,
) -> &'a mut ResolvedExpr {
    let ResolvedExprKind::Block { statements, .. } = &mut function_mut(program, id).body.kind
    else {
        panic!("function body is a block");
    };
    statements
        .iter_mut()
        .find_map(|statement| {
            let hir::ResolvedStatement::Let { value, .. } = statement else {
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

/// The in-loop `let` initializer of `net.stream` that calls `net_wait`.
fn loop_wait_call_mut(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedHostCommandCall {
    let ResolvedExprKind::Block { statements, .. } =
        &mut function_mut(program, "net.stream").body.kind
    else {
        panic!("function body is a block");
    };
    let loop_body = statements
        .iter_mut()
        .find_map(|statement| match statement {
            hir::ResolvedStatement::While { body, .. } => Some(body),
            _ => None,
        })
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut loop_body.kind else {
        panic!("loop body is a block");
    };
    statements
        .iter_mut()
        .find_map(|statement| {
            let hir::ResolvedStatement::Let { value, .. } = statement else {
                return None;
            };
            match &mut value.kind {
                ResolvedExprKind::HostCommandCall(call)
                    if call.operation == ResolvedHostCommandOperation::NetWait =>
                {
                    Some(call)
                }
                _ => None,
            }
        })
        .unwrap()
}

fn network_status(code: u32) -> NormalizedStatus {
    NormalizedStatus::try_new(
        "semaprax.network.v1",
        code,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .unwrap()
}

#[test]
fn canonical_network_module_is_admitted_planned_and_projected() {
    let ast = verified(SOURCE, "bounded-language-network-io.spx");
    assert_eq!(format::canonical(&ast), SOURCE, "source is canonical");

    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).expect("cleanup plan and loan plan replay");

    let mut operations = Vec::new();
    collect_operations(&function(&program, "net.stream").body, &mut operations);
    assert_eq!(
        operations,
        [
            ResolvedHostCommandOperation::NetConnect,
            ResolvedHostCommandOperation::NetSend,
            ResolvedHostCommandOperation::NetWait,
            ResolvedHostCommandOperation::NetStreamStdout,
            ResolvedHostCommandOperation::NetClose,
        ]
    );
    let mut operations = Vec::new();
    collect_operations(&function(&program, "net.receive").body, &mut operations);
    assert_eq!(operations, [ResolvedHostCommandOperation::NetRecv]);

    // Every network operation is fallible: each carries one status source
    // naming its reserved identity, and only `net_recv` owns its result.
    let stream = function(&program, "net.stream");
    let status_callees = stream
        .cleanup_plan
        .status_sources
        .iter()
        .filter_map(|source| match &source.producer {
            StatusProducer::PropagatedCall { callee } => Some(callee.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        status_callees,
        [
            "core.host.net-connect",
            "core.host.net-send",
            "core.host.net-wait",
            "core.host.net-stream-stdout",
            "core.host.net-close",
        ]
    );
    let receive = function(&program, "net.receive");
    let recv = command_expression_mut(
        &mut program.clone(),
        "net.receive",
        ResolvedHostCommandOperation::NetRecv,
    )
    .id
    .clone();
    assert_eq!(
        receive
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| matches!(
                transition,
                CleanupTransition::Initialize { at, .. } if *at == recv
            ))
            .count(),
        1,
        "net_recv initializes its owned slot exactly once, like stdin_read"
    );
    for transition in stream
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
    {
        assert!(
            !matches!(transition, CleanupTransition::Initialize { .. }),
            "scalar network results never initialize an owned slot: {transition:?}"
        );
    }

    let json = graph::to_json(&ast).unwrap();
    for id in NETWORK_IDS {
        assert!(
            json.contains(&format!(
                "\"kind\":\"host_command_call\",\"operation\":\"{id}\""
            )),
            "{id} is projected as a host command call"
        );
    }
    assert!(json.contains(
        "\"function\":\"net.receive\",\"inline_array_frame_bytes\":0,\"active_array_call_path_bytes\":0,\"bytes_copy_sites\":1,\"stdin_read_sites\":0,\"owned_byte_payload_bytes\":65536"
    ));
    assert_eq!(
        json,
        graph::to_json(&parse(SOURCE, "bounded-language-network-io-b.spx").unwrap()).unwrap(),
        "graph bytes are deterministic"
    );
}

#[test]
fn effect_permit_loop_shape_contract_and_identity_diagnostics_are_stable() {
    let missing_connect = SOURCE.replace(
        "uses { network.connect, network.read, network.write, process.stdout.write }",
        "uses { network.read, network.write, process.stdout.write }",
    );
    assert!(codes(&missing_connect, "missing-connect-effect.spx").contains(&"SPX-E102".to_owned()));

    let missing_secondary = SOURCE.replace(
        "uses { network.connect, network.read, network.write, process.stdout.write }",
        "uses { network.connect, network.read, network.write }",
    );
    let ast = parse(&missing_secondary, "missing-stdout-effect.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(
        diagnostics.iter().any(|item| item.code == "SPX-E102"
            && item.message.contains("net_stream_stdout")
            && item.message.contains("process.stdout.write")),
        "{diagnostics:?}"
    );
    let mut hostile_effects = resolved(SOURCE, "hostile-secondary-effect.spx");
    function_mut(&mut hostile_effects, "net.stream")
        .effects
        .retain(|effect| effect != "process.stdout.write");
    let error = hir::validate(&hostile_effects).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("process.stdout.write"), "{error:?}");

    let missing_permit = SOURCE.replace(
        "permit { network.connect, network.read, network.write, process.stdout.write }",
        "permit { network.connect, network.read, process.stdout.write }",
    );
    assert!(codes(&missing_permit, "missing-permit.spx").contains(&"SPX-E101".to_owned()));

    let recv_in_loop = SOURCE.replace(
        "        let ready = net_wait(handle, 1000usize);\n",
        "        let ready = net_wait(handle, 1000usize);\n        let chunk = net_recv(handle, 16usize);\n",
    );
    assert!(codes(&recv_in_loop, "recv-in-loop.spx").contains(&"SPX-T270".to_owned()));

    for (name, from, to) in [
        (
            "swapped-connect-arguments",
            "net_connect(array_as_slice(host), 8080usize)",
            "net_connect(8080usize, array_as_slice(host))",
        ),
        (
            "send-slice-as-handle",
            "net_send(handle, array_as_slice(request))",
            "net_send(array_as_slice(request), handle)",
        ),
        (
            "close-without-handle",
            "net_close(handle) == 0usize",
            "net_close() == 0usize",
        ),
        (
            "wait-missing-timeout",
            "net_wait(handle, 1000usize)",
            "net_wait(handle)",
        ),
        (
            "recv-i64-max",
            "net_recv(handle, 65536usize)",
            "net_recv(handle, 65536)",
        ),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "{name} rewrites the module");
        assert!(
            codes(&source, &format!("{name}.spx")).contains(&"SPX-T270".to_owned()),
            "{name}"
        );
    }

    let contract = r#"
module test.network_contract;
permit { network.connect }
@id("net.guarded")
fn guarded(handle: usize) -> usize uses { network.connect }
    requires net_close(handle) == 0usize
{ handle }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes(contract, "network-contract.spx").contains(&"SPX-T270".to_owned()));
    let ensures = contract.replace(
        "    requires net_close(handle) == 0usize",
        "    ensures net_close(result) == 0usize",
    );
    assert!(codes(&ensures, "network-ensures.spx").contains(&"SPX-T270".to_owned()));

    for id in NETWORK_IDS {
        let reserved = SOURCE.replace("@id(\"net.receive\")", &format!("@id(\"{id}\")"));
        assert!(
            codes(&reserved, "reserved-network-id.spx").contains(&"SPX-S113".to_owned()),
            "{id} is reserved from authored declarations"
        );
    }
    let alias = SOURCE.replace("fn receive(handle: usize)", "fn net_recv(handle: usize)");
    assert!(codes(&alias, "network-name-alias.spx").contains(&"SPX-S113".to_owned()));
}

#[test]
fn while_bodies_admit_scalar_network_operations_on_both_sides() {
    let program = resolved(SOURCE, "loop-admission.spx");
    hir::validate(&program).unwrap();

    // Every Copy-scalar operation, including a send of an authenticated slice
    // alias and a close, is admitted in a loop body.
    let all_scalars = SOURCE
        .replace(
            "    let handle = net_connect(array_as_slice(host), 8080usize);\n",
            "    let host_view = array_as_slice(host);\n    let handle = net_connect(host_view, 8080usize);\n",
        )
        .replace(
            "    let sent = net_send(handle, array_as_slice(request));\n",
            "    let request_view = array_as_slice(request);\n    let sent = net_send(handle, request_view);\n",
        )
        .replace(
            "        let ready = net_wait(handle, 1000usize);\n",
            "        let ready = net_wait(handle, 1000usize);\n        let again = net_send(handle, request_view);\n        let reopened = net_connect(host_view, 8080usize);\n        let shut = net_close(reopened);\n",
        );
    let ast = parse(&all_scalars, "loop-all-scalars.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();

    // Hostile HIR: retarget an admitted loop operation at the owned-result
    // read, or at an operation with a different arity, and validation fails
    // closed like the source verifier would have.
    for hostile in [
        ResolvedHostCommandOperation::NetRecv,
        ResolvedHostCommandOperation::NetClose,
        ResolvedHostCommandOperation::StdinRead,
    ] {
        let mut mutated = resolved(SOURCE, "loop-hostile.spx");
        loop_wait_call_mut(&mut mutated).operation = hostile;
        assert_eq!(
            hir::validate(&mutated).unwrap_err().code,
            "SPX-H006",
            "{hostile:?}"
        );
    }
}

#[test]
fn hostile_hir_operation_ownership_and_status_mutations_fail_closed() {
    // Same arity, different result ownership.
    let mut owned_as_scalar = resolved(STRAIGHT_SOURCE, "hostile-ownership.spx");
    let expression = command_expression_mut(
        &mut owned_as_scalar,
        "net.run",
        ResolvedHostCommandOperation::NetWait,
    );
    let ResolvedExprKind::HostCommandCall(call) = &mut expression.kind else {
        unreachable!()
    };
    call.operation = ResolvedHostCommandOperation::NetRecv;
    assert_eq!(
        hir::validate(&owned_as_scalar).unwrap_err().code,
        "SPX-H006"
    );

    // Different arity.
    let mut wrong_arity = resolved(STRAIGHT_SOURCE, "hostile-arity.spx");
    let expression = command_expression_mut(
        &mut wrong_arity,
        "net.run",
        ResolvedHostCommandOperation::NetClose,
    );
    let ResolvedExprKind::HostCommandCall(call) = &mut expression.kind else {
        unreachable!()
    };
    call.operation = ResolvedHostCommandOperation::NetSend;
    assert_eq!(hir::validate(&wrong_arity).unwrap_err().code, "SPX-H006");

    // Owned result relabelled as a value.
    let mut wrong_ownership = resolved(STRAIGHT_SOURCE, "hostile-recv-ownership.spx");
    command_expression_mut(
        &mut wrong_ownership,
        "net.run",
        ResolvedHostCommandOperation::NetRecv,
    )
    .ownership = OwnershipMode::Value;
    assert_eq!(
        hir::validate(&wrong_ownership).unwrap_err().code,
        "SPX-H006"
    );

    // Status source retargeted at an infallible operation.
    let mut wrong_status = resolved(STRAIGHT_SOURCE, "hostile-status-callee.spx");
    let source = function_mut(&mut wrong_status, "net.run")
        .cleanup_plan
        .status_sources
        .iter_mut()
        .find(|source| {
            matches!(
                &source.producer,
                StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == "core.host.net-recv"
            )
        })
        .unwrap();
    let StatusProducer::PropagatedCall { callee } = &mut source.producer else {
        unreachable!()
    };
    *callee = hir::DeclarationId::new("core.host.args-len");
    assert_eq!(hir::validate(&wrong_status).unwrap_err().code, "SPX-H006");

    // Status source retargeted at a different fallible network operation.
    let mut swapped_status = resolved(STRAIGHT_SOURCE, "hostile-status-swap.spx");
    let source = function_mut(&mut swapped_status, "net.run")
        .cleanup_plan
        .status_sources
        .iter_mut()
        .find(|source| {
            matches!(
                &source.producer,
                StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == "core.host.net-recv"
            )
        })
        .unwrap();
    let StatusProducer::PropagatedCall { callee } = &mut source.producer else {
        unreachable!()
    };
    *callee = hir::DeclarationId::new("core.host.net-connect");
    assert_eq!(hir::validate(&swapped_status).unwrap_err().code, "SPX-H006");

    // Missing owned-slot initialization for the successful read.
    let mut missing_initialize = resolved(STRAIGHT_SOURCE, "hostile-initialize.spx");
    let recv = command_expression_mut(
        &mut missing_initialize,
        "net.run",
        ResolvedHostCommandOperation::NetRecv,
    )
    .id
    .clone();
    let function = function_mut(&mut missing_initialize, "net.run");
    let before = function
        .cleanup_plan
        .blocks
        .iter()
        .map(|block| block.transitions.len())
        .sum::<usize>();
    for block in &mut function.cleanup_plan.blocks {
        block.transitions.retain(|transition| {
            !matches!(transition, CleanupTransition::Initialize { at, .. } if at == &recv)
        });
    }
    let after = function
        .cleanup_plan
        .blocks
        .iter()
        .map(|block| block.transitions.len())
        .sum::<usize>();
    assert_eq!(before, after + 1, "fixture removes exactly the recv init");
    assert_eq!(
        hir::validate(&missing_initialize).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn cleanup_executor_commits_every_network_op_and_settles_recv_exactly() {
    let program = resolved(STRAIGHT_SOURCE, "network-cleanup-execution.spx");
    hir::validate(&program).unwrap();
    let function = function(&program, "net.run");
    let sources = function
        .cleanup_plan
        .status_sources
        .iter()
        .filter_map(|source| match &source.producer {
            StatusProducer::PropagatedCall { callee }
                if callee.as_str().starts_with("core.host.net-") =>
            {
                Some((callee.as_str(), source.id.clone()))
            }
            _ => None,
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(sources.len(), 6, "every network operation is fallible");
    let recv = sources["core.host.net-recv"].clone();

    let mut success = CleanupScenario::new("network-success", Some(TraceResult::Bool(true)));
    for source in sources.values() {
        success
            .operations
            .insert(source.clone(), OperationOutcome::Success);
    }
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
        NETWORK_IDS,
        "operations commit left to right in authored order"
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
                TraceEventKind::Initialize { at, .. } if *at == recv.expression
            ))
            .count(),
        1
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
        "the one received owned value is finalized exactly once"
    );

    // Every admitted code selects the failing source, stops before the next
    // commit, and never initializes the owned slot.
    let order = NETWORK_IDS;
    for (position, id) in order.iter().enumerate() {
        for code in 1..=6 {
            let status = network_status(code);
            let mut failure = CleanupScenario::new(format!("{id}-failure-{code}"), None);
            for earlier in &order[..position] {
                failure
                    .operations
                    .insert(sources[*earlier].clone(), OperationOutcome::Success);
            }
            failure.operations.insert(
                sources[*id].clone(),
                OperationOutcome::Failure(status.clone()),
            );
            let trace = execute_for_conformance(&program, &function.id, failure).unwrap();
            assert_eq!(
                trace.outcome,
                TraceOutcome::Failure {
                    selected_source: sources[*id].clone(),
                    status,
                },
                "{id}/{code}"
            );
            if let Some(next) = order.get(position + 1) {
                assert!(
                    !trace.events.iter().any(|event| matches!(
                        &event.event,
                        TraceEventKind::CallCommit { callee, .. } if callee.as_str() == *next
                    )),
                    "{id}/{code}: failure is sticky"
                );
            }
            if position <= 2 {
                assert!(
                    !trace.events.iter().any(|event| matches!(
                        &event.event,
                        TraceEventKind::Initialize { at, .. } if *at == recv.expression
                    )),
                    "{id}/{code}: a failed or unreached recv never initializes"
                );
            }
        }
    }
}

#[test]
fn cleanup_executor_rejects_hostile_network_status_shapes_exactly() {
    let program = resolved(STRAIGHT_SOURCE, "hostile-network-status.spx");
    let function = function(&program, "net.run");
    let connect = function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|source| {
            matches!(
                &source.producer,
                StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == "core.host.net-connect"
            )
        })
        .map(|source| source.id.clone())
        .unwrap();
    for (name, domain, code, class, retryability) in [
        (
            "foreign-domain",
            "semaprax.command-input.v1",
            1,
            StatusClass::Adapter,
            Retryability::Known(false),
        ),
        (
            "code-seven",
            "semaprax.network.v1",
            7,
            StatusClass::Adapter,
            Retryability::Known(false),
        ),
        (
            "wrong-class",
            "semaprax.network.v1",
            1,
            StatusClass::Import,
            Retryability::Known(false),
        ),
        (
            "retryable",
            "semaprax.network.v1",
            1,
            StatusClass::Adapter,
            Retryability::Known(true),
        ),
        (
            "unknown-retry",
            "semaprax.network.v1",
            1,
            StatusClass::Adapter,
            Retryability::Unknown,
        ),
    ] {
        let status = NormalizedStatus::try_new(domain, code, class, retryability).unwrap();
        let mut scenario = CleanupScenario::new(format!("hostile-network-{name}"), None);
        scenario
            .operations
            .insert(connect.clone(), OperationOutcome::Failure(status));
        let error = execute_for_conformance(&program, &function.id, scenario).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("outside its exact normalized failure domain"),
            "{name}: {error}"
        );
    }
}

#[test]
fn net_recv_is_an_owned_byte_allocation_site_bounded_by_copy_sites() {
    fn module(sites: usize) -> String {
        let reads = (0..sites)
            .map(|index| format!("    let chunk_{index} = net_recv(handle, 65536usize);\n"))
            .collect::<String>();
        let total = (0..sites)
            .map(|index| format!("byte_len(bytes_as_slice(chunk_{index}))"))
            .collect::<Vec<_>>()
            .join(" + ");
        format!(
            r#"
module test.network_recv_sites;
permit {{ network.read }}
@id("net.drain")
fn drain(handle: usize) -> usize uses {{ network.read }} {{
{reads}    {total}
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        )
    }

    // Sixteen reads on one path: exactly the 1 MiB owned-byte payload bound.
    let ast = verified(&module(16), "sixteen-recv-sites.spx");
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let json = graph::to_json(&ast).unwrap();
    assert!(json.contains(
        "\"function\":\"net.drain\",\"inline_array_frame_bytes\":0,\"active_array_call_path_bytes\":0,\"bytes_copy_sites\":16,\"stdin_read_sites\":0,\"owned_byte_payload_bytes\":1048576"
    ));

    // Seventeen reads exceed MAX_BYTES_COPY_SITES; source and HIR agree.
    let seventeen = module(17);
    let ast = parse(&seventeen, "seventeen-recv-sites.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(
        diagnostics.iter().any(|item| item.code == "SPX-T267"),
        "{diagnostics:?}"
    );
    let hostile_hir = hir::resolve(&ast).unwrap_err();
    assert!(
        hostile_hir.iter().any(|item| item.code == "SPX-T267"),
        "{hostile_hir:?}"
    );

    // A network read is not a stdin read: it coexists with the one admitted
    // stdin read on a path, and alternatives do not add up.
    let with_stdin = r#"
module test.network_and_stdin;
permit { network.read, process.stdin.read }
@id("net.mixed")
fn mixed(handle: usize) -> usize uses { network.read, process.stdin.read } {
    let input = stdin_read();
    let chunk = net_recv(handle, 65536usize);
    let alternative = if handle == 0usize { net_recv(handle, 1usize) } else { net_recv(handle, 2usize) };
    byte_len(bytes_as_slice(input)) + byte_len(bytes_as_slice(chunk)) + byte_len(bytes_as_slice(alternative))
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = verified(with_stdin, "network-and-stdin.spx");
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
    let json = graph::to_json(&ast).unwrap();
    assert!(json.contains(
        "\"bytes_copy_sites\":2,\"stdin_read_sites\":1,\"owned_byte_payload_bytes\":196608"
    ));

    // `net_stream_stdout` publishes through the bounded stdout transcript
    // like `stdout_append`: repeated appends on one path stay admitted and
    // never count as a legacy single write.
    let streams = SOURCE.replace(
        "    net_close(handle) == 0usize && sent == 5usize\n",
        "    let more = net_stream_stdout(handle, 4096usize);\n    let appended = stdout_append(array_as_slice(request));\n    net_close(handle) == 0usize && sent == 5usize && more + appended > 0usize\n",
    );
    let ast = verified(&streams, "stream-and-append.spx");
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
    let json = graph::to_json(&ast).unwrap();
    assert!(json.contains("\"function\":\"net.stream\",\"inline_array_frame_bytes\":14,\"active_array_call_path_bytes\":14,\"bytes_copy_sites\":0,\"stdin_read_sites\":0,\"owned_byte_payload_bytes\":0,\"stdout_write_sites\":0,\"stderr_write_sites\":0,\"combined_transcript_bytes\":0"), "{json}");
}
