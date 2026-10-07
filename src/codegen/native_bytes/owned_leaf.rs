//! Native carrier selection for compiler-owned cleanup leaves.

use crate::cleanup_plan::CleanupPlace;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ByteSlot {
    pub(super) place: CleanupPlace,
    pub(super) value: String,
    pub(super) flag: String,
    pub(super) kind: OwnedLeafKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum OwnedLeafKind {
    Bytes,
    Once,
    OnceI64,
    OnceI64Pair,
    String,
    /// String Collections v1 `Map<string, i64>` carrier pointer.
    Map,
    Vec,
    Box,
    Iter,
    StdinReader,
}

impl OwnedLeafKind {
    pub(super) fn c_type(self) -> &'static str {
        match self {
            Self::Once => "spx_once_v1",
            Self::OnceI64 => "spx_once_i64_v2",
            Self::OnceI64Pair => "spx_once_i64_pair_v3",
            Self::Bytes => "spx_bytes_v1",
            Self::String => "char *",
            Self::Map => "spx_map_v1 *",
            Self::Vec => "spx_vec_v1",
            Self::Box => "spx_box_v1",
            Self::Iter => "spx_iter_v1",
            Self::StdinReader => "uintptr_t",
        }
    }

    pub(super) fn move_call(self, source: &str) -> String {
        match self {
            Self::Once => format!("spx_once_move(&{source})"),
            Self::OnceI64 => format!("spx_once_i64_move_v2(&{source})"),
            Self::OnceI64Pair => format!("spx_once_i64_pair_move_v3(&{source})"),
            Self::Bytes => format!("spx_bytes_move(&{source})"),
            Self::String | Self::Map => source.to_owned(),
            Self::Vec => format!("spx_vec_move(spx_ctx, &{source})"),
            Self::Box => format!("spx_box_move(spx_ctx, &{source})"),
            Self::Iter => format!("spx_iter_move(spx_ctx, &{source})"),
            Self::StdinReader => format!("spx_stdin_stream_move_v1(spx_ctx, &{source})"),
        }
    }

    pub(super) fn drop_call(self, value: &str) -> String {
        match self {
            Self::Once => format!("spx_once_drop(&{value})"),
            Self::OnceI64 => format!("spx_once_i64_drop_v2(&{value})"),
            Self::OnceI64Pair => format!("spx_once_i64_pair_drop_v3(&{value})"),
            Self::Bytes => format!("spx_bytes_drop(&{value})"),
            Self::String => format!("spx_string_drop({value})"),
            Self::Map => format!("spx_map_drop_v1({value})"),
            Self::Vec => format!("spx_vec_drop(spx_ctx, &{value})"),
            Self::Box => format!("spx_box_drop(spx_ctx, &{value})"),
            Self::Iter => format!("spx_iter_drop(spx_ctx, &{value})"),
            Self::StdinReader => format!("spx_stdin_stream_drop_v1(spx_ctx, {value})"),
        }
    }
}

pub(super) fn emit_transfer(source: &ByteSlot, destination: &ByteSlot, context: &str) -> String {
    let move_call = if source.kind == destination.kind {
        source.kind.move_call(&source.value)
    } else {
        "spx_runtime_invalid_owned_move()".to_owned()
    };
    format!(
        "if (!{} || {}) spx_runtime_invariant_failure(\"owned {context} liveness {} to {}\");\n{} = {};\n{} = false;\n{} = true;\n",
        source.flag,
        destination.flag,
        source.value,
        destination.value,
        destination.value,
        move_call,
        source.flag,
        destination.flag
    )
}
