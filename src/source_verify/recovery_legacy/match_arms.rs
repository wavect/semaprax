use crate::ast::{MatchMode, MatchPattern, ParamMode};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::diagnostics::{error, source_identifier};
use crate::source_verify::loans::merge_moved;
use crate::source_verify::place::{join_definitely_partial, join_moved_places};
use crate::source_verify::private_names::missing_case;
use crate::source_verify::scope::{VariantMatchState, VerifierFrame, VerifierScope};
use crate::source_verify::type_table::TypeTable;
use crate::source_verify::variant_or::{
    check_variant_or_pattern, VariantOrContext, AGGREGATE_REFUTABLE_HELP,
};
use crate::source_verify::IterativeVerifier;
use std::collections::{BTreeSet, HashMap, HashSet};
impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    pub(in crate::source_verify) fn recovery_old_frame_prepare_variant_match_arm(
        &mut self,
        mut state: VariantMatchState<'p>,
    ) -> Result<(), Diagnostic> {
        if state.index >= state.arms.len() {
            if !state.wildcard_seen {
                if let (Some(variant_name), Some(cases)) =
                    (state.variant_name.as_ref(), state.declared_cases)
                {
                    if let Some(missing) = cases
                        .iter()
                        .find(|case| !state.covered.contains(&case.name))
                    {
                        self.diagnostics.push(missing_case(
                            self.program,
                            self.types,
                            variant_name,
                            &missing.name,
                            !missing.fields.is_empty(),
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
                state.wildcard_seen = arm.guard.is_none();
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
                } else if state.wildcard_seen
                    || state.covered.contains(case_name)
                    || (arm.guard.is_none() && !state.covered.insert(case_name.clone()))
                {
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
                                contract_local: false,
                                traversal_row: None,
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
            MatchPattern::Or { alternatives, span } => {
                let context = VariantOrContext {
                    variant_name: state.variant_name.as_deref(),
                    declared_cases: state.declared_cases,
                    mode: state.mode,
                    wildcard_seen: state.wildcard_seen,
                };
                let mut covered = state.covered.clone();
                check_variant_or_pattern(
                    self.program,
                    alternatives,
                    *span,
                    &context,
                    &mut |case| covered.insert(case.to_owned()),
                    self.diagnostics,
                );
                if arm.guard.is_none() {
                    state.covered = covered;
                }
            }
            MatchPattern::Literal { span, .. } | MatchPattern::Binding { span, .. } => {
                self.diagnostics.push(
                    error(
                        self.program,
                        "SPX-T254",
                        "refutable patterns are incompatible with an aggregate variant \
                         scrutinee",
                        *span,
                    )
                    .with_help(AGGREGATE_REFUTABLE_HELP),
                );
            }
        }
        let next = if let Some(guard) = &arm.guard {
            if !crate::source_verify::variant_guards::guard_shape(
                guard,
                &self.scopes[arm_scope].bindings,
            ) {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T254",
                    "Copy variant guards require an ordinary Boolean expression without yield, closure, unsafe or residual propagation",
                    guard.span,
                ));
            }
            state.guard_pending = true;
            guard.as_ref()
        } else {
            &arm.value
        };
        self.frames
            .push(VerifierFrame::ResumeVariantMatchArm { state, arm_scope });
        self.frames.push(VerifierFrame::Enter {
            expression: next,
            scope: arm_scope,
        });
        Ok(())
    }
}
