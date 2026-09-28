//! Private checked-helper foundation. No Agent association, wire binding, or
//! durable owner authority is supplied by this facade.
mod plan;

pub(crate) use plan::{compile_owned_frame_helper_v2, CheckedOwnedFrameHelperV2};
