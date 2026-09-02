//! The iterative verifier's frame loop: pops verifier frames and dispatches
//! each to the handler that owns its concern.

use crate::ast::Expr;
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::CheckedValue;
#[cfg(test)]
use crate::source_verify::high_water::{
    ast_type_owned_capacity, note_capacity_high_water, scope_owned_capacity,
};
use crate::source_verify::scope::VerifierFrame;
#[cfg(test)]
use crate::source_verify::scope::{
    diagnostics_owned_capacity, verifier_frame_owned_capacity, VerifierScope,
};
use crate::source_verify::IterativeVerifier;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::collapsible_else_if)]
    pub(in crate::source_verify) fn run(
        &mut self,
        expression: &'p Expr,
    ) -> Result<Option<CheckedValue>, Diagnostic> {
        self.frames.push(VerifierFrame::Enter {
            expression,
            scope: 0,
        });
        while let Some(frame) = self.frames.pop() {
            #[cfg(test)]
            note_capacity_high_water(
                self.frames.capacity() * std::mem::size_of::<VerifierFrame<'_>>()
                    + self.scopes.capacity() * std::mem::size_of::<VerifierScope>()
                    + self.values.capacity() * std::mem::size_of::<Option<CheckedValue>>()
                    + self
                        .values
                        .iter()
                        .flatten()
                        .map(|value| ast_type_owned_capacity(&value.ty))
                        .sum::<usize>()
                    + self.scopes.iter().map(scope_owned_capacity).sum::<usize>()
                    + self
                        .frames
                        .iter()
                        .map(verifier_frame_owned_capacity)
                        .sum::<usize>()
                    + verifier_frame_owned_capacity(&frame)
                    + diagnostics_owned_capacity(self.diagnostics),
            );
            match frame {
                VerifierFrame::Enter { expression, scope } => {
                    self.frame_enter(expression, scope)?
                }
                VerifierFrame::ResumeUnary {
                    expression,
                    operand,
                    op,
                } => self.frame_resume_unary(expression, operand, op)?,
                VerifierFrame::ResumeBinaryLeft {
                    expression,
                    op,
                    right,
                    scope,
                } => self.frame_resume_binary_left(expression, op, right, scope)?,
                VerifierFrame::ResumeBinaryRight {
                    expression,
                    op,
                    left,
                    left_value,
                    scope,
                    evaluated_scope,
                    baseline_names,
                } => self.frame_resume_binary_right(
                    expression,
                    op,
                    left,
                    left_value,
                    scope,
                    evaluated_scope,
                    baseline_names,
                )?,
                VerifierFrame::ResumeIfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                } => self.frame_resume_if_condition(expression, then_branch, else_branch, scope)?,
                VerifierFrame::ResumeIfThen {
                    expression,
                    else_branch,
                    scope,
                    then_scope,
                    baseline_names,
                } => self.frame_resume_if_then(
                    expression,
                    else_branch,
                    scope,
                    then_scope,
                    baseline_names,
                )?,
                VerifierFrame::ResumeIfElse {
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                    else_scope,
                    baseline_names,
                    then_value,
                    then_bindings,
                } => self.frame_resume_if_else(
                    expression,
                    then_branch,
                    else_branch,
                    scope,
                    else_scope,
                    baseline_names,
                    then_value,
                    then_bindings,
                )?,
                VerifierFrame::ResumeBlockStatement {
                    expression,
                    statements,
                    tail,
                    parent_scope,
                    block_scope,
                    index,
                    outer_names,
                } => self.frame_resume_block_statement(
                    expression,
                    statements,
                    tail,
                    parent_scope,
                    block_scope,
                    index,
                    outer_names,
                )?,
                VerifierFrame::ResumeWhileCondition { condition, .. } => {
                    self.frame_resume_while_condition(condition)?
                }
                VerifierFrame::ResumeWhileBody {
                    expression,
                    statements,
                    tail,
                    parent_scope,
                    block_scope,
                    index,
                    outer_names,
                    statement_span,
                    baseline_names,
                    baseline_bindings,
                    ..
                } => self.frame_resume_while_body(
                    expression,
                    statements,
                    tail,
                    parent_scope,
                    block_scope,
                    index,
                    outer_names,
                    statement_span,
                    baseline_names,
                    baseline_bindings,
                )?,
                VerifierFrame::ResumeBlockTail {
                    parent_scope,
                    block_scope,
                    outer_names,
                } => self.frame_resume_block_tail(parent_scope, block_scope, outer_names)?,
                VerifierFrame::ResumeCallArgument {
                    expression,
                    name,
                    args,
                    scope,
                    index,
                    target,
                    borrowed_bytes_loans,
                } => self.frame_resume_call_argument(
                    expression,
                    name,
                    args,
                    scope,
                    index,
                    target,
                    borrowed_bytes_loans,
                )?,
                VerifierFrame::ResumeMethodReceiver {
                    expression,
                    receiver,
                    method,
                    args,
                    scope,
                } => {
                    self.frame_resume_method_receiver(expression, receiver, method, args, scope)?
                }
                VerifierFrame::ResumeMethodArgument {
                    expression,
                    method,
                    args,
                    scope,
                    index,
                } => self.frame_resume_method_argument(expression, method, args, scope, index)?,
                VerifierFrame::ResumeTry {
                    expression,
                    operand,
                    scope,
                } => self.frame_resume_try(expression, operand, scope)?,
                VerifierFrame::ResumeProject {
                    expression,
                    base,
                    field,
                } => self.frame_resume_project(expression, base, field)?,
                VerifierFrame::PrepareRecordField {
                    expression,
                    type_name,
                    type_arguments,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                } => self.frame_prepare_record_field(
                    expression,
                    type_name,
                    type_arguments,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                )?,
                VerifierFrame::ResumeRecordField {
                    expression,
                    type_name,
                    type_arguments,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                } => self.frame_resume_record_field(
                    expression,
                    type_name,
                    type_arguments,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                )?,
                VerifierFrame::PrepareVariantField {
                    expression,
                    type_name,
                    type_arguments,
                    case_name,
                    fields,
                    declaration,
                    case,
                    scope,
                    index,
                    supplied,
                } => self.frame_prepare_variant_field(
                    expression,
                    type_name,
                    type_arguments,
                    case_name,
                    fields,
                    declaration,
                    case,
                    scope,
                    index,
                    supplied,
                )?,
                VerifierFrame::ResumeVariantField {
                    expression,
                    type_name,
                    type_arguments,
                    case_name,
                    fields,
                    declaration,
                    case,
                    scope,
                    index,
                    supplied,
                } => self.frame_resume_variant_field(
                    expression,
                    type_name,
                    type_arguments,
                    case_name,
                    fields,
                    declaration,
                    case,
                    scope,
                    index,
                    supplied,
                )?,
                VerifierFrame::ResumeUpdateBase {
                    expression,
                    base,
                    fields,
                    scope,
                } => self.frame_resume_update_base(expression, base, fields, scope)?,
                VerifierFrame::PrepareUpdateField {
                    expression,
                    base_type,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                } => self.frame_prepare_update_field(
                    expression,
                    base_type,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                )?,
                VerifierFrame::ResumeUpdateField {
                    expression,
                    base_type,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                } => self.frame_resume_update_field(
                    expression,
                    base_type,
                    fields,
                    declared_fields,
                    scope,
                    index,
                    supplied,
                )?,
                VerifierFrame::ResumeMatchScrutinee {
                    expression,
                    scrutinee,
                    arms,
                    scope,
                } => self.frame_resume_match_scrutinee(expression, scrutinee, arms, scope)?,
                VerifierFrame::ResumeRecordMatchArm {
                    arm,
                    parent_scope,
                    arm_scope,
                    outer_names,
                } => {
                    self.frame_resume_record_match_arm(arm, parent_scope, arm_scope, outer_names)?
                }
                VerifierFrame::PrepareVariantMatchArm(state) => {
                    self.frame_prepare_variant_match_arm(state)?
                }
                VerifierFrame::ResumeVariantMatchArm { state, arm_scope } => {
                    self.frame_resume_variant_match_arm(state, arm_scope)?
                }
                VerifierFrame::PrepareScalarMatchArm(state) => {
                    self.frame_prepare_scalar_match_arm(state)?
                }
                VerifierFrame::ResumeScalarMatchGuard { state, arm_scope } => {
                    self.frame_resume_scalar_match_guard(state, arm_scope)?
                }
                VerifierFrame::ResumeScalarMatchArm { state, arm_scope } => {
                    self.frame_resume_scalar_match_arm(state, arm_scope)?
                }
            }
        }
        if self.values.len() != 1 {
            return Err(Diagnostic::io(
                "SPX-H006",
                "iterative verifier value stack did not settle",
            ));
        }
        Ok(self.values.pop().expect("value count checked above"))
    }
}
