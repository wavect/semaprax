//! Control-dependent plan (issue #296): lowering, projections across the
//! seven layers, and the frozen backend refusals.

use super::control::{is_control_dependent, lower_control, MAX_CONTROL_SUSPENSIONS};
use super::*;
use std::path::Path;

const CONTROL_SOURCE: &str = r#"
module test.control_lowering;
@id("app.ask")
fn ask(limit: i64) -> i64
    yields i64 -> i64
{
    let mut total = 0;
    let mut round = 0;
    while round < limit {
        let answer = yield round;
        total = total + answer;
        round = round + 1;
        round > 0
    }
    let bonus = if total > 10 {
        let extra = yield total;
        extra
    } else {
        0
    };
    total + bonus
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program(source: &str) -> ResolvedProgram {
    let ast = crate::parse(source, Path::new("control-lowering.spx")).unwrap();
    hir::resolve(&ast).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

#[test]
fn control_plan_is_deterministic_and_disjoint_from_the_sequential_plan() {
    let checked = program(CONTROL_SOURCE);
    hir::validate(&checked).unwrap();
    let ask = function(&checked, "app.ask");
    assert!(is_control_dependent(ask));
    let first = lower_control(&checked, ask).unwrap();
    assert_eq!(first, lower_control(&checked, ask).unwrap());
    assert_eq!(first.sites.len(), 2);
    assert_ne!(first.sites[0].state.id, first.sites[1].state.id);
    assert!(first.sites[0].expression.as_str().contains("body"));
    // The sequential plan cannot lower it, and the control plan cannot lower a
    // purely sequential function.
    assert_eq!(
        lower_sequential(&checked, ask).unwrap_err().code,
        "SPX-H006"
    );
    let sequential = program(
        "module test.sequential;\n@id(\"app.ask\")\nfn ask(seed: i64) -> i64 yields i64 -> i64 {\n    let first = yield seed;\n    yield first\n}\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let direct = function(&sequential, "app.ask");
    assert!(!is_control_dependent(direct));
    assert_eq!(
        lower_control(&sequential, direct).unwrap_err().code,
        "SPX-H006"
    );
    // A changed branch changes the plan identity.
    let changed = program_with(&CONTROL_SOURCE.replace("total > 10", "total > 11"));
    assert_ne!(
        first.identity,
        lower_control(&changed, function(&changed, "app.ask"))
            .unwrap()
            .identity
    );
    assert_eq!(MAX_CONTROL_SUSPENSIONS, 16);
}

fn program_with(source: &str) -> ResolvedProgram {
    program(source)
}

#[test]
fn control_bindings_commit_to_site_order_and_answers() {
    let program = program(CONTROL_SOURCE);
    let plan = lower_control(&program, function(&program, "app.ask")).unwrap();
    let arguments = [ResumableScalar::I64(3)];
    let base = plan
        .binding(1, &arguments, &[(0, ResumableScalar::I64(5))])
        .unwrap();
    assert_ne!(
        base,
        plan.binding(1, &arguments, &[(1, ResumableScalar::I64(5))])
            .unwrap()
    );
    assert_ne!(
        base,
        plan.binding(1, &arguments, &[(0, ResumableScalar::I64(6))])
            .unwrap()
    );
    assert_ne!(
        base,
        plan.binding(0, &arguments, &[(0, ResumableScalar::I64(5))])
            .unwrap()
    );
    let full = vec![(0, ResumableScalar::I64(0)); MAX_CONTROL_SUSPENSIONS];
    assert!(plan.binding(0, &arguments, &full).is_err());
}

/// Parser, formatter, HIR validation and semantic graph carry the new
/// placements; ordinary native and Wasm emission and resumable target
/// preparation keep refusing with their frozen codes.
#[test]
fn control_yields_round_trip_and_every_backend_still_refuses() {
    let parsed = crate::parse(CONTROL_SOURCE, Path::new("control-lowering.spx")).unwrap();
    let once = crate::format::canonical(&parsed);
    assert_eq!(once.matches("yield ").count(), 2);
    let reparsed = crate::parse(&once, Path::new("control-lowering.spx")).unwrap();
    assert_eq!(once, crate::format::canonical(&reparsed));
    let graph = crate::graph::to_json(&parsed).unwrap();
    assert_eq!(graph.matches("\"kind\":\"yield\"").count(), 2);

    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let native = crate::codegen::emit_hir_c(&resolved).unwrap_err();
    assert_eq!(native.code, "SPX-B116");
    let wasm = crate::wasm::emit_resolved_module(&resolved).unwrap_err();
    assert_eq!(wasm.code, "SPX-W126");
    for target in [
        crate::resumable_effects::target::ResumableArtifactTarget::NativeC11,
        crate::resumable_effects::target::ResumableArtifactTarget::CoreWasm,
    ] {
        let refused =
            crate::resumable_effects::target::prepare_target_profile(&resolved, "app.ask", target)
                .unwrap_err();
        assert_eq!(refused.code, "SPX-H006");
    }
}
