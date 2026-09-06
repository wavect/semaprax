//! Native carrier selection for compiler-owned cleanup leaves.

use super::ByteSlot;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum OwnedLeafKind {
    Bytes,
    Vec,
}

impl OwnedLeafKind {
    pub(super) fn c_type(self) -> &'static str {
        match self {
            Self::Bytes => "spx_bytes_v1",
            Self::Vec => "spx_vec_v1",
        }
    }

    pub(super) fn move_call(self, source: &str) -> String {
        match self {
            Self::Bytes => format!("spx_bytes_move(&{source})"),
            Self::Vec => format!("spx_vec_move(spx_ctx, &{source})"),
        }
    }

    pub(super) fn drop_call(self, value: &str) -> String {
        match self {
            Self::Bytes => format!("spx_bytes_drop(&{value})"),
            Self::Vec => format!("spx_vec_drop(spx_ctx, &{value})"),
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
