//! Test-only capacity high-water accounting for the source verifier.
//!
//! Records the peak owned bytes reached by the iterative verifier's frame,
//! scope, and value stacks so capacity regressions surface in tests.

use super::binding::{Availability, Binding, SourceLoan};
use super::scope::VerifierScope;
use crate::ast::Type;

#[cfg(test)]
thread_local! {
    static SOURCE_VERIFY_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_capacity_high_water() {
    SOURCE_VERIFY_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(crate) fn capacity_high_water() -> usize {
    SOURCE_VERIFY_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn note_capacity_high_water(bytes: usize) {
    SOURCE_VERIFY_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
pub(super) fn binding_owned_capacity(binding: &Binding) -> usize {
    let moved = binding
        .moved_places
        .iter()
        .fold(0usize, |bytes, (place, _)| {
            bytes
                + std::mem::size_of::<(Vec<String>, Availability)>()
                + place.capacity() * std::mem::size_of::<String>()
                + place.iter().map(String::capacity).sum::<usize>()
        });
    let partial = binding
        .definitely_partial
        .iter()
        .fold(0usize, |bytes, place| {
            bytes
                + std::mem::size_of::<Vec<String>>()
                + place.capacity() * std::mem::size_of::<String>()
                + place.iter().map(String::capacity).sum::<usize>()
        });
    let loans = binding.active_loans.iter().fold(0usize, |bytes, loan| {
        bytes
            + loan.id.borrower.capacity()
            + loan.projections.capacity() * std::mem::size_of::<String>()
            + loan.projections.iter().map(String::capacity).sum::<usize>()
    });
    let origin = binding.borrow_origin.as_ref().map_or(0usize, |origin| {
        origin.root.capacity()
            + origin.loan.borrower.capacity()
            + origin
                .parent
                .as_ref()
                .map_or(0usize, |parent| parent.borrower.capacity())
            + origin.projections.capacity() * std::mem::size_of::<String>()
            + origin
                .projections
                .iter()
                .map(String::capacity)
                .sum::<usize>()
    });
    binding
        .moved_places
        .capacity()
        .saturating_mul(std::mem::size_of::<(Vec<String>, Availability)>())
        .saturating_add(
            binding
                .definitely_partial
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<String>>()),
        )
        .saturating_add(ast_type_owned_capacity(&binding.ty))
        .saturating_add(moved)
        .saturating_add(partial)
        .saturating_add(
            binding
                .active_loans
                .len()
                .saturating_mul(std::mem::size_of::<SourceLoan>()),
        )
        .saturating_add(loans)
        .saturating_add(origin)
}

#[cfg(test)]
pub(super) fn ast_type_owned_capacity(ty: &Type) -> usize {
    match ty {
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::ArrayU8(_)
        | Type::F32
        | Type::F64
        | Type::Bool => 0,
        Type::String | Type::Bytes | Type::Str | Type::SliceU8 => 0,
        Type::Named { name, arguments } => name
            .capacity()
            .saturating_add(arguments.capacity() * std::mem::size_of::<Type>())
            .saturating_add(arguments.iter().map(ast_type_owned_capacity).sum::<usize>()),
    }
}

#[cfg(test)]
pub(super) fn scope_owned_capacity(scope: &VerifierScope) -> usize {
    scope
        .bindings
        .capacity()
        .saturating_mul(std::mem::size_of::<(String, Binding)>())
        .saturating_add(
            scope
                .bindings
                .iter()
                .fold(0usize, |bytes, (name, binding)| {
                    bytes + name.capacity() + binding_owned_capacity(binding)
                }),
        )
}
