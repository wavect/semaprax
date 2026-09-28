use super::*;
use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{ExitContinuation, StorageId};
use serde_json::Value;
use std::path::Path;
const SOURCE: &str = r#"
module fixture.owned_v2;
@id("v2.state") record State {
    @id("v2.state.z") first: Bytes,
    @id("v2.state.a") second: Bytes,
    @id("v2.state.m") budget: i64,
}
@id("v2.observation") record Observation {
    @id("v2.observation.a") a: i64,
    @id("v2.observation.b") b: i32,
    @id("v2.observation.c") c: u8,
    @id("v2.observation.d") d: usize,
    @id("v2.observation.e") e: char,
    @id("v2.observation.f") f: f32,
    @id("v2.observation.g") g: f64,
    @id("v2.observation.h") h: bool,
}
@id("v2.proposal") record Proposal {
    @id("v2.proposal.a") a: i64,
    @id("v2.proposal.b") b: i32,
    @id("v2.proposal.c") c: u8,
    @id("v2.proposal.d") d: usize,
    @id("v2.proposal.e") e: char,
    @id("v2.proposal.f") f: f32,
    @id("v2.proposal.g") g: f64,
    @id("v2.proposal.h") h: bool,
}
@id("v2.park") fn park(state: own State, observation: Observation) -> State
    yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
