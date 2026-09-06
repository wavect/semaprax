//! Focused evidence for Owned Bounded Byte Buffer v1.
//!
//! One buffer is one write-once chain expression: `bytes_zeroed` allocates a
//! zeroed buffer at a literal capacity and each `bytes_set` transfers that same
//! owner in, stores one byte at a literal index below the capacity, and hands
//! the owner back. Nothing in the chain is nameable, so a partially filled
//! buffer has no second owner, no borrowed view, and no observable intermediate
//! state; binding the chain's result freezes it, after which only the
//! established borrowed reads apply.
//!
//! Capacity exhaustion, an out-of-range element index, a dynamic capacity, and
//! a second owner are therefore compile-time diagnostics on both projections.
//! The reference interpreter and the native C backend execute the feature; the
//! WebAssembly backend rejects it precisely because its owned bytes are opaque
//! host-arena tokens reached through a frozen import set.

use std::path::Path;
use std::process::Command;

use semaprax::cleanup_plan::{CleanupTransition, StorageId};
use semaprax::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement};
use semaprax::{codegen, format, graph, parse, verify, wasm};

/// Three elements written at three literal indices, then frozen, borrowed, and
/// read: length, an in-range element, and an out-of-range element.
const BUFFER: &str = r#"
module test.owned_byte_buffer_v1;

@id("buffer.main")
fn main() -> i64
{
    let buffer = bytes_set(bytes_set(bytes_set(bytes_zeroed(3usize), 0usize, 65u8), 1usize, 66u8), 2usize, 67u8);
    let view = bytes_as_slice(buffer);
    let first = match byte_get(view, 0usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let last = match byte_get(view, 2usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    };
    let past_end = match byte_get(view, 3usize) {
        Option::Some { value: byte } => 1i32,
        Option::None {} => 0i32,
    };
    if byte_len(view) == 3usize && first == 65u8 && last == 67u8 && past_end == 0i32 { 7 } else { 1 }
}
"#;

fn error_codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, "owned-byte-buffer-invalid.spx").unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn assert_rejected(source: &str, expected: &'static str) {
    let codes = error_codes(source);
    assert!(
        codes.contains(&expected),
        "source verifier did not report {expected}: {codes:?}"
    );
}

fn program_source(body: &str) -> String {
    format!("module test.owned_byte_buffer_case;\n\n@id(\"buffer.main\")\nfn main() -> i64\n{{\n{body}\n}}\n")
}

fn command_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn main_function(program: &hir::ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "buffer.main")
        .unwrap()
}

fn main_function_mut(program: &mut hir::ResolvedProgram) -> &mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "buffer.main")
        .unwrap()
}

/// The outermost `bytes_set` call of the first `let` in `buffer.main`.
fn fill_chain(program: &mut hir::ResolvedProgram) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "buffer.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        unreachable!();
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        unreachable!();
    };
    value
}

#[test]
fn write_once_buffer_fills_freezes_and_reads_with_one_owner_and_one_drop() {
    let program = parse(BUFFER, "owned-byte-buffer-v1.spx").unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "the write-once chain is admitted source"
    );

    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "owned-byte-buffer-v1-canonical.spx").unwrap();
    assert_eq!(
        format::canonical(&reparsed),
        canonical,
        "the chain round-trips through the canonical formatter"
    );

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    // Exactly one destruction path. The allocation temporary, each call
    // argument, each intermediate result, and the frozen local are all
    // separate cleanup slots, but only the frozen local is ever finalized:
    // every earlier slot is transferred into the next chain link.
    let plan = &main_function(&resolved).cleanup_plan;
    let finalizers = plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .count();
    assert_eq!(
        finalizers, 1,
        "a filled buffer has exactly one destruction path"
    );
    assert!(
        plan.slots.len() > 1,
        "the chain stages its owner through distinct cleanup slots"
    );

    let json = graph::to_json(&program).unwrap();
    assert_eq!(
        json,
        graph::to_json(&program).unwrap(),
        "the graph projection is deterministic"
    );
    assert!(json.contains("core.bytes.zeroed"));
    assert!(json.contains("core.bytes.set"));
    assert!(json.contains("core.bytes.drop"));

    let interpreted = interpret(BUFFER, "buffer-interp");
    assert!(
        interpreted.contains("\"kind\":\"returned\"") && interpreted.contains("\"value\":\"7\""),
        "the reference interpreter fills, freezes and reads the buffer: {interpreted}"
    );

    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(
        generated,
        codegen::emit_c(&program).unwrap(),
        "native emission is deterministic"
    );
    assert_eq!(
        generated.matches("spx_bytes_zeroed(").count(),
        2,
        "one runtime definition and exactly one allocation site"
    );
    assert_eq!(
        generated.matches("= spx_bytes_set(").count(),
        3,
        "one native store per admitted element"
    );

    if !command_available("clang") {
        return;
    }
    let native = std::env::temp_dir().join(format!(
        "semaprax-owned-byte-buffer-{}.native{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    codegen::build(&program, &native).unwrap();
    let output = Command::new(&native).output().unwrap();
    let _ = std::fs::remove_file(&native);
    assert!(output.status.success(), "native buffer run failed");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "7",
        "the native backend agrees with the reference interpreter"
    );
}

