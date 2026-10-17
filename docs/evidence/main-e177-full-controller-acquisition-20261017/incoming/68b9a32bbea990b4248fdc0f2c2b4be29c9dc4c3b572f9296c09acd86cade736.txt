use super::*;
impl HirValidator<'_> {
    pub(in crate::hir) fn validate_function_before_empty_ensures(
        &mut self,
        function: &ResolvedFunction,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        crate::hir::lexical_exit::validate_function(self.program, function, execution)?;
        crate::hir::declared_collection::validate_function_funded(
            &self.program.declarations,
            function,
        )?;
        crate::hir::iterator_loop::validate_function_paid(function)?;
        owner_renewal::validate_function_paid(self.program, self.byte_helpers, function)?;
        crate::hir::yield_aggregate::check_bytes_request_site_count(
            &self.program.declarations,
            function,
        )?;
        if function.return_type == ResolvedType::Unit {
            return Err(hir_error(
                "ordinary resolved functions cannot declare a unit result",
            ));
        }
        self.validate_type(&function.return_type)?;
        if function.return_type.is_compiler_byte_option() {
            return Err(hir_error(format!(
                "function `{}` cannot author Option<u8>; that widening is reserved to byte_get results",
                function.id
            )));
        }
        let permits = self.functions.permits_borrowed(self.program);
        for effect in &function.effects {
            if !permits.contains(effect.as_str()) {
                return Err(hir_error(format!(
                    "function `{}` declares effect `{effect}` which the module does not permit",
                    function.id
                )));
            }
        }
        if unsafe_scan::contains_unsafe_boundary_paid(&function.body)?
            && !permits.contains("unsafe")
        {
            return Err(hir_error(format!(
                "function `{}` contains an unsafe boundary but the module does not declare `permit {{ unsafe }}`",
                function.id
            )));
        }
        let declared_effects = lifecycle::declared_effects(&function.effects)?;
        let mut required_lifecycle_effects = lifecycle::completed::Collector::new(self.program);
        for param in &function.params {
            if param.ownership == OwnershipMode::Own {
                required_lifecycle_effects.extend(&param.ty)?;
            }
        }
        required_lifecycle_effects.extend(&function.return_type)?;
        crate::hir::inspection::calls::try_visit_resolved_calls(
            &function.body,
            &mut |callee, instance, type_arguments| {
                if instance.is_none()
                    && (crate::string_ops::by_id(callee.as_str()).is_some()
                        || crate::str_ops::by_id(callee.as_str()).is_some()
                        || crate::byte_ops::by_id(callee.as_str()).is_some()
                        || box_intrinsic::is_intrinsic_id(callee)
                        || crate::host_io_ops::by_id(callee.as_str()).is_some())
                {
                    // Intrinsic scalar/string results contribute no lifecycle effects.
                    return Ok(());
                }
                if let Some(target) = self.program.resolve_call_target(callee, instance) {
                    required_lifecycle_effects.extend(&target.return_type)
                } else if self.proof_generic_calls {
                    // The generic proof boundary genuinely produces an owned substituted type.
                    let return_type = generic_template::proof_return_type(
                        self.program,
                        callee,
                        instance,
                        type_arguments,
                    )?;
                    required_lifecycle_effects.extend(&return_type)
                } else {
                    Err(hir_error(format!("function `{callee}` is not indexed")))
                }
            },
        )?;
        if let Some(effect) = required_lifecycle_effects
            .iter()
            .find(|effect| !declared_effects.contains(effect.as_str()))
        {
            return Err(hir_error(format!(
                "function `{}` omits lifecycle effect `{effect}`",
                function.id
            )));
        }
        let mut scope = scope_checkpoint::Scope::default();
        for (index, param) in function.params.iter().enumerate() {
            if param.ty == ResolvedType::Unit {
                return Err(hir_error(
                    "ordinary resolved functions cannot declare a unit parameter",
                ));
            }
            reject_nul_identity("resolved value", param.id.as_str())?;
            if !param.id.matches_parameter(execution, index) {
                return Err(hir_error(format!(
                    "parameter {} of `{}` has a non-canonical identity",
                    index, function.id
                )));
            }
            self.insert_value(&param.id)?;
            self.validate_type(&param.ty)?;
            if param.ty.is_compiler_byte_option() {
                return Err(hir_error(format!(
                    "parameter `{}` cannot author compiler-owned Option<u8>",
                    param.id
                )));
            }
            self.validate_declared_ownership(&param.ty, param.ownership)?;
            if param.ty == ResolvedType::SliceU8
                && (param.ownership != OwnershipMode::Borrow
                    || self
                        .program
                        .declarations
                        .byte_slice_provenance(&param.id)
                        .is_none())
            {
                return Err(hir_error(format!(
                    "byte-slice parameter `{}` lacks exact symbolic parameter-root borrow provenance",
                    param.id
                )));
            }
            if param.ty == ResolvedType::SliceU8 {
                self.byte_slice_aliases.insert(
                    param.id.clone(),
                    Place {
                        root: param.id.clone(),
                        projections: Vec::new(),
                    },
                );
            }
            if param.ty == ResolvedType::Str {
                self.borrowed_str_aliases.insert(
                    param.id.clone(),
                    Place {
                        root: param.id.clone(),
                        projections: Vec::new(),
                    },
                );
            }
            let insertion = scope.prepare_insert()?;
            scope_checkpoint::charge::clone_type(&param.ty)?;
            insertion.insert(
                param.id.clone(),
                ValidationBinding {
                    contract_local: false,
                    mutable: false,
                    ty: param.ty.clone(),
                    ownership: param.ownership,
                    availability: Availability::Available,
                    active_loans: BTreeSet::new(),
                    moved_places: BTreeMap::new(),
                    definitely_partial: BTreeSet::new(),
                },
            )?;
        }
        reject_nul_identity("resolved value", function.result_id.as_str())?;
        if !function.result_id.matches_result(execution) {
            return Err(hir_error(format!(
                "function `{}` has a non-canonical result identity",
                function.id
            )));
        }
        self.insert_value(&function.result_id)?;
        if function.return_type == ResolvedType::Str {
            return Err(hir_error(format!(
                "function `{}` cannot return borrowed `str`",
                function.id
            )));
        }
        if function.return_type == ResolvedType::SliceU8 {
            return Err(hir_error(format!(
                "function `{}` cannot return borrowed `Slice<u8>`",
                function.id
            )));
        }
        self.validate_contracts_persistent(execution, &function.requires, &scope, "requires")?;
        self.validate_expr_persistent(
            execution,
            &function.body,
            &mut scope,
            "body",
            true,
            Some(effect_keys::Allowed::Borrowed(&declared_effects)),
        )?;
        if function.body.ty != ResolvedType::Never {
            self.require_type(&function.body.ty, &function.return_type, "function body")?;
        }
        let returned = self.expected_ownership(&function.return_type, OwnershipMode::Own)?;
        if function.body.ty != ResolvedType::Never && function.body.ownership != returned {
            return Err(hir_error(format!(
                "function `{}` body has invalid return ownership",
                function.id
            )));
        }

        let mut ensures_scope = scope;
        let insertion = ensures_scope.prepare_insert()?;
        scope_checkpoint::charge::clone_type(&function.return_type)?;
        insertion.insert(
            function.result_id.clone(),
            ValidationBinding {
                contract_local: false,
                mutable: false,
                ty: function.return_type.clone(),
                ownership: returned,
                availability: Availability::Available,
                active_loans: BTreeSet::new(),
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        )?;
        self.validate_contracts_persistent(
            execution,
            &function.ensures,
            &ensures_scope,
            "ensures",
        )?;
        Ok(())
    }
}
