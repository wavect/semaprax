//! Complete original receiving bodies, with test-only names/entry dispatch.
use super::binding::{Availability, Binding, CheckedValue};
use super::diagnostics::{error, reject_native_unit_value};
use super::scope::{VariantMatchState, VerifierFrame, VerifierScope};
use super::type_table::TypeTable;
use super::IterativeVerifier;
use crate::ast::program_source::ProgramSource;
use crate::ast::{Expr, Function, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeSet, HashMap, HashSet};
mod blocks;
mod enter;
mod match_arms;
mod operators;
mod records;
#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr_iterative_legacy(
    program: &dyn ProgramSource,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    check_expr_with_helpers_legacy(
        program,
        current,
        expr,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
        None,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr_with_helpers_legacy<'a, 'p>(
    program: &'p dyn ProgramSource,
    current: &'p Function,
    expr: &'p Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &'p HashMap<&'p str, &'p Function>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    helpers: Option<&'a dyn crate::loop_calls::byte_writer::SourceHelpers>,
    fused_producer: Option<&'p Expr>,
) -> Option<CheckedValue> {
    let initial = std::mem::take(variables);
    let mut verifier = IterativeVerifier::new_legacy(
        program,
        current,
        initial,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
    verifier.byte_helpers = helpers;
    verifier.fused_producer = fused_producer;
    let result = verifier.run(expr);
    *variables = verifier
        .scopes
        .first_mut()
        .map(|scope| std::mem::take(&mut scope.bindings))
        .unwrap_or_default();
    drop(verifier);
    match result {
        Ok(value) => value,
        Err(diagnostic) => {
            diagnostics.push(diagnostic.at_path(program.path()));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn require_bool_legacy(
    program: &dyn ProgramSource,
    function: &Function,
    contract: &Expr,
    variables: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
    kind: &str,
) {
    contract.visit_calls(&mut |callee, span| {
        if variables.contains_key(callee) { return; }
        if let Some(target) = functions.get(callee) {
            if !target.effects.is_empty() {
                diagnostics.push(
                    error(
                        program,
                        "SPX-C102",
                        format!(
                            "{kind} on `{}` calls effectful function `{callee}` with effects {{{}}}",
                            function.name,
                            target.effects.join(", ")
                        ),
                        span,
                    )
                    .with_help("contracts must be deterministic and effect-free"),
                );
            }
        }
    });
    let mut contract_variables = variables.clone();
    // No freshness from a body or previous clause enters this contract domain.
    for binding in contract_variables.values_mut() {
        binding.contract_local = false;
    }
    if let Some(ty) = result_type {
        // A provisional owned result is readable only in this postcondition
        // scope. It stays immutable and contract transfers still select O105.
        if ty == &function.return_type
            && (matches!(ty, Type::Bytes | Type::String)
                || matches!(ty, Type::Named { name, arguments }
                if name == "Result" && arguments.as_slice() == [Type::Bytes, Type::Bytes]))
        {
            contract_variables.insert(
                "result".to_owned(),
                Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: ty.clone(),
                    mode: ParamMode::Own,
                    availability: Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: BTreeSet::new(),
                    borrow_origin: None,
                },
            );
        }
    }
    if let Some(value) = check_expr_iterative_legacy(
        program,
        function,
        contract,
        &mut contract_variables,
        functions,
        types,
        result_type,
        false,
        diagnostics,
    ) {
        if value.native_unit {
            reject_native_unit_value(program, contract, &value, diagnostics);
        } else if value.ty != Type::Bool {
            diagnostics.push(error(
                program,
                "SPX-C101",
                format!("{kind} on `{}` must be bool", function.name),
                contract.span,
            ));
        }
    }
}

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    fn new_legacy(
        program: &'p dyn ProgramSource,
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
            constructor_hygiene: None,
            byte_helpers: None,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
            scopes: vec![VerifierScope {
                bindings: variables,
                local_borrow_count,
            }],
            recovery: super::recovery::Recovery::default(),
            recovery_legacy: true,
            operators_before_799: false,
            extended_before_799: false,
            frames: Vec::new(),
            values: Vec::new(),
            buffer_reopen_sites: std::collections::BTreeSet::new(),
            loop_depth: 0,
            owned_iterator_items: Vec::new(),
            fused_producer: None,
            row_loan_sites: std::collections::BTreeSet::new(),
        }
    }
}
