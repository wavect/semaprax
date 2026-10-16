use crate::ast::{Expr, ParamMode, Statement, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use crate::source_verify::diagnostics::{error, is_scalar_source_type};
use crate::source_verify::loans::{
    activate_local_loan, has_active_overlapping_loan, local_borrow_origin,
    mark_value_sources_moved, release_dead_local_loans,
};
use crate::source_verify::IterativeVerifier;
use std::collections::{BTreeSet, HashMap, HashSet};
impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::source_verify) fn recovery_old_frame_resume_block_statement(
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
                    self.diagnostics.push(
                        error(
                            self.program,
                            "SPX-T209",
                            format!("local binding `{name}` shadows an existing value"),
                            *name_span,
                        )
                        .with_help(crate::source_verify::hints::SHADOW_HELP),
                    );
                } else if let Some(actual) = actual {
                    if let Some(diagnostic) = crate::source_verify::expression_statement::check(
                        self.program,
                        &statements[index],
                        &actual,
                    ) {
                        self.diagnostics.push(diagnostic);
                    }
                    if matches!(actual.ty, Type::SliceU8 | Type::Str) && *mutable {
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
                            binding_mode = if matches!(declared_ty, Type::SliceU8 | Type::Str) {
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
                            let help = (diagnostic == "SPX-T232")
                                .then(|| {
                                    crate::source_verify::hints::declared_binding_help(
                                        declared_ty,
                                        value,
                                    )
                                })
                                .flatten();
                            self.diagnostics.push(crate::source_verify::hints::with_optional_help(
                                error(
                                    self.program,
                                    diagnostic,
                                    format!(
                                        "declared binding type `{}` does not accept value type `{}`",
                                        // Generic validation specialization
                                        // substitutes `let` annotations, so a
                                        // declared type can carry a private copy.
                                        crate::source_verify::private_names::type_name(
                                            self.program,
                                            self.types,
                                            declared_ty
                                        ),
                                        crate::source_verify::private_names::type_name(
                                            self.program,
                                            self.types,
                                            &actual.ty
                                        )
                                    ),
                                    value.span,
                                ),
                                help,
                            ));
                        }
                    }
                    let contract_local = !self.allow_moves
                        && actual.mode == ParamMode::Own
                        && crate::source_verify::contract_owners::fresh(
                            value,
                            &actual.ty,
                            &self.scopes[block_scope].bindings,
                        );
                    if self.types.needs_drop(&actual.ty) && actual.mode == ParamMode::Own {
                        if self.allow_moves || contract_local {
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
                                self.program,
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
                        self.scopes[block_scope].local_borrow_count += 1;
                    }
                    self.scopes[block_scope].bindings.insert(
                        name.clone(),
                        Binding {
                            contract_local,
                            traversal_row: None,
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
                            // The source verifier runs before HIR has the
                            // enclosing `yields Request -> Response` clause
                            // available to retag this direct expression. Its
                            // initial type is therefore the request type;
                            // defer only this whole-binding mismatch to
                            // `finish_yields_admission`, which checks the
                            // binding against the declared response type.
                            // Field assignments and every non-yield value
                            // retain the ordinary exact-type diagnostic here.
                            let direct_yield =
                                matches!(&value.kind, crate::ast::ExprKind::Yield { .. });
                            // Same-owner replacement: a `Vec<T>` reopen such as
                            // `values = vec_push(values, v)` and the byte-buffer
                            // `buffer = bytes_set(buffer, index, value)` and the
                            // Owned String Loops v1 append
                            // `text = string_concat(text, more)` are the
                            // admitted owned reopen shapes. The right-hand side
                            // evaluates before publication, so exactly one
                            // generation of the owner is ever live.
                            let owned_reopen = (binding_ty == crate::ast::Type::Bytes
                                && self.byte_helper_assignment(name, value))
                                || (binding_ty == crate::ast::Type::String
                                    && actual.as_ref().is_some_and(|v| {
                                        v.ty == crate::ast::Type::String && v.mode == ParamMode::Own
                                    }))
                                || crate::vec_ops::is_same_owner_reassignment_source(
                                    self.program,
                                    value,
                                    name,
                                    &binding_ty,
                                )
                                || crate::byte_ops::is_same_owner_set_source(
                                    value,
                                    name,
                                    &binding_ty,
                                )
                                || crate::string_ops::is_same_owner_concat_source(
                                    value,
                                    name,
                                    &binding_ty,
                                )
                                || (self.loop_depth != 0
                                    && self.admits_record_owner_renewal(name, value))
                                || (crate::stdin_stream_ops::ast_is_reader(&binding_ty)
                                    && crate::stdin_stream_ops::source_next_is_same_owner(
                                        self.program,
                                        name,
                                        value,
                                    ));
                            if !mutable {
                                let mut diagnostic = error(
                                    self.program,
                                    "SPX-U101",
                                    format!(
                                        "cannot assign to immutable binding `{name}`; declare it with `let mut`"
                                    ),
                                    *name_span,
                                );
                                if self
                                    .current
                                    .params
                                    .iter()
                                    .any(|parameter| parameter.name == *name)
                                {
                                    diagnostic = diagnostic.with_help(format!(
                                        "parameters are immutable; copy `{name}` into a new mutable local and use that: `let mut current_{name} = {name};`"
                                    ));
                                }
                                self.diagnostics.push(diagnostic);
                            }
                            if let Some(actual) = &actual {
                                if mutable && actual.ty != binding_ty && !direct_yield {
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
                                        || !crate::source_verify::closure::scoped_scalar(
                                            self.current,
                                            &actual.ty,
                                        ))
                                    && !owned_reopen
                                {
                                    self.diagnostics.push(error(
                                        self.program,
                                        "SPX-U105",
                                        "explicit mutation v1 supports only scalar Copy values",
                                        value.span,
                                    ));
                                }
                            }
                            if mutable && owned_reopen {
                                if binding_ty == Type::String
                                    && !crate::string_ops::is_same_owner_concat_source(
                                        value,
                                        name,
                                        &binding_ty,
                                    )
                                {
                                    mark_value_sources_moved(
                                        self.program,
                                        value,
                                        &mut self.scopes[block_scope].bindings,
                                        self.types,
                                        self.diagnostics,
                                    );
                                }
                                if let Some(binding) =
                                    self.scopes[block_scope].bindings.get_mut(name.as_str())
                                {
                                    binding.availability = Availability::Available;
                                    binding.moved_places.clear();
                                    binding.definitely_partial.clear();
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
                    if !self
                        .program
                        .permits()
                        .iter()
                        .any(|permit| permit == "unsafe")
                    {
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
            Statement::While { .. }
            | Statement::For { .. }
            | Statement::ForBorrow { .. }
            | Statement::ForOwn { .. } => {}
        }
        // Most blocks have no borrowed local at all. Avoid rescanning every
        // accumulated scalar binding after each statement in that common
        // case; the full liveness walk remains exact once a borrow exists.
        if self.scopes[block_scope].local_borrow_count != 0 {
            let released = release_dead_local_loans(
                &mut self.scopes[block_scope].bindings,
                statements.get(index + 1..).unwrap_or_default(),
                tail,
            );
            self.scopes[block_scope].local_borrow_count = self.scopes[block_scope]
                .local_borrow_count
                .saturating_sub(released);
        }
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
}
