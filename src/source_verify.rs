//! Source-level verification: ownership, borrow, and type checking over the
//! parsed AST, ahead of HIR resolution.
//!
//! - [`declaration`] runs the program-level passes reached from `verify`, with
//!   the declaration-level checks in [`declared_type`].
//! - [`type_table`] resolves declared types, [`binding`] holds per-variable
//!   ownership state, and [`place`] the projection paths it is keyed by.
//! - [`scope`] declares the verifier's scope and frame state; [`iterative`]
//!   drives the frame loop that checks expressions.
//! - [`loans`], [`arguments`], and [`diagnostics`] own invocation-local loans,
//!   call-boundary ownership, and diagnostic construction; [`hints`] holds the
//!   fix hints both verifiers attach to the same diagnostics.
//! - [`capacity`] projects storage capacity; [`oracle`] is the test-only
//!   recursive cross-check of the frame loop.

#[cfg(test)]
use std::collections::HashSet;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, MatchPattern, Program, RecordMatchFieldPattern, Statement,
    Type, TypeDeclarationKind,
};
#[cfg(test)]
use crate::ast::{Param, ParamMode, Span};
use crate::diagnostic::Diagnostic;

mod arguments;
mod binding;
pub(crate) mod borrowed_closure;
mod capacity;
pub(crate) mod closure;
mod declaration;
mod declared_type;
mod diagnostics;
pub(crate) mod function_value_inventory;
mod generic_inference;
mod hints;
mod iterative;
mod loans;
mod mutable_closure;
mod owned_buffer;
mod owning_closure;
mod place;
mod record_invariants;
mod scope;
mod stdin_stream;
mod type_table;
mod variant_guards;
mod variant_or;

#[cfg(test)]
mod high_water;
#[cfg(test)]
mod oracle;

use binding::{Binding, CheckedValue};
use scope::{VariantMatchState, VerifierFrame, VerifierScope};
use type_table::{resolve_class_method, TypeTable};

pub(crate) use declaration::verify;
pub(crate) use diagnostics::is_scalar_source_type;

pub(crate) fn program_uses_record_iterator(program: &Program) -> bool {
    fn type_uses(program: &Program, ty: &Type) -> bool {
        match ty {
            Type::Named { name, arguments } => {
                (matches!(name.as_str(), "Iter" | "IterStep")
                    && matches!(arguments.as_slice(), [element]
                        if declared_type::owned_record_collection::
                            is_admitted_authored_record_collection_element(program, element)))
                    || arguments
                        .iter()
                        .any(|argument| type_uses(program, argument))
            }
            Type::Function { parameters, result } => {
                parameters
                    .iter()
                    .any(|parameter| type_uses(program, parameter))
                    || type_uses(program, result)
            }
            _ => false,
        }
    }
    let record_element = |ty: &Type| {
        declared_type::owned_record_collection::is_admitted_authored_record_collection_element(
            program, ty,
        )
    };
    let function_uses = |function: &Function| {
        if type_uses(program, &function.return_type)
            || function.params.iter().any(|p| type_uses(program, &p.ty))
        {
            return true;
        }
        let mut pending = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if let ExprKind::Call {
                name,
                type_arguments,
                ..
            } = &expression.kind
            {
                if crate::iterator_ops::by_name(name).is_some()
                    && matches!(type_arguments.as_slice(), [element] if record_element(element))
                {
                    return true;
                }
            }
            if let ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                ..
            } = &expression.kind
            {
                if type_name == "IterStep"
                    && matches!(type_arguments.as_slice(), [element] if record_element(element))
                {
                    return true;
                }
            }
            let mut index = 0;
            while let Some(child) = expression.child(index) {
                pending.push(child);
                index += 1;
            }
        }
        false
    };
    program.functions.iter().any(function_uses)
        || program.types.iter().any(|declaration| {
            matches!(&declaration.kind, TypeDeclarationKind::Class { methods, .. }
                if methods.iter().any(function_uses))
        })
}