@id("v2.main") fn main() -> i64 { 0 }
"#;
fn program(source: &str) -> hir::ResolvedProgram {
    hir::resolve(&crate::parse(source, Path::new("owned-frame-v2.spx")).unwrap()).unwrap()
}
fn entry(p: &hir::ResolvedProgram) -> &ResolvedFunction {
    p.functions
        .iter()
        .find(|f| f.id.as_str() == "v2.park")
        .unwrap()
}
#[test]
fn owned_frame_v2_real_source_roundtrip_graph_and_compiler_vectors() {
    let source = SOURCE.replace(
        "yields Observation -> Proposal {",
        "yields Observation -> Proposal requires false {",
    );
    let checked = crate::check(&source, Path::new("owned-frame-v2.spx")).unwrap();
    let canonical = crate::format::canonical(&checked);
    let again = crate::check(&canonical, Path::new("owned-frame-v2.spx")).unwrap();
    assert_eq!(crate::format::canonical(&again), canonical);
    assert_eq!(
        crate::graph::to_json(&checked).unwrap(),
        crate::graph::to_json(&again).unwrap()
    );
    let p = hir::resolve(&checked).unwrap();
    let f = entry(&p);
    assert!(!crate::cleanup_plan::owned_frame_parameter(
        &p.declarations,
        &f.params
    ));
    assert!(crate::cleanup_plan::owned_frame_liveness(&p.declarations, f).is_err());
    let proof = owned_frame_v2_liveness(&p.declarations, f).unwrap();
    assert_eq!(proof.storage, StorageId::Value(f.params[0].id.clone()));
    assert_eq!(proof.leaves.len(), 2);
    assert!(proof.completion_cleanup.is_empty());
    let failure = f
        .cleanup_plan
        .exits
        .iter()
        .find(|e| matches!(e.continuation, ExitContinuation::ReturnFailure { .. }))
        .unwrap();
    assert_eq!(proof.failure_cleanup, failure.finalize_in_order);
    assert_eq!(
        proof
            .failure_cleanup
            .iter()
            .map(|a| a.source.projections[0].as_str())
            .collect::<Vec<_>>(),
        ["v2.state.a", "v2.state.z"]
    );
    let graph: Value = serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "v2.park")
        .unwrap();
    assert_eq!(node["params"][0]["ownership_mode"], "own");
    assert_eq!(node["params"][0]["type_id"], f.params[0].ty.identity_key());
    assert_eq!(node["params"][1]["type_id"], f.params[1].ty.identity_key());
    let yielded = &node["body"]["statements"][0]["value"];
    assert_eq!(yielded["kind"], "yield");
    assert_eq!(yielded["request_type_id"], f.params[1].ty.identity_key());
    assert_eq!(
        yielded["type_id"],
        f.yields.as_ref().unwrap().response_type.identity_key()
    );
    assert_eq!(crate::codegen::emit_hir_c(&p).unwrap_err().code, "SPX-B116");
    assert_eq!(
        crate::wasm::emit_resolved_module(&p).unwrap_err().code,
        "SPX-W126"
    );
}
#[test]
fn owned_frame_v2_exact_helper_body_and_copy_shapes_refuse_stably() {
    let ordinary = SOURCE
        .replace("yields Observation -> Proposal", "")
        .replace(
            "let proposal = yield observation;",
            "let extra = bytes_zeroed(1usize);",
        );
    program(&ordinary); // Positive control: the local is valid before suspension admission.
    for body in [
        "let prefix = 1; let proposal = yield observation; state",
        "let extra = bytes_zeroed(1usize); let proposal = yield observation; state",
        "let proposal = yield observation; let suffix = 1; state",
        "let proposal = yield observation; let second = yield observation; state",
        "let proposal = yield Observation { a: 0, b: 0i32, c: 0u8, d: 0usize, e: 'x', f: 0.0f32, g: 0.0, h: false }; state",
    ] {
        let source=SOURCE.replace("let proposal = yield observation;\n    state",body);
        let errors=hir::resolve(&crate::parse(&source,Path::new("owned-frame-v2-negative.spx")).unwrap()).unwrap_err();
        assert!(errors.iter().any(|e|e.code=="SPX-T303"),"body={body}: {errors:?}");
    }
    let source = SOURCE.replace(
        "observation: Observation",
        "observation: borrow Observation",
    );
    let errors =
        hir::resolve(&crate::parse(&source, Path::new("owned-frame-v2-borrow.spx")).unwrap())
            .unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T303"), "{errors:?}");
    let p = program(SOURCE);
    for ty in [&ResolvedType::Bytes, &ResolvedType::I64] {
        assert!(!flat_copy_record(&p.declarations, ty));
    }
}
#[test]
fn owned_frame_v2_hostile_actual_liveness_flags_storage_order_and_vectors_refuse() {
    for hostile in ["flag", "storage", "order", "vector"] {
        let mut p = program(SOURCE);
        let action = owned_frame_v2_liveness(&p.declarations, entry(&p))
            .unwrap()
            .failure_cleanup[0]
            .clone();
        let f = p
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "v2.park")
            .unwrap();
        match hostile {
            "flag" => {
                let FieldLivenessShape::Record { fields, .. } =
                    &mut f.cleanup_plan.slots[0].field_liveness_shape
                else {
                    panic!()
                };
                let FieldLivenessShape::Leaf { flag, .. } = &mut fields[1].shape else {
                    panic!()
                };
                *flag = LivenessFlagId(0);
                let FieldLivenessShape::Leaf { flag, .. } = &mut fields[0].shape else {
                    panic!()
                };
                *flag = LivenessFlagId(0);
            }
            "storage" => {
                f.cleanup_plan.entry_state.live_owned_parameters[0].storage =
                    StorageId::ProvisionalResult
            }
            "order" => {
                let FieldLivenessShape::Record { fields, .. } =
                    &mut f.cleanup_plan.slots[0].field_liveness_shape
                else {
                    panic!()
                };
                fields.swap(0, 1);
            }
            _ => {
                let exit = f
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|e| matches!(e.continuation, ExitContinuation::CommitResult { .. }))
                    .unwrap();
                // Actual compiler root finalizer cannot become successful nonresult cleanup.
                exit.finalize_in_order.push(action);
            }
        }
        assert_eq!(
            owned_frame_v2_liveness(&p.declarations, entry(&p))
                .unwrap_err()
                .code,
            "SPX-T303",
            "{hostile}"
        );
    }
}
