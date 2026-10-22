//! Selected structural-control receiving closure. Original general APIs remain.
use super::*;
use crate::cleanup_plan::replay::expression_facts::errors::EngineError as Error;
use crate::cleanup_plan::replay::selected_context::child::Permit;
// The closure capture and its new typed Result are admitted before the
// closure is formed. Macro expansion adds no forwarding helper activation.
macro_rules! control_refusal {
    ($budget:ident,$function:ident,$permit:ident,$value:ty,$kind:expr) => {{
        $budget.reserve_array::<(&Permit<'_>, Result<$value, Error>)>(
            $function,
            1,
            "selected control closure receiving",
        )?;
        $budget.charge($function, 2, "selected control closure receiving")?;
        || $permit.control(Reason::Finite($kind))
    }};
}
// The iterator header and local frame do not pay a fresh next call's
// argument, Option result or pattern bindings. This lexical packet is acquired
// on every call, including the terminal None; no forwarding helper is formed.
macro_rules! dynamic_next {
    ($budget:ident,$function:ident,$iter:ident,$iterator:ty,$item:ty,($($binding:ty),*),$fields:expr) => {{
        $budget.reserve_array::<(&mut $iterator, Option<$item>, $($binding,)*)>(
            $function, 1, "selected dynamic iterator next receiving")?;
        $budget.charge($function, $fields, "selected dynamic iterator next receiving")?;
        $iter.next()
    }};

    ($budget:ident,$function:ident,$iter:ident,$roles:ty,$fields:expr) => {{
        $budget.reserve_array::<$roles>($function, 1,
            "selected dynamic iterator next receiving")?;
        $budget.charge($function, $fields, "selected dynamic iterator next receiving")?;
        $iter.next()
    }};
}
macro_rules! dynamic_get {
    ($budget:ident,$function:ident,$receiver:expr,$receiver_type:ty,$index:expr,$index_type:ty,$result:ty,($($binding:ty),*),$fields:expr) => {{
        $budget.reserve_array::<($receiver_type, $index_type, $result, $($binding,)*)>(
            $function, 1, "selected dynamic get receiving")?;
        $budget.charge($function, $fields, "selected dynamic get receiving")?;
        $receiver.get($index)
    }};
}
pub(in crate::cleanup_plan::replay) use dynamic_get;
pub(in crate::cleanup_plan::replay) use dynamic_next;
// Existing dynamic Atom routes keep their original renderer and stage. Their
// fresh clone/argument/callee/receipt/error transport is distinct from the
// already paid finite-control permit, and is admitted before either ID clone.
macro_rules! existing_structural_error {
    ($budget:ident,$function:ident,$id:expr,$variant:ident) => {{
        $budget.reserve_array::<(
            &ResolvedFunction,
            &ExpressionId,
            ExpressionId,
            semantic_refusal::Atom,
            &ResolvedFunction,
            semantic_refusal::Atom,
            &ExpressionId,
            ExpressionId,
            semantic_refusal::Receipt,
            Error,
            Error,
        )>($function, 1, "selected existing dynamic atom receiving")?;
        $budget.charge($function, 11, "selected existing dynamic atom receiving")?;
        Error::structural($function, semantic_refusal::Atom::$variant($id.clone()))
    }};
    ($budget:ident,$function:ident,$id:expr,$variant:ident,$flag:expr) => {{
        $budget.reserve_array::<(
            &ResolvedFunction,
            &ExpressionId,
            ExpressionId,
            semantic_refusal::Atom,
            &ResolvedFunction,
            semantic_refusal::Atom,
            &ExpressionId,
            ExpressionId,
            semantic_refusal::Receipt,
            Error,
            Error,
            LivenessFlagId,
        )>(
            $function,
            1,
            "selected existing dynamic atom flag receiving",
        )?;
        $budget.charge(
            $function,
            12,
            "selected existing dynamic atom flag receiving",
        )?;
        Error::structural(
            $function,
            semantic_refusal::Atom::$variant($id.clone(), $flag),
        )
    }};
}
pub(in crate::cleanup_plan::replay) use existing_structural_error;
pub(in crate::cleanup_plan::replay) mod reason;
use reason::{Kind, Reason};
mod blocks;
mod calls;
mod cfg;
mod entry;
#[cfg(test)]
mod entry_before_dynamic;
#[cfg(test)]
mod entry_before_formals;
mod exits;
pub(in crate::cleanup_plan::replay) mod helpers;
#[cfg(test)]
mod place_before_formals;
mod status;
pub(in crate::cleanup_plan::replay) use blocks::validate_blocks_and_edges_paid;
pub(in crate::cleanup_plan::replay) use cfg::validate_reachable_acyclic_cfg;
pub(in crate::cleanup_plan::replay) use entry::validate_entry;
pub(in crate::cleanup_plan::replay) use exits::validate_exits;
pub(in crate::cleanup_plan::replay) use status::validate_core as validate_status;

#[cfg(test)]
mod tests;
