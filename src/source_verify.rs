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
mod capacity;
mod declaration;
mod declared_type;
mod diagnostics;
mod hints;
mod iterative;
mod loans;
mod place;
mod scope;
mod type_table;

#[cfg(test)]
mod high_water;
#[cfg(test)]
mod oracle;

use binding::{Binding, CheckedValue};
use scope::{VariantMatchState, VerifierFrame, VerifierScope};
use type_table::{resolve_class_method, TypeTable};

pub(crate) use declaration::verify;
pub(crate) use diagnostics::is_scalar_source_type;

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
            }],
            frames: Vec::new(),
            values: Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "source_verify/iterative_verifier_tests.rs"]
mod iterative_verifier_tests;
