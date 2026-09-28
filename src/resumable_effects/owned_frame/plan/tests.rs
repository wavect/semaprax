use super::*;
use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{CleanupPlace, CleanupTransition, StorageId};
use std::path::Path;
const SOURCE: &str = r#"
module fixture.owned_frame;
@id("fixture.state") record State {
    @id("fixture.state.z") objective: Bytes,
    @id("fixture.state.a") second: Bytes,
    @id("fixture.state.m") budget: i64,
}
@id("fixture.park") fn park_state(state: own State) -> State yields i64 -> i64 {
    let answer = yield state.budget;
    state
}
@id("fixture.main") fn main() -> i64 { 0 }
"#;
fn program() -> ResolvedProgram {
    hir::resolve(&crate::parse(SOURCE, Path::new("owned-frame-proof.spx")).unwrap()).unwrap()
}
#[test]
fn hostile_flags_storage_order_partial_move_and_loans_refuse_checked_plan() {
    for kind in ["flag", "storage", "order", "partial", "loan"] {
        let mut program = program();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "fixture.park")
            .unwrap();
        match kind {
            "flag" => {
                let FieldLivenessShape::Record { fields, .. } =
                    &mut function.cleanup_plan.slots[0].field_liveness_shape
                else {
                    panic!()
                };
                let FieldLivenessShape::Leaf { flag, .. } = &mut fields[0].shape else {
                    panic!()
                };
                flag.0 += 100;
            }
            "storage" => {
                function.cleanup_plan.entry_state.live_owned_parameters[0].storage =
                    StorageId::ProvisionalResult
            }
            "order" => {
                let FieldLivenessShape::Record { fields, .. } =
                    &mut function.cleanup_plan.slots[0].field_liveness_shape
                else {
                    panic!()
                };
                fields.swap(0, 1);
            }
            "partial" => {
                let site =
                    crate::cleanup_plan::owned_frame_liveness(&program.declarations, function)
                        .unwrap()
                        .site;
                function.cleanup_plan.blocks[0]
                    .transitions
                    .push(CleanupTransition::Transfer {
                        at: site,
                        source: CleanupPlace {
                            storage: StorageId::Value(function.params[0].id.clone()),
                            projections: vec![DeclarationId::new("fixture.state.z")],
                        },
                        destination: CleanupPlace {
                            storage: StorageId::ProvisionalResult,
                            projections: Vec::new(),
                        },
                    });
            }
            "loan" => {
                // A real checked projected-Bytes loan, transplanted into the
                // otherwise loan-free suspension; no synthetic counters.
                let source = SOURCE.replace(
                    "let answer = yield state.budget;",
                    "let view = bytes_as_slice(state.objective); let answer = yield state.budget;",
                );
                let errors = hir::resolve(
                    &crate::parse(&source, Path::new("owned-frame-loan.spx")).unwrap(),
                )
                .unwrap_err();
                assert!(
                    errors
                        .iter()
                        .any(|e| e.code == "SPX-T303" || e.code == "SPX-T305"),
                    "{errors:?}"
                );
                continue;
            }
            _ => unreachable!(),
        }
        let error = compile_owned_frame_plan(&program, &DeclarationId::new("fixture.park"))
            .err()
            .expect("tampered actual plan must refuse before evaluation");
        assert!(
            error.code == "SPX-H006" || error.code == "SPX-T303",
            "{kind}: {error:?}"
        );
    }
}
#[test]
fn exact_plan_binding_changes_with_source_scalar_and_cleanup_facts() {
    let program = program();
    let plan = compile_owned_frame_plan(&program, &DeclarationId::new("fixture.park")).unwrap();
    assert!(plan.binding().starts_with("sha256:"));
    assert_eq!(plan.binding().len(), 71);
    let changed = SOURCE.replace("yield state.budget", "yield state.budget + 1");
    let changed =
        hir::resolve(&crate::parse(&changed, Path::new("owned-frame-proof.spx")).unwrap()).unwrap();
    assert_ne!(
        plan.binding(),
        compile_owned_frame_plan(&changed, &DeclarationId::new("fixture.park"))
            .unwrap()
            .binding()
    );
}
