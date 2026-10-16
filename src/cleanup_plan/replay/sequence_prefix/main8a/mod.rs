//! Independent complete Main8a protocol; never used by shipping replay.
use crate::cleanup_plan::replay::path_array::main696 as path_array;
use crate::cleanup_plan::replay::{
    empty_expr_path, lexical_skeleton, note_skeleton_materialization, skeleton_materializations,
    CleanupPlace, Diagnostic, ExprSkeletonPath, Observations, ReplayBudget, ResolvedFunction,
    ResolvedProgram, SkeletonObservation, SkeletonWork, StatusLane, StatusSourceId, StorageId,
    MAX_REPLAY_WORK_UNITS,
};
pub(in crate::cleanup_plan::replay) use current::sequence_skeleton_paths;
#[path = "current.rs"]
mod current;
#[path = "helper.rs"]
pub(in crate::cleanup_plan::replay) mod helper;
