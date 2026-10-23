//! Borrowed source names only: ordinary FFI resolution remains authoritative.
use super::{reserve_builder_structure, Census};
use crate::ast::{
    ImportDeclaration, ImportFailure, ImportResult, InterfaceDeclaration, Param, Program, Type,
};
use crate::diagnostic::Diagnostic;
use std::mem::size_of;
use std::slice::Iter;

type Answer = Result<(), Vec<Diagnostic>>;
// Each reservation includes its own reserve_builder_structure argument/result
// boundary. No old census backing or payload slot funds a new receiver.
const PAYMENT: usize = 2 * size_of::<usize>() + 2 * size_of::<Answer>();
pub(super) const CALL: usize = PAYMENT
    + 2 * size_of::<&mut Census<'_>>()
    + 2 * size_of::<&Program>()
    + 2 * size_of::<Answer>();
const NAME: usize = PAYMENT
    + 2 * size_of::<&mut Census<'_>>()
    + 2 * size_of::<&String>()
    + 4 * size_of::<&str>()
    + 2 * size_of::<Answer>();
const TYPE: usize =
    PAYMENT + 2 * size_of::<&mut Census<'_>>() + 2 * size_of::<&Type>() + 2 * size_of::<Answer>();
const CHARGE: usize =
    2 * size_of::<&mut Census<'_>>() + 2 * size_of::<usize>() + 2 * size_of::<Answer>();

// SETUP pays Vec deref, slice loan, slice::iter and returned/received iterator.
// STEP is renewed before every advance, including terminal None; the extracted
// reference is a separate receiver from its enclosing Option payload.
macro_rules! walk {
    ($c:ident, $items:expr, $ty:ty, $item:ident, $body:block) => {{
        reserve_builder_structure(
            PAYMENT
                + 2 * size_of::<&Vec<$ty>>()
                + 4 * size_of::<&[$ty]>()
                + 2 * size_of::<Iter<'_, $ty>>(),
        )?;
        let mut iter = $items.iter();
        loop {
            reserve_builder_structure(
                PAYMENT
                    + CHARGE
                    + 2 * size_of::<&mut Iter<'_, $ty>>()
                    + 2 * size_of::<Option<&$ty>>()
                    + 2 * size_of::<&$ty>(),
            )?;
            $c.charge(1)?;
            let Some($item) = iter.next() else { break };
            $body
        }
    }};
}
macro_rules! name {
    ($c:ident, $value:expr) => {{
        reserve_builder_structure(NAME)?;
        $c.name($value)?;
    }};
}

macro_rules! optional_name {
    ($c:ident, $value:expr) => {{
        reserve_builder_structure(
            PAYMENT + CHARGE + 2 * size_of::<&Option<String>>() + 2 * size_of::<&String>(),
        )?;
        $c.charge(1)?;
        if let Some(value) = $value {
            name!($c, value);
        }
    }};
}

pub(super) fn scan<'a>(census: &mut Census<'a>, caller: &'a Program) -> Answer {
    walk!(
        census,
        caller.interfaces,
        InterfaceDeclaration,
        interface,
        {
            name!(census, &interface.name);
            name!(census, &interface.stable_id);
            walk!(census, interface.permits, String, permit, {
                name!(census, permit);
            });
            walk!(census, interface.imports, ImportDeclaration, import, {
                name!(census, &import.name);
                name!(census, &import.stable_id);
                // Each optional metadata field is a distinct source occurrence.
                optional_name!(census, &import.selected_signature);
                optional_name!(census, &import.selected_index_digest);
                optional_name!(census, &import.selected_receiver);
                optional_name!(census, &import.rust_path);
                walk!(census, import.params, Param, parameter, {
                    name!(census, &parameter.name);
                    reserve_builder_structure(TYPE)?;
                    census.ty(&parameter.ty)?;
                });
                reserve_builder_structure(
                    PAYMENT + CHARGE + 2 * size_of::<&ImportResult>() + 2 * size_of::<&String>(),
                )?;
                census.charge(1)?;
                match &import.result {
                    ImportResult::OwnedResource { name }
                    | ImportResult::OwnedResultResourceI64 { name } => name!(census, name),
                    ImportResult::BorrowedStr { owner } => name!(census, owner),
                    ImportResult::Unit
                    | ImportResult::I64
                    | ImportResult::Bool
                    | ImportResult::ResultI64I64
                    | ImportResult::OwnedString
                    | ImportResult::OwnedOptionString
                    | ImportResult::OwnedResultStringI64
                    | ImportResult::OwnedResultStringOptionI64 => {}
                }
                walk!(census, import.effects, String, effect, {
                    name!(census, effect);
                });
                reserve_builder_structure(
                    PAYMENT + CHARGE + 2 * size_of::<&ImportFailure>() + 2 * size_of::<&String>(),
                )?;
                census.charge(1)?;
                if let ImportFailure::Status { domain_id } = &import.failure {
                    name!(census, domain_id);
                }
                name!(census, &import.consumes);
            });
        }
    );
    Ok(())
}
