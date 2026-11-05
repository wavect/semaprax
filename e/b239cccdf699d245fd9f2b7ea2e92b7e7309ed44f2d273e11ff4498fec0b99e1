//! The iterative expression verifier's entry point and frame helpers.
//!
//! `IterativeVerifier` itself is declared in the module root, which pins its
//! frame layout. The frame loop lives in [`run`] and the `while` admission
//! rules in [`while_rules`].

use super::binding::{Binding, CheckedValue};
use super::diagnostics::{error, source_identifier};
use super::hints;
use super::scope::VerifierFrame;
use super::type_table::TypeTable;
use super::IterativeVerifier;
use crate::ast::program_source::ProgramSource;
#[cfg(test)]
use crate::ast::Function;
use crate::ast::{Expr, Statement, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

mod blocks;
mod calls;
mod enter;
mod lexical_exit;
mod match_arms;
mod matching;
mod operators;
mod records;
mod run;
mod while_rules;

impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    /// Class Inheritance v1: `true` when consuming `child` as `ancestor`
    /// would leave owned child-declared state behind. The suffix is every
    /// effective child field beyond the ancestor prefix length.
    pub(super) fn upcast_discards_owned_state(&self, child: &str, ancestor: &str) -> bool {
        let child_fields = match self.types.merged_class_fields.get(child) {
            Some(fields) => fields.as_slice(),
            None => return false,
        };
        let parent_len = self
            .types
            .merged_class_fields
            .get(ancestor)
            .map(Vec::as_slice)
            .map(|fields| {
                // The ancestor may be parentless: fall back to its declared
                // field count through the declaration itself.
                fields.len()
            })
            .unwrap_or_else(|| {
                self.types
                    .declaration(ancestor)
                    .map(|declaration| match &declaration.kind {
                        TypeDeclarationKind::Class { fields, .. } => fields.len(),
                        _ => 0,
                    })
                    .unwrap_or(0)
            });
        if child_fields.len() < parent_len {
            return true;
        }
        child_fields[parent_len..]
            .iter()
            .any(|field| self.types.needs_drop(&field.ty))
    }

    /// Queue the next block statement (any kind) or fall through to the
    /// block tail after one statement completes.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn advance_block_statement(
        &mut self,
        expression: &'p Expr,
        statements: &'p [Statement],
        tail: &'p Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
    ) {
        let next = index + 1;
        let Some(next_statement) = statements.get(next) else {
            self.frames.push(VerifierFrame::ResumeBlockTail {
                parent_scope,
                block_scope,
                outer_names,
            });
            self.frames.push(VerifierFrame::Enter {
                expression: tail,
                scope: block_scope,
            });
            return;
        };
        if let Statement::Let {
            name, name_span, ..
        } = next_statement
        {
            if !source_identifier(name) {
                self.diagnostics.push(hints::reserved_local_binding(
                    self.program,
                    name,
                    *name_span,
                ));
            }
        }
        if let Statement::While {
            condition, body, ..
        } = next_statement
        {
            // While statements complete through their own continuation
            // pair; no per-statement value is produced or consumed.
            self.begin_while_statement(
                expression,
                statements,
                tail,
                parent_scope,
                block_scope,
                next,
                outer_names,
                condition,
                body,
            );
            return;
        }
        if let Statement::For {
            item,
            item_span,
            values,
            body,
            ..
        }
        | Statement::ForBorrow {
            item,
            item_span,
            values,
            body,
            ..
        }
        | Statement::ForOwn {
            item,
            item_span,
            values,
            body,
            ..
        } = next_statement
        {
            self.begin_for_statement(
                expression,
                statements,
                tail,
                parent_scope,
                block_scope,
                next,
                outer_names,
                item,
                *item_span,
                values,
                body,
            );
            return;
        }
        self.check_string_replacement_entry(next_statement, block_scope);
        self.note_owned_buffer_reopen(next_statement);
        self.frames.push(VerifierFrame::ResumeBlockStatement {
            expression,
            statements,
            tail,
            parent_scope,
            block_scope,
            index: next,
            outer_names,
            recovery_checkpoint: self.recovery.checkpoint(self.diagnostics),
        });
        self.frames.push(VerifierFrame::Enter {
            expression: next_statement.value(),
            scope: block_scope,
        });
    }

    /// Begin verification of one `while` statement inside the block scope:
    /// run the Bounded While-Loops v1 admission scan once, reject contract
    /// contexts up front, then type-check the condition.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_while_statement(
        &mut self,
        expression: &'p Expr,
        statements: &'p [Statement],
        tail: &'p Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        condition: &'p Expr,
        body: &'p Expr,
    ) {
        if !self.allow_moves {
            self.diagnostics.push(error(
                self.program,
                "SPX-T253",
                "while statements are not allowed in contract expressions",
                condition.span,
            ));
        }
        self.loop_depth += 1;
        let _ = self.reject_while_disallowed(condition);
        let _ = self.reject_while_disallowed(body);
        let baseline_names = self.scopes[block_scope]
            .bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let baseline_bindings = self.scopes[block_scope].bindings.clone();
        self.frames.push(VerifierFrame::ResumeWhileBody {
            expression,
            statements,
            tail,
            parent_scope,
            block_scope,
            index,
            outer_names,
            statement_span: condition.span.merge(body.span),
            baseline_names,
            baseline_bindings: baseline_bindings.clone(),
        });
        // The body is an ordinary block in the loop's scope: it verifies
        // with its own child scope after the condition, and
        // ResumeWhileBody discards its value and checks ownership drift.
        self.frames.push(VerifierFrame::Enter {
            expression: body,
            scope: block_scope,
        });
        self.frames.push(VerifierFrame::ResumeWhileCondition {
            condition,
            block_scope,
            baseline_bindings,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: condition,
            scope: block_scope,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_for_statement(
        &mut self,
        expression: &'p Expr,
        statements: &'p [Statement],
        tail: &'p Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        item: &'p str,
        item_span: crate::ast::Span,
        values: &'p Expr,
        body: &'p Expr,
    ) {
        self.frames.push(VerifierFrame::ResumeForSource {
            expression,
            statements,
            tail,
            parent_scope,
            block_scope,
            index,
            outer_names,
            item,
            item_span,
            values,
            body,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: values,
            scope: block_scope,
        });
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr_iterative(
    program: &dyn ProgramSource,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &crate::source_verify::function_map::FunctionMap<'_>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    check_expr_with_helpers(
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
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr_with_helpers<'a, 'p>(
    program: &'p dyn ProgramSource,
    current: &'p Function,
    expr: &'p Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &'p crate::source_verify::function_map::FunctionMap<'p>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    helpers: Option<&'a dyn crate::loop_calls::byte_writer::SourceHelpers>,
    fused_producer: Option<&'p Expr>,
) -> Option<CheckedValue> {
    check_expr_with_helpers_view(
        program,
        current,
        expr,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
        helpers,
        fused_producer,
    )
    .0
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn check_expr_with_helpers_view<'a, 'p>(
    program: &'p dyn ProgramSource,
    current: &'p Function,
    expr: &'p Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &'p crate::source_verify::function_map::FunctionMap<'p>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    helpers: Option<&'a dyn crate::loop_calls::byte_writer::SourceHelpers>,
    fused_producer: Option<&'p Expr>,
) -> (
    Option<CheckedValue>,
    Option<super::recovery::RefusalView<'p>>,
) {
    check_expr_with_signature_view(
        program,
        current,
        expr,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
        helpers,
        fused_producer,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(super) fn check_expr_with_signature_view<'a, 'p>(
    program: &'p dyn ProgramSource,
    current: &'p Function,
    expr: &'p Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &'p crate::source_verify::function_map::FunctionMap<'p>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    helpers: Option<&'a dyn crate::loop_calls::byte_writer::SourceHelpers>,
    fused_producer: Option<&'p Expr>,
    signature: Option<super::declaration::SignatureOrigin<'p>>,
) -> (
    Option<CheckedValue>,
    Option<super::recovery::RefusalView<'p>>,
) {
    let current = crate::ast::program_source::FunctionRef::ordinary(current);
    check_expr_with_signature_cell(
        program,
        &current,
        expr,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
        helpers,
        fused_producer,
        signature,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
pub(super) fn check_expr_with_signature_view_ref<'a, 'p>(
    program: &'p dyn ProgramSource,
    current: crate::ast::program_source::FunctionRef<'p>,
    expr: &'p Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &'p crate::source_verify::function_map::FunctionMap<'p>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    helpers: Option<&'a dyn crate::loop_calls::byte_writer::SourceHelpers>,
    fused_producer: Option<&'p Expr>,
    signature: Option<super::declaration::SignatureOrigin<'p>>,
) -> (
    Option<CheckedValue>,
    Option<super::recovery::RefusalView<'p>>,
) {
    check_expr_with_signature_cell(
        program,
        &current,
        expr,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
        helpers,
        fused_producer,
        signature,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr_with_signature_cell<'a, 'p>(
    program: &'p dyn ProgramSource,
    current: &crate::ast::program_source::FunctionRef<'p>,
    expr: &'p Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &'p crate::source_verify::function_map::FunctionMap<'p>,
    types: &'p TypeTable<'p>,
    result_type: Option<&'p Type>,
    allow_moves: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    helpers: Option<&'a dyn crate::loop_calls::byte_writer::SourceHelpers>,
    fused_producer: Option<&'p Expr>,
    signature: Option<super::declaration::SignatureOrigin<'p>>,
) -> (
    Option<CheckedValue>,
    Option<super::recovery::RefusalView<'p>>,
) {
    let initial = std::mem::take(variables);
    let mut verifier = IterativeVerifier::new_ref(
        program,
        current,
        initial,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
    // The public body producer holds the exact authored template before its
    // validation-only scalar substitutions clear the type-parameter names.
    // Authenticate the original receipt against that environment and the real
    // specialized bindings; no source lookup or span-only inference is used.
    if let Some(recovery) = signature.as_ref().and_then(|origin| {
        super::recovery::Recovery::signature_origin_ref(
            program,
            current,
            &verifier.scopes[0].bindings,
            types,
            verifier.diagnostics,
            origin,
            result_type,
        )
    }) {
        verifier.recovery = recovery;
        super::declaration::record_shape::seed(
            &mut verifier.recovery,
            program,
            current.payload(),
            &verifier.scopes[0].bindings,
            types,
            verifier.diagnostics,
        );
    }
    verifier.byte_helpers = helpers;
    verifier.fused_producer = fused_producer;
    let result = verifier.run(expr);
    let refusal = verifier.recovery.view(expr);
    *variables = verifier
        .scopes
        .first_mut()
        .map(|scope| std::mem::take(&mut scope.bindings))
        .unwrap_or_default();
    drop(verifier);
    match result {
        Ok(value) => (value, refusal),
        Err(diagnostic) => {
            diagnostics.push(diagnostic.at_path(program.path()));
            (None, None)
        }
    }
}

impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    fn byte_helper_target(&self, target: &crate::ast::program_source::FunctionRef<'p>) -> bool {
        self.byte_helpers.map_or_else(
            || crate::loop_calls::byte_writer::SourceIndex::new(self.program).target_ref(target),
            |index| index.target_ref(target),
        )
    }
    pub(in crate::source_verify) fn byte_helper_assignment(
        &self,
        name: &str,
        value: &Expr,
    ) -> bool {
        self.byte_helpers.map_or_else(
            || crate::loop_calls::byte_writer::source_assignment(self.program, name, value),
            |index| index.assignment(name, value),
        )
    }
}