#[test]
fn empty_and_completely_filled_buffers_are_admitted() {
    let empty = program_source(
        "    let buffer = bytes_zeroed(0usize);\n    let view = bytes_as_slice(buffer);\n    if byte_len(view) == 0usize { 0 } else { 1 }",
    );
    assert!(
        error_codes(&empty).is_empty(),
        "an empty buffer is an admitted capacity"
    );
    let program = parse(&empty, "owned-byte-buffer-empty.spx").unwrap();
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();

    let full = program_source(
        "    let buffer = bytes_set(bytes_set(bytes_zeroed(2usize), 0usize, 1u8), 1usize, 2u8);\n    let view = bytes_as_slice(buffer);\n    if byte_len(view) == 2usize { 0 } else { 1 }",
    );
    assert!(
        error_codes(&full).is_empty(),
        "writing every element of the capacity is admitted"
    );
    let program = parse(&full, "owned-byte-buffer-full.spx").unwrap();
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn capacity_and_element_index_failures_are_compile_time_diagnostics() {
    // One past the capacity.
    assert_rejected(
        &program_source(
            "    let buffer = bytes_set(bytes_zeroed(2usize), 2usize, 1u8);\n    let view = bytes_as_slice(buffer);\n    if byte_len(view) == 2usize { 0 } else { 1 }",
        ),
        "SPX-T272",
    );
    // An index that is not a literal is not an index at a known position.
    assert_rejected(
        &program_source(
            "    let where = 1usize;\n    let buffer = bytes_set(bytes_zeroed(2usize), where, 1u8);\n    let view = bytes_as_slice(buffer);\n    if byte_len(view) == 2usize { 0 } else { 1 }",
        ),
        "SPX-T272",
    );
    // A capacity that is not known at the allocation site.
    assert_rejected(
        &program_source(
            "    let capacity = 2usize;\n    let buffer = bytes_zeroed(capacity);\n    let view = bytes_as_slice(buffer);\n    if byte_len(view) == 2usize { 0 } else { 1 }",
        ),
        "SPX-T271",
    );
    // A capacity above the admitted owned byte payload extent: the allocation
    // cannot succeed, and saying so is a diagnostic rather than a backend
    // failure.
    assert_rejected(
        &program_source(
            "    let buffer = bytes_zeroed(65537usize);\n    let view = bytes_as_slice(buffer);\n    if byte_len(view) == 65537usize { 0 } else { 1 }",
        ),
        "SPX-T271",
    );
}

#[test]
fn a_frozen_buffer_has_exactly_one_owner_and_no_stale_view() {
    // A named binding is a frozen buffer. Re-opening it would create a second
    // owner of a buffer that a borrowed view may already root.
    assert_rejected(
        &program_source(
            "    let first = bytes_zeroed(2usize);\n    let second = bytes_set(first, 0usize, 1u8);\n    let view = bytes_as_slice(second);\n    if byte_len(view) == 2usize { 0 } else { 1 }",
        ),
        "SPX-T271",
    );
    // A transfer out of a frozen buffer while a lexical view is live stays the
    // established byte-view diagnostic.
    let moved = "module test.owned_byte_buffer_move;\n\n@id(\"buffer.take\")\nfn take(value: own Bytes) -> usize { byte_len(bytes_as_slice(value)) }\n\n@id(\"buffer.main\")\nfn main() -> i64\n{\n    let buffer = bytes_set(bytes_zeroed(2usize), 0usize, 1u8);\n    let view = bytes_as_slice(buffer);\n    let taken = take(buffer);\n    if byte_len(view) == 2usize && taken == 2usize { 0 } else { 1 }\n}\n";
    assert_rejected(moved, "SPX-T265");
}

#[test]
fn allocating_a_buffer_inside_a_loop_stays_rejected() {
    let looped = program_source(
        "    let mut index = 0usize;\n    while index < 2usize {\n        let buffer = bytes_zeroed(2usize);\n        index = index + 1usize;\n        index < 2usize\n    }\n    0",
    );
    // The byte-operation while-body rule and the owned byte allocation capacity
    // rule both fire; a growable push-in-a-loop needs both decided first.
    assert_rejected(&looped, "SPX-T252");
    assert_rejected(&looped, "SPX-T267");
}

#[test]
fn cleanup_plan_authenticates_each_write_once_owner_transfer() {
    let program = parse(BUFFER, "owned-byte-buffer-cleanup.spx").unwrap();
    let baseline = hir::resolve(&program).unwrap();
    hir::validate(&baseline).unwrap();
    let plan = &main_function(&baseline).cleanup_plan;

    let owned_commits = plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::CallCommit { call, arguments } if !arguments.is_empty() => {
                Some((call, arguments))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owned_commits.len(),
        3,
        "one owned commit per bytes_set link"
    );
    for (call, arguments) in owned_commits {
        assert_eq!(arguments.len(), 1);
        let transfer = &arguments[0];
        assert_eq!(transfer.parameter_index, 0);
        let StorageId::CallArgument {
            call: staged_call,
            parameter_index,
            ..
        } = &transfer.source.storage
        else {
            panic!("owned buffer commit did not use authenticated call-argument storage")
        };
        assert_eq!(staged_call, call);
        assert_eq!(*parameter_index, 0);
        assert!(transfer.source.projections.is_empty());
        assert_eq!(
            plan.blocks
                .iter()
                .flat_map(|block| &block.transitions)
                .filter(|transition| matches!(transition,
                    CleanupTransition::Transfer { destination, .. }
                        if destination == &transfer.source))
                .count(),
            1,
            "each bytes_set call argument receives exactly one owner transfer"
        );
    }
    assert!(plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .all(|transition| !matches!(
            transition,
            CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::StageCopyResult { .. }
        )));

    let mut wrong_commit = baseline.clone();
    let transition = main_function_mut(&mut wrong_commit)
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(transition,
                CleanupTransition::CallCommit { arguments, .. } if !arguments.is_empty())
        })
        .unwrap();
    let CleanupTransition::CallCommit { arguments, .. } = transition else {
        unreachable!()
    };
    arguments[0].parameter_index = 1;
    assert_eq!(hir::validate(&wrong_commit).unwrap_err().code, "SPX-H006");

    let mut missing_transfer = baseline.clone();
    let committed_source = main_function(&missing_transfer)
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::CallCommit { arguments, .. } if !arguments.is_empty() => {
                Some(arguments[0].source.clone())
            }
            _ => None,
        })
        .unwrap();
    let transitions = &mut main_function_mut(&mut missing_transfer)
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block.transitions.iter().any(|transition| {
                matches!(transition,
                    CleanupTransition::Transfer { destination, .. }
                        if destination == &committed_source)
            })
        })
        .unwrap()
        .transitions;
    let index = transitions
        .iter()
        .position(|transition| {
            matches!(transition,
                CleanupTransition::Transfer { destination, .. }
                    if destination == &committed_source)
        })
        .unwrap();
    transitions.remove(index);
    assert_eq!(
        hir::validate(&missing_transfer).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn core_webassembly_emits_deterministically_while_the_public_adapter_stays_closed() {
    let program = parse(BUFFER, "owned-byte-buffer-wasm.spx").unwrap();
    let emitted = wasm::emit_module(&program).unwrap();
    assert_eq!(emitted, wasm::emit_module(&program).unwrap());
    assert!(
        emitted.starts_with(b"\0asm"),
        "the internal Core-Wasm route emits a valid Wasm container"
    );

    let rejection =
        wasm::emit_module_with_byte_exports(&program, &["buffer.main".to_owned()]).unwrap_err();
    assert_eq!(rejection.code, "SPX-W115", "{}", rejection.message);
    assert!(rejection.message.contains("internal-only"));
}

#[test]
fn hostile_hir_cannot_forge_a_buffer_capacity_or_element_index() {
    let program = parse(BUFFER, "owned-byte-buffer-hostile.spx").unwrap();
    let baseline = hir::resolve(&program).unwrap();
    hir::validate(&baseline).unwrap();

    // An element index past the chain's capacity.
    let mut hostile = baseline.clone();
    let chain = fill_chain(&mut hostile);
    let ResolvedExprKind::Call { args, .. } = &mut chain.kind else {
        unreachable!();
    };
    args[1].kind = ResolvedExprKind::Usize(3);
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    // An element index that is not a literal at all.
    let mut hostile = baseline.clone();
    let chain = fill_chain(&mut hostile);
    let ResolvedExprKind::Call { args, .. } = &mut chain.kind else {
        unreachable!();
    };
    let forged = args[1].clone();
    args[1].kind = ResolvedExprKind::Binary {
        op: semaprax::ast::BinaryOp::Add,
        left: Box::new(forged.clone()),
        right: Box::new(forged),
    };
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    // A capacity above the admitted owned byte payload extent.
    let mut hostile = baseline.clone();
    let chain = fill_chain(&mut hostile);
    let allocation = innermost_allocation(chain);
    let ResolvedExprKind::Call { args, .. } = &mut allocation.kind else {
        unreachable!();
    };
    args[0].kind = ResolvedExprKind::Usize(65_537);
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    // A nonliteral capacity cannot be smuggled through resolved HIR.
    let mut hostile = baseline.clone();
    let chain = fill_chain(&mut hostile);
    let allocation = innermost_allocation(chain);
    let ResolvedExprKind::Call { args, .. } = &mut allocation.kind else {
        unreachable!();
    };
    let literal = args[0].clone();
    args[0].kind = ResolvedExprKind::Binary {
        op: semaprax::ast::BinaryOp::Add,
        left: Box::new(literal.clone()),
        right: Box::new(literal),
    };
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    // A nested link cannot be relabeled as another byte operation even when
    // the outer owner transfer still looks like a write-once chain.
    let mut hostile = baseline.clone();
    let chain = fill_chain(&mut hostile);
    let ResolvedExprKind::Call { args, .. } = &mut chain.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Call { callee, .. } = &mut args[0].kind else {
        unreachable!();
    };
    *callee = hir::DeclarationId::new("core.bytes.copy");
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

/// Walk to the `bytes_zeroed` call at the base of one fill chain.
fn innermost_allocation(chain: &mut ResolvedExpr) -> &mut ResolvedExpr {
    let mut current = chain;
    loop {
        let is_set = matches!(
            &current.kind,
            ResolvedExprKind::Call { callee, .. } if callee.as_str() == "core.bytes.set"
        );
        if !is_set {
            return current;
        }
        let ResolvedExprKind::Call { args, .. } = &mut current.kind else {
            unreachable!();
        };
        current = &mut args[0];
    }
}

fn interpret(source: &str, label: &str) -> String {
    use semaprax::interpreter::{self, InterpreterOptions};
    let path = std::env::temp_dir().join(format!(
        "semaprax-owned-byte-buffer-{label}-{}.spx",
        std::process::id()
    ));
    std::fs::write(&path, format::canonical(&parse(source, &path).unwrap())).unwrap();
    let interpretation =
        interpreter::interpret(&path, "buffer.main", &[], &InterpreterOptions::default()).unwrap();
    let _ = std::fs::remove_file(Path::new(&path));
    interpretation.envelope
}
