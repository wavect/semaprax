//! Block and match planning for the aggregate walker. These recursive
//! planners live outside the profile root so extending the walker does not
//! grow a budgeted module.

use super::*;

impl FunctionPlan {
    pub(super) fn collect_block(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        statements: &[ResolvedStatement],
        tail: &ResolvedExpr,
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        for statement in statements {
            let ResolvedStatement::Let { binding, value, .. } = statement else {
                // Assignment targets reuse their `let` slot and while
                // statements contribute their condition and body; only
                // evaluated expressions join the local walk.
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        self.collect_expr(program, variant_layouts, child, parameter_count, frame)?;
                    }
                }
                continue;
            };
            self.collect_expr(program, variant_layouts, value, parameter_count, frame)?;
            if is_aggregate(program, &binding.ty)? {
                let (size, align) = aggregate_size_align(program, variant_layouts, &binding.ty)?;
                let offset = frame.allocate(size, align)?;
                if self
                    .aggregate_bindings
                    .insert(binding.id.clone(), offset)
                    .is_some()
                {
                    return Err(error(format!(
                        "duplicate aggregate binding identity `{}`",
                        binding.id
                    )));
                }
            } else {
                let local = self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                if binding.ty == ResolvedType::String {
                    self.owned_strings.insert(local)?;
                }
                if self
                    .scalar_bindings
                    .insert(binding.id.clone(), local)
                    .is_some()
                {
                    return Err(error(format!(
                        "duplicate scalar binding identity `{}`",
                        binding.id
                    )));
                }
            }
        }
        self.collect_expr(program, variant_layouts, tail, parameter_count, frame)
    }

    pub(super) fn collect_match(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        scrutinee: &ResolvedExpr,
        arms: &[crate::hir::ResolvedMatchArm],
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        self.collect_expr(program, variant_layouts, scrutinee, parameter_count, frame)?;
        for arm in arms {
            match &arm.pattern {
                crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
                    for field in fields {
                        let local =
                            self.add_local(parameter_count, scalar_wasm_type(&field.binding.ty)?)?;
                        if self
                            .scalar_bindings
                            .insert(field.binding.id.clone(), local)
                            .is_some()
                        {
                            return Err(error(format!(
                                "duplicate match binding identity `{}`",
                                field.binding.id
                            )));
                        }
                    }
                }
                crate::hir::ResolvedMatchPattern::Record { fields, .. } => self
                    .collect_record_match_bindings(
                        program,
                        variant_layouts,
                        fields,
                        parameter_count,
                        frame,
                    )?,
                crate::hir::ResolvedMatchPattern::Wildcard => {}
                // Refutable Match v1: a binding arm owns one scalar local;
                // literals and or-patterns own nothing.
                crate::hir::ResolvedMatchPattern::Binding(binding) => {
                    let local = self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                    if self
                        .scalar_bindings
                        .insert(binding.id.clone(), local)
                        .is_some()
                    {
                        return Err(error(format!(
                            "duplicate match binding identity `{}`",
                            binding.id
                        )));
                    }
                }
                crate::hir::ResolvedMatchPattern::Literal(_)
                | crate::hir::ResolvedMatchPattern::Or(_) => {}
            }
            if let Some(guard) = &arm.guard {
                self.collect_expr(program, variant_layouts, guard, parameter_count, frame)?;
            }
            self.collect_expr(program, variant_layouts, &arm.value, parameter_count, frame)?;
        }
        Ok(())
    }

    pub(super) fn collect_record_match_bindings(
        &mut self,
        program: &ResolvedProgram,
        variant_layouts: &VariantLayoutCache,
        fields: &[crate::hir::ResolvedRecordMatchPatternField],
        parameter_count: u32,
        frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        for field in fields {
            match &field.pattern {
                crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let duplicate = if is_aggregate(program, &binding.ty)? {
                        let (size, align) =
                            aggregate_size_align(program, variant_layouts, &binding.ty)?;
                        self.aggregate_bindings
                            .insert(binding.id.clone(), frame.allocate(size, align)?)
                            .is_some()
                    } else {
                        let local =
                            self.add_local(parameter_count, scalar_wasm_type(&binding.ty)?)?;
                        self.scalar_bindings
                            .insert(binding.id.clone(), local)
                            .is_some()
                    };
                    if duplicate {
                        return Err(error(format!(
                            "duplicate record match binding identity `{}`",
                            binding.id
                        )));
                    }
                }
                crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                crate::hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => self
                    .collect_record_match_bindings(
                        program,
                        variant_layouts,
                        fields,
                        parameter_count,
                        frame,
                    )?,
            }
        }
        Ok(())
    }
}