#[cfg(test)]
use binding::Availability;
#[cfg(test)]
use high_water::ast_type_owned_capacity;
#[cfg(test)]
pub(crate) use high_water::{capacity_high_water, reset_capacity_high_water};
#[cfg(test)]
use oracle::check_expr;

#[cfg(test)]
use capacity::{
    reset_source_capacity_scope_peak, reset_source_transcript_scope_peak,
    source_capacity_expr_type, source_capacity_functions, source_capacity_match_next_scratch_peak,
    source_capacity_scope_live, source_capacity_scope_peak, source_transcript_frame_scratch_peak,
    source_transcript_owned_map_allocations, source_transcript_scope_live,
    source_transcript_scope_peak, source_transcript_source_from_roots,
    source_type_scope_copy_totals, verify_byte_data_capacity, SourceCapacityContext,
};

struct IterativeVerifier<'a, 'p> {
    program: &'p Program,
    current: &'p Function,
    functions: &'p HashMap<&'p str, &'p Function>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    scopes: Vec<VerifierScope>,
    frames: Vec<VerifierFrame<'p>>,
    values: Vec<Option<CheckedValue>>,
    /// Byte spans of the `bytes_set` calls this function admits as a
    /// same-owner re-open, `buffer = bytes_set(buffer, index, value)`. Every
    /// other `bytes_set` buffer operand must still be the enclosing write-once
    /// chain's previous link. Statement scheduling records a site before the
    /// right-hand side is entered, so the call's own admission rule reads it in
    /// the ordinary order.
    buffer_reopen_sites: std::collections::BTreeSet<(usize, usize)>,
    /// How many `while`/`for` conditions or bodies enclose the frame being
    /// checked; Owned String Loops v2 narrows `match` scrutinees there.
    loop_depth: usize,
    /// Exact owned `for own` item bindings whose record payload may be matched
    /// inside the active loop body.
    owned_iterator_items: Vec<String>,
}

/// Declared in the module root, rather than beside the frame loop, because the
/// interop builder pins the verifier frame and variant match state layouts to
/// this file.
impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        program: &'p Program,
        current: &'p Function,
        variables: HashMap<String, Binding>,
        functions: &'p HashMap<&'p str, &'p Function>,
        types: &'p TypeTable<'p>,
        result_type: Option<&'p Type>,
        allow_moves: bool,
        diagnostics: &'a mut Vec<Diagnostic>,
    ) -> Self {
        const { assert!(std::mem::size_of::<VerifierFrame<'static>>() == 320) };
        const { assert!(std::mem::size_of::<VariantMatchState<'static>>() == 312) };
        let local_borrow_count = variables
            .values()
            .filter(|binding| binding.borrow_origin.is_some())
            .count();
        Self {
            program,
            current,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
            scopes: vec![VerifierScope {
                bindings: variables,
                local_borrow_count,
            }],
            frames: Vec::new(),
            values: Vec::new(),
            buffer_reopen_sites: std::collections::BTreeSet::new(),
            loop_depth: 0,
            owned_iterator_items: Vec::new(),
        }
    }

    /// Record one admitted same-owner byte-buffer re-open before its
    /// right-hand side is entered.
    fn note_owned_buffer_reopen(&mut self, statement: &Statement) {
        if let Statement::Assign {
            name,
            field: None,
            value,
            ..
        } = statement
        {
            if crate::byte_ops::is_same_owner_set_shape(value, name) {
                self.buffer_reopen_sites
                    .insert((value.span.start, value.span.end));
            }
        }
    }
}

#[cfg(test)]
#[path = "source_verify/iterative_verifier_tests.rs"]
mod iterative_verifier_tests;

pub(crate) use declared_type::generic_collection::profile as generic_collection_profile;

pub(crate) fn generic_variant_profile(program: &Program, function: &Function) -> bool {
    declared_type::generic_variant::profile(function, &type_table::TypeTable::new(program))
}
