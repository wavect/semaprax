//! The iterative expression verifier's entry point and frame helpers.
//!
//! `IterativeVerifier` itself is declared in the module root, which pins its
//! frame layout. The frame loop lives in [`run`] and the `while` admission
//! rules in [`while_rules`].

use super::binding::{Binding, CheckedValue};
use super::diagnostics::{error, source_identifier};
use super::scope::VerifierFrame;
use super::type_table::TypeTable;
use super::IterativeVerifier;
use crate::ast::{Expr, Function, Program, Statement, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::collections::HashMap;

mod blocks;
mod calls;
mod enter;
mod match_arms;
mod matching;
mod operators;
mod records;
mod run;
mod while_rules;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
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
                self.diagnostics.push(error(
                    self.program,
                    "SPX-S109",
                    format!("`{name}` is reserved and cannot name a local binding"),
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
        self.frames.push(VerifierFrame::ResumeBlockStatement {
            expression,
            statements,
            tail,
            parent_scope,
            block_scope,
            index: next,
            outer_names,
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
            baseline_bindings,
        });
        self.frames
            .push(VerifierFrame::ResumeWhileCondition { condition });
        self.frames.push(VerifierFrame::Enter {
            expression: condition,
            scope: block_scope,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_expr_iterative(
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    let initial = std::mem::take(variables);
    let mut verifier = IterativeVerifier::new(
        program,
        current,
        initial,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
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
            diagnostics.push(diagnostic.at_path(&program.path));
            None
        }
    }
}
