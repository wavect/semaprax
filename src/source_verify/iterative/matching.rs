//! The match scrutinee frame: classifying the scrutinee and starting the
//! record, variant, or scalar arm state machine.

use crate::ast::{Expr, ExprKind, MatchMode, MatchPattern, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::declared_type::check_record_pattern;
use crate::source_verify::diagnostics::{error, reject_native_unit_value};
use crate::source_verify::loans::{activate_match_loan, mark_value_sources_moved};
use crate::source_verify::place::source_place;
use crate::source_verify::scope::{
    pattern_literal_type, ScalarMatchState, VariantMatchState, VerifierFrame, VerifierScope,
};
use crate::source_verify::IterativeVerifier;
use std::collections::HashSet;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn frame_resume_match_scrutinee(
        &mut self,
        expression: &'p Expr,
        scrutinee: &'p Expr,
        arms: &'p [crate::ast::MatchArm],
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let match_mode = match &expression.kind {
            ExprKind::Match { mode, .. } => *mode,
            _ => unreachable!("match continuation has a non-match expression"),
        };
        let scrutinee_value = self.values.pop().unwrap_or(None);
        if let Some(value) = &scrutinee_value {
            reject_native_unit_value(self.program, scrutinee, value, self.diagnostics);
        }
        // Refutable Match v1: Copy-scalar scrutinees take the
        // literal/guard decision chain; every other type keeps
        // the pre-feature record/variant surfaces below.
        let scalar_scrutinee = match_mode == MatchMode::Value
            && scrutinee_value.as_ref().is_some_and(|value| {
                matches!(
                    value.ty,
                    Type::I64 | Type::I32 | Type::Char | Type::U8 | Type::Usize | Type::Bool
                ) && value.mode == ParamMode::Value
            });
        if scalar_scrutinee {
            let scrutinee_ty = scrutinee_value
                .as_ref()
                .map(|value| value.ty.clone())
                .expect("scalar scrutinee checked above");
            if arms.is_empty() {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-M101",
                    "match has no arms",
                    expression.span,
                ));
                self.values.push(None);
                return Ok(());
            }
            for arm in arms.iter() {
                match &arm.pattern {
                    MatchPattern::Wildcard { .. } | MatchPattern::Binding { .. } => {}
                    MatchPattern::Literal { value, span } => {
                        if pattern_literal_type(*value) != scrutinee_ty {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T255",
                                format!(
                                    "literal pattern of type `{}` cannot match a \
                                     `{}` scrutinee; pattern literals compare \
                                     against exactly their own type",
                                    value.type_text(),
                                    scrutinee_ty
                                ),
                                *span,
                            ));
                        }
                    }
                    MatchPattern::Or { alternatives, span } => {
                        let mut seen_type: Option<Type> = None;
                        for alternative in alternatives {
                            let MatchPattern::Literal { value, span } = alternative else {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-M105",
                                    "or-patterns admit only literal alternatives in v1",
                                    alternative.span(),
                                ));
                                continue;
                            };
                            if pattern_literal_type(*value) != scrutinee_ty {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-T255",
                                    format!(
                                        "literal pattern of type `{}` cannot match a \
                                         `{}` scrutinee; pattern literals compare \
                                         against exactly their own type",
                                        value.type_text(),
                                        scrutinee_ty
                                    ),
                                    *span,
                                ));
                            }
                            match &seen_type {
                                None => seen_type = Some(pattern_literal_type(*value)),
                                Some(seen) if *seen == pattern_literal_type(*value) => {}
                                Some(seen) => {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-M105",
                                        format!(
                                            "or-pattern mixes `{seen}` and `{}` literal \
                                             alternatives",
                                            pattern_literal_type(*value)
                                        ),
                                        *span,
                                    ));
                                }
                            }
                        }
                        if alternatives.is_empty() {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-M105",
                                "or-pattern needs at least one literal alternative",
                                *span,
                            ));
                        }
                    }
                    MatchPattern::Variant { span, .. } | MatchPattern::Record { span, .. } => {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-M103",
                            "aggregate pattern is incompatible with a Copy-scalar \
                             scrutinee",
                            *span,
                        ));
                    }
                }
            }
            let last = arms.last().expect("empty checked above");
            let catch_all = matches!(
                &last.pattern,
                MatchPattern::Wildcard { .. } | MatchPattern::Binding { .. }
            );
            if !catch_all || last.guard.is_some() {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T257",
                    "refutable match requires a trailing irrefutable catch-all arm \
                     (`_` or a binding) without a guard",
                    last.span,
                ));
            }
            let outer_names = self.scopes[scope]
                .bindings
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            self.frames
                .push(VerifierFrame::PrepareScalarMatchArm(ScalarMatchState {
                    expression,
                    arms,
                    parent_scope: scope,
                    index: 0,
                    scrutinee_ty: scrutinee_ty.clone(),
                    outer_names,
                    baseline: self.scopes[scope].bindings.clone(),
                    arm_states: Vec::new(),
                    result: None,
                }));
            return Ok(());
        }
        let refutable_syntax = arms.iter().any(|arm| {
            arm.guard.is_some()
                || matches!(
                    &arm.pattern,
                    MatchPattern::Literal { .. }
                        | MatchPattern::Or { .. }
                        | MatchPattern::Binding { .. }
                )
        });
        if refutable_syntax {
            self.diagnostics.push(error(
                self.program,
                "SPX-T254",
                "guards and literal/or/binding patterns require a Copy-scalar \
                 scrutinee (i64/i32/u8/char/bool)",
                scrutinee.span,
            ));
        }
        if scrutinee_value
            .as_ref()
            .is_some_and(|value| self.types.record_fields(&value.ty).is_some())
        {
            let scrutinee_value = scrutinee_value.expect("record checked above");
            let needs_drop = self.types.needs_drop(&scrutinee_value.ty);
            if match_mode != MatchMode::Value
                && self.types.is_nested_owned_byte_record(&scrutinee_value.ty)
                && !self.types.is_flat_owned_byte_record(&scrutinee_value.ty)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-O117",
                    "ownership-aware patterns over nested owned-Bytes records remain closed",
                    scrutinee.span,
                ));
            }
            match match_mode {
                MatchMode::Value => {
                    if needs_drop || scrutinee_value.mode != ParamMode::Value {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-O111",
                            "plain record match requires a Copy scrutinee",
                            scrutinee.span,
                        ));
                    }
                }
                MatchMode::Own => {
                    if !needs_drop || scrutinee_value.mode != ParamMode::Own {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-O117",
                            "`match own` requires an owned non-Copy record scrutinee",
                            scrutinee.span,
                        ));
                    } else if self.allow_moves {
                        mark_value_sources_moved(
                            self.program,
                            scrutinee,
                            &mut self.scopes[scope].bindings,
                            self.types,
                            self.diagnostics,
                        );
                    } else {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-O105",
                            "contract expression cannot consume a match scrutinee",
                            scrutinee.span,
                        ));
                    }
                }
                MatchMode::Borrow => {
                    if !needs_drop
                        || !matches!(scrutinee_value.mode, ParamMode::Own | ParamMode::Borrow)
                        || source_place(scrutinee, &self.scopes[scope].bindings, self.types)
                            .is_none_or(|place| !place.projections.is_empty())
                    {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-O117",
                            "`match borrow` requires a named owned or borrowed non-Copy record place",
                            scrutinee.span,
                        ));
                    }
                }
            }
            let Some((first, rest)) = arms.split_first() else {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-M101",
                    format!(
                        "non-exhaustive match; missing record pattern for `{}`",
                        scrutinee_value.ty
                    ),
                    expression.span,
                ));
                self.values.push(None);
                return Ok(());
            };
            for arm in rest {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-M102",
                    "unreachable arm after an irrefutable record pattern",
                    arm.pattern.span(),
                ));
            }
            let outer_names = self.scopes[scope]
                .bindings
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            let arm_scope = self.scopes.len();
            self.scopes.push(VerifierScope {
                bindings: self.scopes[scope].bindings.clone(),
            });
            if match_mode == MatchMode::Borrow {
                if let Some(place) =
                    source_place(scrutinee, &self.scopes[scope].bindings, self.types)
                {
                    activate_match_loan(
                        &mut self.scopes[arm_scope].bindings,
                        &place,
                        expression.span,
                    );
                }
            }
            match &first.pattern {
                MatchPattern::Wildcard { span } if match_mode != MatchMode::Value => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O117",
                        "explicit ownership match requires an exact record pattern",
                        *span,
                    ));
                }
                MatchPattern::Wildcard { .. } => {}
                MatchPattern::Record {
                    type_name,
                    fields,
                    span,
                    ..
                } => check_record_pattern(
                    self.program,
                    type_name,
                    fields,
                    &scrutinee_value.ty,
                    &mut self.scopes[arm_scope].bindings,
                    self.types,
                    self.diagnostics,
                    *span,
                    match_mode,
                ),
                MatchPattern::Variant { .. } => self.diagnostics.push(error(
                    self.program,
                    "SPX-M103",
                    "variant pattern is incompatible with a record scrutinee",
                    first.pattern.span(),
                )),
                MatchPattern::Literal { .. }
                | MatchPattern::Or { .. }
                | MatchPattern::Binding { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T254",
                        "refutable patterns are incompatible with an aggregate \
                         record scrutinee",
                        first.pattern.span(),
                    ));
                }
            }
            self.frames.push(VerifierFrame::ResumeRecordMatchArm {
                arm: first,
                parent_scope: scope,
                arm_scope,
                outer_names,
            });
            self.frames.push(VerifierFrame::Enter {
                expression: &first.value,
                scope: arm_scope,
            });
            return Ok(());
        }
        let variant_instance = scrutinee_value.as_ref().and_then(|value| match &value.ty {
            Type::Named { name, arguments } if self.types.variant_cases(&value.ty).is_some() => {
                Some((name.clone(), arguments.clone()))
            }
            Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::ArrayU8(_)
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::Str
            | Type::SliceU8
            | Type::Named { .. } => None,
        });
        let variant_name = variant_instance.as_ref().map(|(name, _)| name.clone());
        let declared_cases = scrutinee_value
            .as_ref()
            .and_then(|value| self.types.variant_cases(&value.ty));
        if declared_cases.is_none() {
            self.diagnostics.push(error(
                self.program,
                "SPX-M103",
                format!(
                    "match scrutinee must be a Copy variant, received {}",
                    scrutinee_value.as_ref().map_or_else(
                        || "an invalid value".to_owned(),
                        |value| value.ty.to_string()
                    )
                ),
                scrutinee.span,
            ));
        }
        let variant_needs_drop = scrutinee_value
            .as_ref()
            .is_some_and(|value| self.types.needs_drop(&value.ty));
        if let Some(scrutinee_value) = &scrutinee_value {
            match match_mode {
                MatchMode::Value if variant_needs_drop => self.diagnostics.push(error(
                    self.program,
                    "SPX-O111",
                    "plain variant match requires a Copy scrutinee",
                    scrutinee.span,
                )),
                MatchMode::Own
                    if !variant_needs_drop
                        || !self.types.is_flat_owned_byte_variant(&scrutinee_value.ty)
                        || scrutinee_value.mode != ParamMode::Own =>
                {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O117",
                        "`match own` requires an owned admitted non-Copy variant scrutinee",
                        scrutinee.span,
                    ));
                }
                MatchMode::Own if self.allow_moves => mark_value_sources_moved(
                    self.program,
                    scrutinee,
                    &mut self.scopes[scope].bindings,
                    self.types,
                    self.diagnostics,
                ),
                MatchMode::Own => self.diagnostics.push(error(
                    self.program,
                    "SPX-O105",
                    "contract expression cannot consume a match scrutinee",
                    scrutinee.span,
                )),
                MatchMode::Borrow
                    if !variant_needs_drop
                        || !self.types.is_flat_owned_byte_variant(&scrutinee_value.ty)
                        || !matches!(scrutinee_value.mode, ParamMode::Own | ParamMode::Borrow)
                        || source_place(scrutinee, &self.scopes[scope].bindings, self.types)
                            .is_none_or(|place| !place.projections.is_empty()) =>
                {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O117",
                        "`match borrow` requires an unprojected named owned or borrowed admitted non-Copy variant place",
                        scrutinee.span,
                    ));
                }
                MatchMode::Borrow | MatchMode::Value => {}
            }
        }
        let outer_names = self.scopes[scope]
            .bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let mut baseline = self.scopes[scope].bindings.clone();
        if match_mode == MatchMode::Borrow {
            if let Some(place) = source_place(scrutinee, &self.scopes[scope].bindings, self.types) {
                if place.projections.is_empty() {
                    activate_match_loan(&mut baseline, &place, expression.span);
                }
            }
        }
        let state = VariantMatchState {
            expression,
            arms,
            parent_scope: scope,
            index: 0,
            outer_names,
            baseline,
            arm_states: Vec::new(),
            covered: HashSet::new(),
            wildcard_seen: false,
            result: None,
            variant_name,
            variant_arguments: variant_instance
                .map(|(_, arguments)| arguments)
                .unwrap_or_default(),
            declared_cases,
            mode: match_mode,
            needs_drop: variant_needs_drop,
        };
        self.frames
            .push(VerifierFrame::PrepareVariantMatchArm(state));
        Ok(())
    }
}
