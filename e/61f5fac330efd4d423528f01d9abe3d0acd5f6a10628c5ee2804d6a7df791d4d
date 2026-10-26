#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! One lexical acquisition, deferred until an actual owned payload is needed.
//! The entry directory already owns the unchanged exact checked upper bound.
use crate::ast::program_source::function_alias::FunctionEntry;
use crate::ast::Function;
use crate::workspace_graph::constructor_failure::{
    receiving::{Receiver, Setup},
    typed::Failure,
};
use std::mem::size_of;

// Fund the capacity receiver/result, comparison operand and decision before
// querying the owned directory. Each method has its actual caller reborrow
// and callee-self slot; no surviving primitive bank is charged twice.
// There is no forwarded helper/callee Result.
pub(super) const CHECK_CONTROLS: usize =
    2 * size_of::<&Vec<Function>>() + 2 * size_of::<usize>() + size_of::<bool>();
// The 'static lifetime is used only in size_of: reference carrier layout is
// independent of the actual provider lifetime held by FunctionEntry.
pub(super) const ACQUIRE_CONTROLS: usize = 2 * size_of::<&Vec<FunctionEntry<'static>>>()
    + 6 * size_of::<usize>() // entry capacity result, binding, layout/reserve arguments and exact-capacity comparison operands
    + size_of::<Result<std::alloc::Layout, std::alloc::LayoutError>>()
    + size_of::<std::alloc::Layout>()
    + 2 * size_of::<&std::alloc::Layout>()
    + size_of::<usize>() // layout size result passed to the original receiver
    + 2 * size_of::<&mut Vec<Function>>()
    + size_of::<Result<(), std::collections::TryReserveError>>()
    + 2 * size_of::<&Vec<Function>>()
    + size_of::<bool>()
    + size_of::<&mut Receiver>()
    + 2 * size_of::<Setup>()
    + size_of::<Failure>();

macro_rules! owned_function_directory {
    ($functions:ident, $entries:ident, $owner:ident) => {{
        $owner.reserve(owned_growth::CHECK_CONTROLS)?;
        if $functions.capacity() == 0 {
            $owner.reserve(owned_growth::ACQUIRE_CONTROLS)?;
            // directory::<FunctionEntry> already checked capacity against the
            // original imports + synthetic-main count. This owner never grows.
            let required = $entries.capacity();
            let layout = match std::alloc::Layout::array::<crate::ast::Function>(required) {
                Ok(layout) => layout,
                Err(_) => return Err($owner.physical().into()),
            };
            $owner.reserve(layout.size())?;
            match $functions.try_reserve_exact(required) {
                Ok(()) => {}
                Err(_) => return Err($owner.physical().into()),
            }
            if $functions.capacity() != required {
                return Err($owner.physical().into());
            }
        }
    }};
}
