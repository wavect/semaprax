//! GEN-06A independent replay of mixed Result residual ownership.
use super::*;

#[test]
fn mixed_result_residual_replay_keeps_empty_error_cases_and_rejects_missing_transfer() {
    for error in [
        "i64", "i32", "u8", "usize", "char", "f32", "f64", "bool", "Bytes",
    ] {
        let text = format!(
            r#"
module test.mixed_result_replay;
@id("propagate") fn propagate(value: own Result<Bytes, {error}>) -> Result<Bytes, {error}> {{
  let payload = value?;
  Result<Bytes, {error}>::Ok {{ value: payload }}
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let parsed = crate::check(&text, "mixed-result-replay.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "propagate")
            .unwrap();
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V6);
        validate_structure(&program, function).unwrap();
        let cases = &function.cleanup.entry_state.conditional_owned_parameters[0].cases;
        assert_eq!(cases[0].live_flags.len(), 1);
        assert_eq!(cases[1].live_flags.len(), usize::from(error == "Bytes"));

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

        let mut hostile = function.clone();
        let cases = &mut hostile.cleanup.entry_state.conditional_owned_parameters[0].cases;
        if error == "Bytes" {
            cases[1].live_flags.clear();
        } else {
            let ok_flag = cases[0].live_flags[0];
            cases[1].live_flags.push(ok_flag);
        }
        assert_eq!(
            validate_structure(&program, &hostile).unwrap_err().code,
            "SPX-H006"
        );
    }
}
