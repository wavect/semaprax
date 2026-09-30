use crate::{cleanup_plan, hir};
use std::path::Path;

pub(crate) const SOURCE: &str = r#"
module fixture.owned_frame;
@id("fixture.state")
record State {
    @id("fixture.state.z") objective: Bytes,
    @id("fixture.state.a") second: Bytes,
    @id("fixture.state.m") budget: i64,
}
@id("fixture.park")
fn park_state(state: own State) -> State yields i64 -> i64 {
    let answer = yield state.budget;
    state
}
@id("fixture.main") fn main() -> i64 { 0 }
"#;

#[test]
fn owned_state_source_canonical_graph_and_real_record_liveness() {
    let source = r#"module fixture.owned_frame;

@id("fixture.state")
record State {
    @id("fixture.state.objective")
    objective: Bytes,
    @id("fixture.state.budget")
    budget: i64,
    @id("fixture.state.epoch")
    epoch: i64,
}

@id("fixture.park")
fn park_state(state: own State) -> State
    yields i64 -> i64
{
    let answer = yield state.budget;
    state
}

@id("fixture.main")
fn main() -> i64
{
    0
}
"#;
    // Keep the executable fixture self-contained: the baseline's exact source is
    // separately projected below with a second owned leaf for order discrimination.
    assert!(source.contains("state: own State"));
    for source in [SOURCE, source] {
        let path = Path::new("owned-frame.spx");
        let checked = crate::check(source, path).unwrap();
        let canonical = crate::format::canonical(&checked);
        let rechecked = crate::check(&canonical, path).unwrap();
        assert_eq!(crate::format::canonical(&rechecked), canonical);
        assert_eq!(
            crate::graph::to_json(&checked).unwrap(),
            crate::graph::to_json(&rechecked).unwrap()
        );
        let program = hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fixture.park")
            .unwrap();
        let proof = cleanup_plan::owned_frame_liveness(&program.declarations, function).unwrap();
        assert_eq!(
            proof.storage,
            cleanup_plan::StorageId::Value(function.params[0].id.clone())
        );
        assert!(matches!(
            function.cleanup_plan.slots[0].field_liveness_shape,
            crate::cleanup::FieldLivenessShape::Record { .. }
        ));
        assert!(proof.completion_cleanup.is_empty());
        assert_eq!(proof.leaves.len(), proof.failure_cleanup.len());
        let graph: serde_json::Value =
            serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
        let node = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "fixture.park")
            .unwrap();
        assert_eq!(node["params"][0]["ownership_mode"], "own");
        assert_eq!(
            node["params"][0]["type_id"],
            function.return_type.identity_key()
        );
        assert_eq!(node["body"]["statements"][0]["value"]["kind"], "yield");
        assert_eq!(
            crate::codegen::emit_hir_c(&program).unwrap_err().code,
            "SPX-B116"
        );
        assert_eq!(
            crate::resumable_effects::target::prepare_target_profile(
                &program,
                "fixture.park",
                crate::resumable_effects::target::ResumableArtifactTarget::NativeC11
            )
            .unwrap_err()
            .code,
            "SPX-H006"
        );
        assert_eq!(
            crate::wasm::emit_resolved_module(&program)
                .unwrap_err()
                .code,
            "SPX-W126"
        );
    }
}

#[test]
fn suspension_cleanup_matches_existing_requires_failure_without_id_sorting() {
    let source = SOURCE.replace("yields i64 -> i64 {", "yields i64 -> i64 requires false {");
    let program =
        hir::resolve(&crate::parse(&source, Path::new("owned-frame-failure.spx")).unwrap())
            .unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "fixture.park")
        .unwrap();
    let proof = cleanup_plan::owned_frame_liveness(&program.declarations, function).unwrap();
    let ordinary = function
        .cleanup_plan
        .exits
        .iter()
        .find(|e| {
            matches!(
                e.continuation,
                cleanup_plan::ExitContinuation::ReturnFailure { .. }
            )
        })
        .unwrap();
    assert_eq!(proof.failure_cleanup, ordinary.finalize_in_order);
    assert_eq!(
        proof
            .failure_cleanup
            .iter()
            .map(|a| a.source.projections[0].as_str())
            .collect::<Vec<_>>(),
        ["fixture.state.a", "fixture.state.z"]
    );
}

#[test]
fn owned_frame_refuses_additional_owned_local_assignment_call_and_second_yield() {
    // The additional Bytes local is valid ordinary source. Its rejection must
    // come from suspension admission, not a byte-operation type mismatch.
    for local in [
        "let extra = bytes_zeroed(1usize);",
        "let mut extra = bytes_zeroed(1usize);",
    ] {
        let ordinary = SOURCE
            .replace("yields i64 -> i64", "")
            .replace("let answer = yield state.budget;", local);
        hir::resolve(&crate::parse(&ordinary, Path::new("owned-frame-local-control.spx")).unwrap())
            .expect("the additional owned local is well typed without suspension");
    }
    for body in [
        "let extra = bytes_zeroed(1usize); let answer = yield state.budget; state",
        "let mut extra = bytes_zeroed(1usize); let answer = yield state.budget; state",
        "let answer = yield state.budget; let other = yield answer; state",
    ] {
        let source = SOURCE.replace("let answer = yield state.budget;\n    state", body);
        let error =
            hir::resolve(&crate::parse(&source, Path::new("owned-frame-rejected.spx")).unwrap())
                .unwrap_err();
        assert!(error.iter().any(|e| e.code == "SPX-T303"), "{error:?}");
    }

    // Parameters are immutable: this hostile assignment selects the language's
    // ownership diagnostic before suspension-profile admission.
    let source = SOURCE.replace(
        "let answer = yield state.budget;\n    state",
        "let answer = yield state.budget; state.budget = answer; state",
    );
    let error =
        hir::resolve(&crate::parse(&source, Path::new("owned-frame-rejected.spx")).unwrap())
            .unwrap_err();
    assert!(error.iter().any(|e| e.code == "SPX-U107"), "{error:?}");
}
