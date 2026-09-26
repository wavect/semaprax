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
        .binding(1, &arguments, &[(0, ResumableScalar::I64(5))], &[])
        .unwrap();
    assert_ne!(
        base,
        plan.binding(1, &arguments, &[(1, ResumableScalar::I64(5))], &[])
            .unwrap()
    );
    assert_ne!(
        base,
        plan.binding(1, &arguments, &[(0, ResumableScalar::I64(6))], &[])
            .unwrap()
    );
    assert_ne!(
        base,
        plan.binding(0, &arguments, &[(0, ResumableScalar::I64(5))], &[])
            .unwrap()
    );
    let full = vec![(0, ResumableScalar::I64(0)); MAX_CONTROL_SUSPENSIONS];
    assert!(plan.binding(0, &arguments, &full, &[]).is_err());
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

/// Issue #296, spec section 11.6, second increment: an owned `Bytes` local
/// created before the control-dependent site and used after it is admitted
/// into the plan with a v4 identity, carries exactly that one local at its
/// one site, and native/Wasm emission keep refusing it with their frozen
/// codes exactly as they do the Copy-scalar control profile.
const OWNED_BYTES_SOURCE: &str = r#"
module test.control_lowering_owned;
@id("bytes.make")
fn make_buf() -> Bytes {
    let bytes = [1u8, 2u8, 3u8];
    bytes_copy(array_as_slice(bytes))
}
@id("bytes.consume")
fn consume(value: own Bytes) -> i64 {
    let _ = bytes_as_slice(value);
    100
}
@id("app.ask")
fn ask(seed: i64, flag: bool) -> i64
    yields i64 -> i64
{
    let outcome = if flag {
        let buf = make_buf();
        let answer = yield seed;
        let used = consume(buf);
        answer + used
    } else {
        0
    };
    outcome
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn an_owned_bytes_local_live_across_the_site_is_admitted_with_a_v4_identity() {
    let resolved = program(OWNED_BYTES_SOURCE);
    hir::validate(&resolved).unwrap();
    let ask = function(&resolved, "app.ask");
    let plan = lower_control(&resolved, ask).unwrap();
    assert!(plan.carries_owned_bytes);
    assert_eq!(plan.sites.len(), 1);
    assert_eq!(plan.sites[0].carried.len(), 1);

    // Two identically-shaped plans, one carrying and one not, never collide:
    // the domain, not merely the hashed facts, differs.
    let non_carrying = lower_control(
        &program(CONTROL_SOURCE),
        function(&program(CONTROL_SOURCE), "app.ask"),
    )
    .unwrap();
    assert!(!non_carrying.carries_owned_bytes);
    assert_ne!(plan.identity, non_carrying.identity);

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

/// A `Bytes` local that never reaches any suspension (it is fully created and
/// consumed before the one `yield`) resolves entirely within one
/// non-suspended segment: this admission is not about it at all, so it is
/// admitted exactly as it always was outside a `yields` function.
#[test]
fn an_owned_bytes_local_never_live_across_any_site_is_admitted() {
    let source = OWNED_BYTES_SOURCE.replace(
        "let buf = make_buf();\n        let answer = yield seed;\n        let used = consume(buf);\n        answer + used",
        "let buf = make_buf();\n        let used = consume(buf);\n        let answer = yield seed;\n        answer + used",
    );
    let ast = crate::parse(&source, Path::new("control-lowering-owned-dead.spx")).unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
}

/// A `Bytes` local that does reach the suspension, but only past a preceding
/// statement that branches on its own, is exactly the shape
/// `cleanup_plan::owned_liveness::owned_locals_live_at`'s own doc comment
/// says this increment's narrower query refuses rather than joins; the whole
/// function stays refused with the ordinary `SPX-T303`, not silently admitted
/// with an approximated liveness.
#[test]
fn an_owned_bytes_local_past_a_branching_predecessor_stays_refused() {
    let source = OWNED_BYTES_SOURCE.replace(
        "let buf = make_buf();\n        let answer = yield seed;",
        "let buf = make_buf();\n        let branched = if seed > 0 { 1 } else { 2 };\n        let answer = yield seed + branched;",
    );
    let ast = crate::parse(&source, Path::new("control-lowering-owned-branch.spx")).unwrap();
    let error = hir::resolve(&ast).unwrap_err().remove(0);
    assert_eq!(error.code, "SPX-T303");
}
