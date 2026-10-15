//! Ordered states retain their complete value; tree nodes hold pointers rather
//! than eleven inline copies of the state layout. Moving a held box between
//! frontiers transfers its paid backing without cloning or refunding it.
use super::super::physical_storage::btree_node_upper;
use std::collections::BTreeSet;
use std::mem::size_of;

pub(super) fn insert_owned<T: Ord, E>(
    target: &mut BTreeSet<Box<T>>,
    value: T,
    reserve: &mut impl FnMut(usize, &'static str) -> Result<(), E>,
) -> Result<bool, E> {
    reserve(0, "incoming ownership state authority")?;
    if target.contains(&value) {
        return Ok(false);
    }
    reserve_node::<T, E>(reserve)?;
    // The node pays its pointer slots; this separate debit pays the complete
    // new box allocation. Nested owned backing is moved, never copied here.
    reserve(size_of::<T>(), "incoming ownership state box")?;
    Ok(target.insert(Box::new(value)))
}

pub(super) fn insert_held<T: Ord, E>(
    target: &mut BTreeSet<Box<T>>,
    value: Box<T>,
    reserve: &mut impl FnMut(usize, &'static str) -> Result<(), E>,
) -> Result<bool, E> {
    reserve(0, "incoming ownership state authority")?;
    if target.contains(value.as_ref()) {
        return Ok(false);
    }
    reserve_node::<T, E>(reserve)?;
    Ok(target.insert(value))
}

fn reserve_node<T, E>(
    reserve: &mut impl FnMut(usize, &'static str) -> Result<(), E>,
) -> Result<(), E> {
    reserve(
        btree_node_upper::<Box<T>, ()>().unwrap_or(usize::MAX),
        "incoming ownership state node",
    )
}

#[cfg(test)]
mod tests;
