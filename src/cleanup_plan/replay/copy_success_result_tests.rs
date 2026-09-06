//! Independent replay keeps Copy-success extraction separate from owner transfer.
use super::*;

#[test]
fn copy_success_result_has_no_success_owner_and_replays_error_transfer() {
    for success in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        let text = format!(
            r#"
module test.copy_success_result_replay;
@id("propagate") fn propagate(value: own Result<{success}, Bytes>) -> Result<{success}, Bytes> {{
  let payload = value?;
  Result<{success}, Bytes>::Ok {{ value: payload }}
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let parsed = crate::check(&text, "copy-success-result-replay.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "propagate")
            .unwrap();
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V6);
        validate_structure(&program, function).unwrap();
        let cases = &function.cleanup.entry_state.conditional_owned_parameters[0].cases;
        assert!(cases[0].live_flags.is_empty());
        assert_eq!(cases[1].live_flags.len(), 1);
        let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("block");
        };
        let ResolvedStatement::Let {
            value: extracted, ..
        } = &statements[0]
        else {
            panic!("let");
        };
        assert!(matches!(extracted.kind, ResolvedExprKind::Try { .. }));
        assert_eq!(extracted.ownership, OwnershipMode::Value);
        assert!(!function.cleanup.slots.iter().any(|slot| matches!(
            &slot.origin, CleanupStorageOrigin::Temporary { expression } if expression == &extracted.id)));
        assert!(!function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|b| &b.transitions)
            .any(|t| matches!(
            t, CleanupTransition::Transfer { at, source, .. } if at == &extracted.id
                && source.projections.iter().any(|id| id.as_str() == prelude::RESULT_OK_ID))));
        let mut hostile = function.clone();
        let block = hostile
            .cleanup_plan
            .blocks
            .iter_mut()
            .find(|b| {
                b.transitions
                    .iter()
                    .any(|t| matches!(t, CleanupTransition::TransferVariant { .. }))
            })
            .unwrap();
        let index = block
            .transitions
            .iter()
            .position(|t| matches!(t, CleanupTransition::TransferVariant { .. }))
            .unwrap();
        block.transitions.remove(index);
        assert_eq!(
            validate_structure(&program, &hostile).unwrap_err().code,
            "SPX-H006"
        );
    }
}
