//! Block and `while` frames: statement sequencing, block tails, and the
//! loop-invariance checks a `while` body must satisfy.

use crate::ast::{Expr, ParamMode, Span, Statement, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::diagnostics::{error, is_scalar_source_type, reject_native_unit_value};
use crate::source_verify::loans::{
    activate_local_loan, has_active_overlapping_loan, local_borrow_origin,
    mark_value_sources_moved, merge_moved, release_dead_local_loans,
};
use crate::source_verify::IterativeVerifier;
use std::collections::{BTreeSet, HashMap, HashSet};

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_block_statement(
        &mut self,
        expression: &'p Expr,
        statements: &'p [Statement],
        tail: &'p Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        match &statements[index] {
            Statement::Let {
                name,
                name_span,
                mutable,
                declared,
                value,
                ..
            } => {
                if self.scopes[block_scope].bindings.contains_key(name) {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T209",
                        format!("local binding `{name}` shadows an existing value"),
                        *name_span,
                    ));
                } else if let Some(actual) = actual {
                    if actual.ty == Type::SliceU8 && *mutable {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T264",
                            format!(
                                "byte-slice alias `{name}` must be immutable and cannot be reassigned"
                            ),
                            *name_span,
                        ));
                    }
                    // Class Inheritance v1: a declared type accepts
                    // the value's exact type or an ancestor class
                    // whose prefix consumes the value cleanly; the
                    // binding carries the declared type.
                    let mut binding_ty = actual.ty.clone();
                    let mut binding_mode = actual.mode;
                    if let Some(declared_ty) = declared {
                        let exact = actual.ty == *declared_ty;
                        let mut upcast = false;
                        if !exact {
                            if let (
                                Type::Named { name: child, .. },
                                Type::Named { name: ancestor, .. },
                            ) = (&actual.ty, declared_ty)
                            {
                                if self.types.class_extends(child, ancestor) {
                                    if self.upcast_discards_owned_state(child, ancestor) {
                                        self.diagnostics.push(error(
                                            self.program,
                                            "SPX-T233",
                                            format!(
                                                "upcast from `{child}` to `{ancestor}` would discard owned state; only cleanup-inert child fields are admitted in this slice"
                                            ),
                                            value.span,
                                        ));
                                    }
                                    upcast = true;
                                }
                            }
                        }
                        if exact || upcast {
                            binding_ty = declared_ty.clone();
                            binding_mode = if *declared_ty == Type::SliceU8 {
                                ParamMode::Borrow
                            } else if self.types.needs_drop(declared_ty)
                                || declared_ty.is_uniquely_owned()
                            {
                                ParamMode::Own
                            } else {
                                ParamMode::Value
                            };
                        } else {
                            let diagnostic = if matches!(
                                (&actual.ty, declared_ty),
                                (Type::ArrayU8(_), Type::ArrayU8(_))
                            ) {
                                "SPX-T262"
                            } else {
                                "SPX-T232"
                            };
                            self.diagnostics.push(error(
                                self.program,
                                diagnostic,
                                format!(
                                    "declared binding type `{declared_ty}` does not accept value type `{}`",
                                    actual.ty
                                ),
                                value.span,
                            ));
                        }
                    }
                    if self.types.needs_drop(&actual.ty) && actual.mode == ParamMode::Own {
                        if self.allow_moves {
                            mark_value_sources_moved(
                                self.program,
                                value,
                                &mut self.scopes[block_scope].bindings,
                                self.types,
                                self.diagnostics,
                            );
                        } else {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-O105",
                                "contract expression cannot transfer an owned resource into a local binding",
                                value.span,
                            ));
                        }
                    }
                    let borrow_origin = matches!(binding_ty, Type::SliceU8 | Type::Str)
                        .then(|| {
                            local_borrow_origin(
                                value,
                                name,
                                *name_span,
                                &self.scopes[block_scope].bindings,
                                self.types,
                            )
                        })
                        .flatten();
                    if let Some(origin) = &borrow_origin {
                        activate_local_loan(&mut self.scopes[block_scope].bindings, origin);
                    }
                    self.scopes[block_scope].bindings.insert(
                        name.clone(),
                        Binding {
                            ty: binding_ty,
                            mode: binding_mode,
                            availability: Availability::Available,
                            moved_places: HashMap::new(),
                            definitely_partial: HashSet::new(),
                            native_unit_discard: actual.native_unit,
                            mutable: *mutable,
                            active_loans: BTreeSet::new(),
                            borrow_origin,
                        },
                    );
                }
            }
            Statement::Assign { value, .. } if !self.allow_moves => {
                // Contract expressions stay pure: no stores.
                self.diagnostics.push(error(
                    self.program,
                    "SPX-U106",
                    "assignment statements are not allowed in contract expressions",
                    value.span,
                ));
            }
            Statement::Assign {
                name,
                name_span,
                field,
                value,
                ..
            } => {
                let assigned_projections = field
                    .iter()
                    .map(|field| field.name.clone())
                    .collect::<Vec<_>>();
                let target = self.scopes[block_scope]
                    .bindings
                    .get(name.as_str())
                    .map(|binding| {
                        (
                            binding.mutable,
                            binding.ty.clone(),
                            has_active_overlapping_loan(binding, &assigned_projections),
                        )
                    });
                if target.is_none() {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T202",
                        format!("unknown value `{name}` in `{}`", self.current.name),
                        *name_span,
                    ));
                }
                if let Some((mutable, binding_ty, has_active_loans)) = target {
                    if has_active_loans {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T265",
                            "assignment would replace storage held by a lexical byte view",
                            *name_span,
                        ));
                    }
                    match field {
                        Some(field) => {
                            // Field Mutation v1: one direct scalar
                            // Copy field of a `let mut` record or
                            // class local.
                            if !mutable {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-U107",
                                    format!(
                                        "cannot assign to field of immutable binding `{name}`; declare it with `let mut`"
                                    ),
                                    *name_span,
                                ));
                            }
                            match self.types.record_fields(&binding_ty) {
                                None => {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-U112",
                                        format!(
                                            "cannot mutate a field of non-record value `{binding_ty}`"
                                        ),
                                        field.span,
                                    ));
                                }
                                Some(fields) => {
                                    let declared = fields
                                        .iter()
                                        .find(|candidate| candidate.name == field.name);
                                    if let Some(declared) = declared {
                                        let field_ty = self
                                            .types
                                            .record_field_type(&binding_ty, declared)
                                            .unwrap_or_else(|| declared.ty.clone());
                                        if !is_scalar_source_type(&field_ty) {
                                            self.diagnostics.push(error(
                                                self.program,
                                                "SPX-U109",
                                                "field mutation v1 supports only direct scalar Copy record fields",
                                                field.span,
                                            ));
                                        }
                                        if let Some(actual) = &actual {
                                            if actual.ty != field_ty {
                                                self.diagnostics.push(error(
                                                    self.program,
                                                    "SPX-U110",
                                                    format!(
                                                        "assigned value type `{}` does not exactly match field type `{}`",
                                                        actual.ty, field_ty
                                                    ),
                                                    value.span,
                                                ));
                                            }
                                        }
                                    } else {
                                        self.diagnostics.push(error(
                                            self.program,
                                            "SPX-U108",
                                            format!(
                                                "record `{binding_ty}` has no field `{}`",
                                                field.name
                                            ),
                                            field.span,
                                        ));
                                    }
                                }
                            }
                        }
                        None => {
                            if !mutable {
                                self.diagnostics.push(error(
                                    self.program,
                                    "SPX-U101",
                                    format!(
                                        "cannot assign to immutable binding `{name}`; declare it with `let mut`"
                                    ),
                                    *name_span,
                                ));
                            }
                            if let Some(actual) = &actual {
                                if mutable && actual.ty != binding_ty {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-U102",
                                        format!(
                                            "assigned value type `{}` does not exactly match binding type `{}`",
                                            actual.ty, binding_ty
                                        ),
                                        value.span,
                                    ));
                                }
                                if mutable
                                    && (actual.mode != ParamMode::Value
                                        || !is_scalar_source_type(&actual.ty))
                                {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-U105",
                                        "explicit mutation v1 supports only scalar Copy values",
                                        value.span,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            Statement::Unsafe { body, span, .. } => {
                // Unsafe Boundary Mechanics v1: the module must
                // explicitly permit the `unsafe` capability and
                // the discarded body result must be a scalar Copy
                // value. Contract expressions reject boundaries.
                if !self.allow_moves {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-N105",
                        "unsafe boundary statements are not allowed in contract expressions",
                        *span,
                    ));
                } else {
                    if !self.program.permits.iter().any(|permit| permit == "unsafe") {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-N101",
                            "unsafe block requires the module capability declaration `permit { unsafe }`",
                            *span,
                        ));
                    }
                    if let Some(actual) = &actual {
                        if actual.mode != ParamMode::Value || !is_scalar_source_type(&actual.ty) {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-N104",
                                "unsafe boundary bodies must produce a scalar Copy value",
                                body.span,
                            ));
                        }
                    }
                }
            }
            // While statements never route through this frame:
            // they complete through ResumeWhileBody instead.
            Statement::While { .. } => {}
        }
        release_dead_local_loans(
            &mut self.scopes[block_scope].bindings,
            statements.get(index + 1..).unwrap_or_default(),
            tail,
        );
        self.advance_block_statement(
            expression,
            statements,
            tail,
            parent_scope,
            block_scope,
            index,
            outer_names,
        );
        Ok(())
    }

    pub(super) fn frame_resume_while_condition(
        &mut self,
        condition: &'p Expr,
    ) -> Result<(), Diagnostic> {
        // The condition is re-evaluated before every iteration and
        // must be exactly `bool`.
        let condition_value = self.values.pop().unwrap_or(None);
        if let Some(value) = condition_value {
            if value.native_unit {
                reject_native_unit_value(self.program, condition, &value, self.diagnostics);
            } else if value.ty != Type::Bool {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T251",
                    "`while` condition must be bool",
                    condition.span,
                ));
            }
        }
        // The body is an ordinary block; it verifies with its own
        // child scope and its value is discarded by
        // ResumeWhileBody.
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_while_body(
        &mut self,
        expression: &'p Expr,
        statements: &'p [Statement],
        tail: &'p Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        statement_span: Span,
        baseline_names: Vec<String>,
        baseline_bindings: HashMap<String, Binding>,
    ) -> Result<(), Diagnostic> {
        // Discard the body block's value: while statements
        // produce none. The body block merged its own child scope
        // through ResumeBlockTail. Because the v1 admission
        // profile admits only Copy-scalar operations inside the
        // loop, every outer binding must be exactly as available
        // as it was on entry; any drift means a move happened
        // inside the loop and is rejected fail-closed.
        let _ = self.values.pop();
        for name in &baseline_names {
            let drifted = match (
                self.scopes[block_scope].bindings.get(name),
                baseline_bindings.get(name),
            ) {
                (Some(now), Some(before)) => {
                    now.availability != before.availability
                        || now.moved_places != before.moved_places
                        || now.definitely_partial != before.definitely_partial
                }
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };
            if drifted {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T252",
                    format!(
                        "ownership of `{name}` changes inside a while loop, which is not yet admitted"
                    ),
                    statement_span,
                ));
            }
        }
        let _ = baseline_names;
        release_dead_local_loans(
            &mut self.scopes[block_scope].bindings,
            statements.get(index + 1..).unwrap_or_default(),
            tail,
        );
        self.advance_block_statement(
            expression,
            statements,
            tail,
            parent_scope,
            block_scope,
            index,
            outer_names,
        );
        Ok(())
    }

    pub(super) fn frame_resume_block_tail(
        &mut self,
        parent_scope: usize,
        block_scope: usize,
        outer_names: Vec<String>,
    ) -> Result<(), Diagnostic> {
        if block_scope + 1 != self.scopes.len() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "block verifier scope is not the active child",
            ));
        }
        let actual = self.values.pop().unwrap_or(None);
        let block_bindings = self
            .scopes
            .pop()
            .expect("active block scope index checked above")
            .bindings;
        merge_moved(
            &mut self.scopes[parent_scope].bindings,
            &block_bindings,
            &outer_names,
        );
        self.values.push(actual);
        Ok(())
    }
}
