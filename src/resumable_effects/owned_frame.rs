//! Compiler/evaluator foundation only; no checkpoint or durable ownership authority.
pub(crate) mod plan;
pub(crate) use plan::{compile_owned_frame_plan, CheckedOwnedFramePlan};
