//! Structural grammar; independently specialized source checks authenticate ownership.
use crate::ast::{Expr, Function};
use crate::source_verify::type_table::TypeTable;
pub(super) fn is_admitted(function: &Function, types: &TypeTable<'_>, expression: &Expr) -> bool {
    super::generic_function_has_owned_record_composition(function, types)
        && super::generic_function_expression_is_admitted(expression, true, false)
}
