//! Exact owned `Result<Bytes, E>` postfix-`?` cleanup construction.

use super::*;

impl PlanBuilder<'_> {
    pub(super) fn merge_owned_try_residual_states(
        &self,
        state: &mut FlowState,
        residuals: &[PendingTryResidual],
    ) -> Result<(), Diagnostic> {
        if !state.live_order.is_empty()
            || state.conditional_variants.len() != 1
            || state.conditional_variants[0].root
                != CleanupPlace::whole(StorageId::ProvisionalResult)
        {
            return Err(plan_error(
                "owned postfix `?` normal path retains unrelated live owners",
            ));
        }
        for residual in residuals {
            if !residual.state.live_order.is_empty()
                || residual.state.conditional_variants.len() != 1
                || residual.state.conditional_variants[0].root
                    != CleanupPlace::whole(StorageId::ProvisionalResult)
            {
                return Err(plan_error(
                    "owned postfix `?` residual retains unrelated live owners",
                ));
            }
            *state = self.merge_states(state, &residual.state)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_owned_try(
        &mut self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        result: &DeclarationId,
        ok_case: &DeclarationId,
        ok_field: &DeclarationId,
        evaluated: EvalResult,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        schema::promote_v6(&mut self.schema);
        let source = evaluated.owned_source.ok_or_else(|| {
            plan_error("owned postfix `?` operand has no conditional cleanup source")
        })?;
        let success = self.new_block(region)?;
        let residual = self.new_block(region)?;
        let success_edge = self.new_edge(
            evaluated.block,
            success,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: ok_case.clone(),
                matches: true,
            },
        )?;
        let residual_edge = self.new_edge(
            evaluated.block,
            residual,
            EdgeCondition::VariantCase {
                scrutinee: operand.id.clone(),
                case: ok_case.clone(),
                matches: false,
            },
        )?;
        self.terminate(
            evaluated.block,
            CleanupTerminator::Branch(vec![success_edge, residual_edge]),
        )?;

        let mut success_state = evaluated.state.clone();
        self.authenticate_variant_case(
            success,
            expression.id.clone(),
            &source,
            result,
            ok_case,
            &mut success_state,
        )?;
        let success_destination = if self.needs_drop(&expression.ty)? {
            let destination = self
                .expression_slot(expression, region)?
                .ok_or_else(|| plan_error("owned postfix `?` has no success cleanup slot"))?;
            self.transfer(
                success,
                expression.id.clone(),
                source
                    .projected(ok_case.clone())
                    .projected(ok_field.clone()),
                destination.clone(),
                &mut success_state,
                false,
            )?;
            Some(destination)
        } else {
            None
        };

        let mut residual_state = evaluated.state;
        self.transfer(
            residual,
            expression.id.clone(),
            source,
            CleanupPlace::whole(StorageId::ProvisionalResult),
            &mut residual_state,
            true,
        )?;
        self.pending_try_residuals.push(PendingTryResidual {
            block: residual,
            state: residual_state,
            region,
        });
        Ok(EvalResult {
            block: success,
            state: success_state,
            owned_source: success_destination,
        })
    }
}

impl PlanBuilder<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn check_try_metadata(
        &self,
        expression: &ResolvedExpr,
        operand: &ResolvedExpr,
        result: &DeclarationId,
        ok_case: &DeclarationId,
        ok_field: &DeclarationId,
        err_case: &DeclarationId,
        err_field: &DeclarationId,
        residual_type: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        if result.as_str() != prelude::RESULT_ID
            || ok_case.as_str() != prelude::RESULT_OK_ID
            || ok_field.as_str() != prelude::RESULT_OK_VALUE_ID
            || err_case.as_str() != prelude::RESULT_ERR_ID
            || err_field.as_str() != prelude::RESULT_ERR_ERROR_ID
        {
            return Err(plan_error(
                "postfix `?` does not authenticate the ordinary Result prelude",
            ));
        }
        for id in [result, ok_case, ok_field, err_case, err_field] {
            let declaration = self
                .program
                .declarations
                .declaration(id)
                .ok_or_else(|| plan_error(format!("postfix `?` references unknown `{id}`")))?;
            if declaration.identity_origin != IdentityOrigin::CompilerOwned {
                return Err(plan_error(format!(
                    "postfix `?` reference `{id}` is not compiler-owned"
                )));
            }
        }
        let source_arguments = result_arguments(&operand.ty, result)?;
        let target_arguments = result_arguments(residual_type, result)?;
        let exact_owned = matches!(
            source_arguments,
            [
                ResolvedType::Bytes,
                ResolvedType::Bytes
                    | ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::Char
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool
            ]
        ) && source_arguments == target_arguments
            || matches!(source_arguments, [success, ResolvedType::Bytes]
                if crate::hir::is_scalar_resolved_type(success))
                && source_arguments == target_arguments;
        if source_arguments.len() != 2
            || target_arguments.len() != 2
            || (!exact_owned
                && source_arguments
                    .iter()
                    .chain(target_arguments.iter())
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)))
            || expression.ty != source_arguments[0]
            || source_arguments[1] != target_arguments[1]
            || residual_type != &self.function.return_type
        {
            return Err(plan_error(
                "postfix `?` has inconsistent source, value, residual, or function types",
            ));
        }
        for ty in [&operand.ty, residual_type] {
            let facts = self
                .program
                .declarations
                .type_facts(ty)
                .ok_or_else(|| plan_error("postfix `?` Result instance has no type facts"))?;
            let valid = if exact_owned {
                !facts.copy && facts.sized && !facts.contains_resource && facts.needs_drop
            } else {
                facts.copy && facts.sized && !facts.contains_resource && !facts.needs_drop
            };
            if !valid {
                return Err(plan_error(
                    "postfix `?` reached cleanup planning outside its exact Result slice",
                ));
            }
        }
        Ok(())
    }
}
