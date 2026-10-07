//! Record Invariants v1: source checks for the `requires` clauses after a
//! record declaration.
//!
//! Each clause is checked exactly like a function precondition whose
//! parameters are the record's fields, by bare name and declared type: it must
//! be `bool` (`SPX-C101`), call only effect-free functions (`SPX-C102`), name
//! no host or command I/O operation (`SPX-T269`, `SPX-T270`), and resolve
//! every name against the fields, the declared functions, and the builtins.
//! Invariants are admitted only on non-generic records (`SPX-C103`).

use super::binding::{Availability, Binding};
use super::diagnostics::{error, require_bool};
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Function, ParamMode, Program, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) fn check_record_invariants(
    program: &Program,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &program.types {
        let invariants = declaration.invariants();
        let Some(first) = invariants.first() else {
            continue;
        };
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            continue;
        };
        if !declaration.type_parameters.is_empty() {
            diagnostics.push(
                error(
                    program,
                    "SPX-C103",
                    format!(
                        "invariants on generic record `{}` are outside Record Invariants v1",
                        declaration.name
                    ),
                    first.span,
                )
                .with_help("state the rule on a non-generic record, or check it in a function"),
            );
            continue;
        }
        // Only direct string owners retain the graph/webapp-only exception.
        // Executable owners cannot carry a clause without a production check.
        if fields
            .iter()
            .any(|field| field.ty != Type::String && types.needs_drop(&field.ty))
        {
            diagnostics.push(error(program, "SPX-C104",
                format!("invariants on owned record `{}` have no ownership-preserving executable check", declaration.name),
                first.span).with_help("use a Copy record for executable invariants, or check scalar observations in a function"));
            continue;
        }
        // The clauses check as preconditions of a function whose parameters
        // are the fields; this context is never resolved or emitted.
        let context = Function {
            stable_id: declaration.stable_id.clone(),
            explicit_id: declaration.explicit_id,
            name: declaration.name.clone(),
            name_span: declaration.name_span,
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::Bool,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Bool(true),
                span: declaration.span,
            },
            span: declaration.span,
        };
        let variables = fields
            .iter()
            .map(|field| {
                let mode = if field.ty.is_uniquely_owned() {
                    ParamMode::Own
                } else {
                    ParamMode::Value
                };
                (
                    field.name.clone(),
                    Binding {
                        ty: field.ty.clone(),
                        mode,
                        availability: Availability::Available,
                        moved_places: HashMap::new(),
                        definitely_partial: HashSet::new(),
                        native_unit_discard: false,
                        mutable: false,
                        active_loans: BTreeSet::new(),
                        borrow_origin: None,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        for invariant in invariants {
            invariant.visit_calls(&mut |callee, span| {
                if crate::host_io_ops::by_name(callee).is_some() {
                    diagnostics.push(error(
                        program,
                        "SPX-T269",
                        "stdout_write is not admitted in contracts",
                        span,
                    ));
                }
                if crate::command_io_ops::by_name(callee).is_some() {
                    diagnostics.push(error(
                        program,
                        "SPX-T270",
                        "command I/O operations are not admitted in contracts",
                        span,
                    ));
                }
            });
            let first_new = diagnostics.len();
            require_bool(
                program,
                &context,
                invariant,
                &variables,
                functions,
                types,
                None,
                diagnostics,
                "invariant",
            );
            for diagnostic in &mut diagnostics[first_new..] {
                if diagnostic.code == "SPX-T202" && diagnostic.help.is_none() {
                    diagnostic.help = Some(format!(
                        "an invariant names the fields of `{}` by bare name: {}",
                        declaration.name,
                        fields
                            .iter()
                            .map(|field| format!("`{}`", field.name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
    }
}
