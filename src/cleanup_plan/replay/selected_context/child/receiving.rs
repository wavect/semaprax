//! Lexical receiving banks: argument expressions execute only after admission.
//! The two acquiring primitives keep their genuine prepaid Diagnostic boundary.
macro_rules! receive {
    ($function:expr, $budget:expr, $permit:ident, $layout:ty, $fields:literal, $call:expr) => {{
        match $budget.reserve_array::<$layout>($function, 1, "selected structural receiving") {
            Ok(()) => {}
            Err(error) => {
                return Err(
                    crate::cleanup_plan::replay::expression_facts::errors::EngineError::Owned(
                        error,
                    ),
                )
            }
        }
        match $budget.charge($function, $fields, "selected structural receiving") {
            Ok(()) => {}
            Err(error) => {
                return Err(
                    crate::cleanup_plan::replay::expression_facts::errors::EngineError::Owned(
                        error,
                    ),
                )
            }
        }
        match $call {
            Ok(value) => value,
            Err(error) => return $permit.resolve(Err(error)),
        }
    }};
}
pub(in crate::cleanup_plan::replay) use receive;
