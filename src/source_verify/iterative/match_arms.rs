//! Record, variant, and scalar match arm frames: per-arm scope handling and
//! the joins that produce the match's value.

use crate::ast::{MatchMode, MatchPattern, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::diagnostics::{
    error, reject_aggregate_match_result, reject_native_unit_value, source_identifier,
};
use crate::source_verify::loans::merge_moved;
use crate::source_verify::place::{join_definitely_partial, join_moved_places};
use crate::source_verify::scope::{
    ScalarMatchState, VariantMatchState, VerifierFrame, VerifierScope,
};
use crate::source_verify::type_table::TypeTable;
use crate::source_verify::IterativeVerifier;
use std::collections::{BTreeSet, HashMap, HashSet};

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn frame_resume_record_match_arm(
        &mut self,
        arm: &'p crate::ast::MatchArm,
        parent_scope: usize,
        arm_scope: usize,
        outer_names: Vec<String>,
    ) -> Result<(), Diagnostic> {
        if arm_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "record match arm scope is not the active child",
            ));
        }
        let result = self.values.pop().unwrap_or(None);
        if let Some(value) = &result {
            reject_native_unit_value(self.program, &arm.value, value, self.diagnostics);
        }
        let arm_bindings = self
            .scopes
            .pop()
            .expect("record arm scope is active")
            .bindings;
        merge_moved(
            &mut self.scopes[parent_scope].bindings,
            &arm_bindings,
            &outer_names,
        );
        if result.as_ref().is_some_and(|value| {
            !matches!(value.ty, Type::I64 | Type::Bool) || value.mode != ParamMode::Value
        }) {
            self.diagnostics.push(error(
                self.program,
                "SPX-T216",
                "record match arm must return a Copy i64 or bool value",
                arm.value.span,
            ));
            self.values.push(None);
        } else {
            self.values.push(result);
        }
        Ok(())
    }

    pub(super) fn frame_prepare_variant_match_arm(
        &mut self,
        mut state: VariantMatchState<'p>,
    ) -> Result<(), Diagnostic> {
        if state.index >= state.arms.len() {
            if !state.wildcard_seen {
                if let (Some(variant_name), Some(cases)) =
                    (&state.variant_name, state.declared_cases)
                {
                    if let Some(missing) = cases
                        .iter()
                        .find(|case| !state.covered.contains(&case.name))
                    {
                        let witness = if missing.fields.is_empty() {
                            format!("{variant_name}::{} {{}}", missing.name)
                        } else {
                            format!("{variant_name}::{} {{ .. }}", missing.name)
                        };
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M101",
                            format!("non-exhaustive match; missing case `{witness}`"),
                            state.expression.span,
                        ));
                    }
                }
            }
            if let Some((first, rest)) = state.arm_states.split_first() {
                let mut joined = first.clone();
                for branch in rest {
                    for name in &state.outer_names {
                        if let (Some(joined_binding), Some(branch_binding)) =
                            (joined.get_mut(name), branch.get(name))
                        {
                            joined_binding.availability = joined_binding
                                .availability
                                .join(branch_binding.availability);
                            joined_binding.moved_places =
                                join_moved_places(joined_binding, branch_binding);
                            joined_binding.definitely_partial =
                                join_definitely_partial(joined_binding, branch_binding);
                        }
                    }
                }
                merge_moved(
                    &mut self.scopes[state.parent_scope].bindings,
                    &joined,
                    &state.outer_names,
                );
            }
            self.values.push(state.result);
            return Ok(());
        }
        let arm = &state.arms[state.index];
        let arm_scope = self.scopes.len();
        self.scopes.push(VerifierScope {
            bindings: state.baseline.clone(),
            local_borrow_count: state
                .baseline
                .values()
                .filter(|binding| binding.borrow_origin.is_some())
                .count(),
        });
        match &arm.pattern {
            MatchPattern::Wildcard { span } => {
                if state.mode != MatchMode::Value {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O117",
                        "explicit ownership variant match requires every case pattern",
                        *span,
                    ));
                }
                if state.wildcard_seen
                    || state
                        .declared_cases
                        .is_some_and(|cases| state.covered.len() == cases.len())
                {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-M102",
                        "unreachable wildcard match arm",
                        *span,
                    ));
                }
                state.wildcard_seen = true;
            }
            MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                span,
                ..
            } => {
                let compatible = state.variant_name.as_deref() == Some(type_name);
                let declared_case = compatible
                    .then_some(state.declared_cases)
                    .flatten()
                    .and_then(|cases| cases.iter().find(|case| case.name == *case_name));
                if declared_case.is_none() {
                    let diagnostic = error(
                        self.program,
                        "SPX-M103",
                        format!("pattern `{type_name}::{case_name}` is incompatible with the match scrutinee"),
                        *span,
                    );
                    self.diagnostics.push(
                        match compatible
                            .then_some(state.declared_cases)
                            .flatten()
                            .and_then(|cases| {
                                crate::source_verify::hints::nearest_variant_case_name(
                                    case_name, cases,
                                )
                            }) {
                            Some(nearest) => diagnostic.with_help(format!(
                                "did you mean `{type_name}::{nearest} {{ ... }}`?"
                            )),
                            None => diagnostic,
                        },
                    );
                } else if state.wildcard_seen || !state.covered.insert(case_name.clone()) {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-M102",
                        format!("unreachable duplicate case `{type_name}::{case_name}`"),
                        *span,
                    ));
                }
                let mut supplied = HashSet::new();
                let mut bindings = HashSet::new();
                for field in fields {
                    let declared_field = declared_case.and_then(|case| {
                        case.fields
                            .iter()
                            .find(|candidate| candidate.name == field.name)
                    });
                    if !supplied.insert(field.name.as_str())
                        || (declared_case.is_some() && declared_field.is_none())
                    {
                        self.diagnostics.push(error(self.program, "SPX-M104", format!("unknown or duplicate pattern field `{}` in `{type_name}::{case_name}`", field.name), field.span));
                    }
                    if !source_identifier(&field.binding)
                        || !bindings.insert(field.binding.as_str())
                        || self.scopes[arm_scope].bindings.contains_key(&field.binding)
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M104",
                            format!("invalid or duplicate pattern binding `{}`", field.binding),
                            field.binding_span,
                        ));
                        continue;
                    }
                    if let Some(declared_field) = declared_field {
                        let binding_ty = state
                            .variant_name
                            .as_ref()
                            .and_then(|name| {
                                self.types.declaration(name).and_then(|declaration| {
                                    TypeTable::substitute_variant_type(
                                        declaration,
                                        &state.variant_arguments,
                                        &declared_field.ty,
                                    )
                                })
                            })
                            .unwrap_or_else(|| declared_field.ty.clone());
                        let binding_mode = if self.types.needs_drop(&binding_ty) {
                            match state.mode {
                                MatchMode::Own => ParamMode::Own,
                                MatchMode::Borrow => ParamMode::Borrow,
                                MatchMode::Value => ParamMode::Value,
                            }
                        } else {
                            ParamMode::Value
                        };
                        self.scopes[arm_scope].bindings.insert(
                            field.binding.clone(),
                            Binding {
                                ty: binding_ty,
                                mode: binding_mode,
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
                if let Some(declared_case) = declared_case {
                    for field in &declared_case.fields {
                        if !supplied.contains(field.name.as_str()) {
                            self.diagnostics.push(error(self.program, "SPX-M104", format!("pattern `{type_name}::{case_name}` is missing payload field `{}`", field.name), *span));
                        }
                    }
                }
            }
            MatchPattern::Record { span, .. } => self.diagnostics.push(error(
                self.program,
                "SPX-M103",
                "record pattern is incompatible with a variant scrutinee",
                *span,
            )),
            MatchPattern::Literal { span, .. }
            | MatchPattern::Or { span, .. }
            | MatchPattern::Binding { span, .. } => {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T254",
                    "refutable patterns are incompatible with an aggregate variant \
                     scrutinee",
                    *span,
                ));
            }
        }
        self.frames
            .push(VerifierFrame::ResumeVariantMatchArm { state, arm_scope });
        self.frames.push(VerifierFrame::Enter {
            expression: &arm.value,
            scope: arm_scope,
        });
        Ok(())
    }

    pub(super) fn frame_resume_variant_match_arm(
        &mut self,
        mut state: VariantMatchState<'p>,
        arm_scope: usize,
    ) -> Result<(), Diagnostic> {
        if arm_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "variant match arm scope is not the active child",
            ));
        }
        let arm = &state.arms[state.index];
        let arm_value = self.values.pop().unwrap_or(None);
        if let Some(value) = &arm_value {
            reject_native_unit_value(self.program, &arm.value, value, self.diagnostics);
            reject_aggregate_match_result(self.program, &arm.value, value, self.diagnostics);
        }
        if let Some(arm_value) = arm_value {
            if state.needs_drop
                && (state.mode == MatchMode::Value
                    || !matches!(arm_value.ty, Type::I64 | Type::Bool)
                    || arm_value.mode != ParamMode::Value)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T216",
                    "owned variant match arms must return a Copy i64 or bool value",
                    arm.value.span,
                ));
            }
            if let Some(expected) = &state.result {
                if expected.ty != arm_value.ty || expected.mode != arm_value.mode {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T216",
                        format!(
                            "match arms return incompatible values: {} and {}",
                            expected.ty, arm_value.ty
                        ),
                        arm.value.span,
                    ));
                }
            } else {
                state.result = Some(arm_value);
            }
        }
        state.arm_states.push(
            self.scopes
                .pop()
                .expect("variant arm scope is active")
                .bindings,
        );
        state.index += 1;
        self.frames
            .push(VerifierFrame::PrepareVariantMatchArm(state));
        Ok(())
    }

    pub(super) fn frame_prepare_scalar_match_arm(
        &mut self,
        state: ScalarMatchState<'p>,
    ) -> Result<(), Diagnostic> {
        if state.index >= state.arms.len() {
            if let Some((first, rest)) = state.arm_states.split_first() {
                let mut joined = first.clone();
                for branch in rest {
                    for name in &state.outer_names {
                        if let (Some(joined_binding), Some(branch_binding)) =
                            (joined.get_mut(name), branch.get(name))
                        {
                            joined_binding.availability = joined_binding
                                .availability
                                .join(branch_binding.availability);
                            joined_binding.moved_places =
                                join_moved_places(joined_binding, branch_binding);
                            joined_binding.definitely_partial =
                                join_definitely_partial(joined_binding, branch_binding);
                        }
                    }
                }
                merge_moved(
                    &mut self.scopes[state.parent_scope].bindings,
                    &joined,
                    &state.outer_names,
                );
            }
            self.values.push(state.result);
            return Ok(());
        }
        let arm = &state.arms[state.index];
        let arm_scope = self.scopes.len();
        self.scopes.push(VerifierScope {
            bindings: state.baseline.clone(),
            local_borrow_count: state
                .baseline
                .values()
                .filter(|binding| binding.borrow_origin.is_some())
                .count(),
        });
        match &arm.pattern {
            MatchPattern::Wildcard { .. } => {}
            MatchPattern::Binding { name, span } => {
                if !source_identifier(name) || self.scopes[arm_scope].bindings.contains_key(name) {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-M104",
                        format!("invalid or duplicate pattern binding `{name}`"),
                        *span,
                    ));
                } else {
                    self.scopes[arm_scope].bindings.insert(
                        name.clone(),
                        Binding {
                            ty: state.scrutinee_ty.clone(),
                            mode: ParamMode::Value,
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
            // Literal/or diagnostics fired during admission; the
            // arm value still checks so downstream errors stay
            // deterministic.
            _ => {}
        }
        match &arm.guard {
            Some(guard) => {
                self.frames
                    .push(VerifierFrame::ResumeScalarMatchGuard { state, arm_scope });
                self.frames.push(VerifierFrame::Enter {
                    expression: guard.as_ref(),
                    scope: arm_scope,
                });
            }
            None => {
                self.frames
                    .push(VerifierFrame::ResumeScalarMatchArm { state, arm_scope });
                self.frames.push(VerifierFrame::Enter {
                    expression: &arm.value,
                    scope: arm_scope,
                });
            }
        }
        Ok(())
    }

    pub(super) fn frame_resume_scalar_match_guard(
        &mut self,
        state: ScalarMatchState<'p>,
        arm_scope: usize,
    ) -> Result<(), Diagnostic> {
        if arm_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "scalar match guard scope is not the active child",
            ));
        }
        let guard_value = self.values.pop().unwrap_or(None);
        let arm = &state.arms[state.index];
        if let Some(value) = &guard_value {
            if value.ty != Type::Bool || value.mode != ParamMode::Value {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T256",
                    format!("match guard must be bool; received {}", value.ty),
                    arm.guard.as_ref().map_or(arm.span, |guard| guard.span),
                ));
            }
        }
        // The arm value observes the guard's moves within this
        // arm only.
        self.frames
            .push(VerifierFrame::ResumeScalarMatchArm { state, arm_scope });
        self.frames.push(VerifierFrame::Enter {
            expression: &arm.value,
            scope: arm_scope,
        });
        Ok(())
    }

    pub(super) fn frame_resume_scalar_match_arm(
        &mut self,
        mut state: ScalarMatchState<'p>,
        arm_scope: usize,
    ) -> Result<(), Diagnostic> {
        if arm_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "scalar match arm scope is not the active child",
            ));
        }
        let arm = &state.arms[state.index];
        let arm_value = self.values.pop().unwrap_or(None);
        if let Some(value) = &arm_value {
            reject_native_unit_value(self.program, &arm.value, value, self.diagnostics);
            reject_aggregate_match_result(self.program, &arm.value, value, self.diagnostics);
        }
        if let Some(arm_value) = arm_value {
            if let Some(expected) = &state.result {
                if expected.ty != arm_value.ty || expected.mode != arm_value.mode {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T259",
                        format!(
                            "match arms return incompatible values: {} and {}",
                            expected.ty, arm_value.ty
                        ),
                        arm.value.span,
                    ));
                }
            } else {
                state.result = Some(arm_value);
            }
        }
        state.arm_states.push(
            self.scopes
                .pop()
                .expect("scalar arm scope is active")
                .bindings,
        );
        state.index += 1;
        self.frames
            .push(VerifierFrame::PrepareScalarMatchArm(state));
        Ok(())
    }
}
