//! String clone/literal ownership joins the canonical lifecycle inventory.
use super::*;

pub(super) fn owns_clone(expression: &ResolvedExpr) -> bool {
    expression.ty == ResolvedType::String
        && expression.ownership == OwnershipMode::Own
        && matches!(
            expression.kind,
            ResolvedExprKind::String(_) | ResolvedExprKind::Place(_)
        )
}
impl PlanBuilder<'_> {
    /// An owning String read allocates a clone, except the first operand of
    /// an Owned String Loops v1 same-owner append or a direct literal-format
    /// owned Place argument, which transfers its existing generation.
    pub(super) fn owns_string_clone(&self, expression: &ResolvedExpr) -> bool {
        owns_clone(expression)
            && !self.string_owner_moves.contains(&expression.id)
            && !self.string_condition_reads.contains(&expression.id)
    }

    /// A consuming move or condition inspection never initializes its inventory
    /// temporary, but that storage still belongs to the clone's lexical region.
    pub(super) fn assign_moved_string_slot(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
    ) -> Result<(), Diagnostic> {
        // Only a String operand would otherwise have cloned; a moving map
        // reopen operand never had an inventory temporary.
        if expression.ty == ResolvedType::String
            && (self.string_owner_moves.contains(&expression.id)
                || self.string_condition_reads.contains(&expression.id))
        {
            let region = self.blocks[block.0 as usize].region;
            self.expression_slot(expression, region)?;
        }
        Ok(())
    }

    /// Staging a same-owner append operand reserves the binding's position.
    pub(super) fn reserve_string_append(
        &self,
        at: &ExpressionId,
        source: &CleanupPlace,
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        if self.string_appends.contains_key(at)
            && matches!(source.storage, StorageId::Value(_))
            && source.projections.is_empty()
            && state
                .renewals
                .insert(at.clone(), state.live_order.clone())
                .is_some()
        {
            return Err(plan_error("string append reserves its owner twice"));
        }
        Ok(())
    }

    /// Publishing the appended value returns the binding to that position.
    pub(super) fn publish_string_append(
        &self,
        at: &ExpressionId,
        destination: &CleanupPlace,
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        let Some((operand, _)) = self.string_appends.iter().find(|(_, value)| *value == at) else {
            return Ok(());
        };
        if !matches!(destination.storage, StorageId::Value(_))
            || !destination.projections.is_empty()
        {
            return Ok(());
        }
        let history = state
            .renewals
            .remove(operand)
            .ok_or_else(|| plan_error("string append publishes without a reserved owner"))?;
        state.live_order = crate::string_ops::append_publication_order(&history, &state.live_order);
        Ok(())
    }

    pub(super) fn initialize_string(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        mut state: FlowState,
    ) -> Result<EvalResult, Diagnostic> {
        let region = self.blocks[block.0 as usize].region;
        let destination = self
            .expression_slot(expression, region)?
            .ok_or_else(|| plan_error("owned string clone has no temporary slot"))?;
        self.initialize_owned_result(block, expression, destination.clone(), &mut state)?;
        Ok(EvalResult {
            block,
            state,
            owned_source: Some(destination),
        })
    }
}

impl PlanBuilder<'_> {
    pub(super) fn initialize_owned_result(
        &mut self,
        block: BlockId,
        expression: &ResolvedExpr,
        destination: CleanupPlace,
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        let slot = self
            .storage_to_slot
            .get(&destination.storage)
            .and_then(|slot| self.slots.get(slot.0 as usize))
            .ok_or_else(|| plan_error("owned result has no cleanup slot"))?;
        if let FieldLivenessShape::Variant { declaration, .. } = &slot.field_liveness_shape {
            let declaration = declaration.clone();
            self.initialize_variant(
                block,
                expression.id.clone(),
                destination,
                declaration,
                state,
            )
        } else {
            self.initialize(block, expression.id.clone(), destination, state)
        }
    }
}

impl PlanBuilder<'_> {
    /// Recursive-reference twin for scalar matches. Arm and guard child
    /// regions keep String temporaries out of the parent decision state.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lower_scalar_match(
        &mut self,
        expression: &ResolvedExpr,
        scrutinee: &ResolvedExpr,
        arms: &[ResolvedMatchArm],
        decision_start: BlockId,
        branch_state: FlowState,
        region: CleanupRegionId,
        destination: Option<CleanupPlace>,
    ) -> Result<EvalResult, Diagnostic> {
        if arms.is_empty() {
            return Err(plan_error("refutable match has no arms"));
        }
        let entry_state = branch_state.clone();
        let mut decision = decision_start;
        let mut arm_results = Vec::with_capacity(arms.len());
        for (index, arm) in arms.iter().enumerate() {
            let arm_entry = self.new_block(region)?;
            let direct_single_catchall = arms.len() == 1 && arm.guard.is_none();
            let (value_region, value_entry) = if direct_single_catchall {
                (None, arm_entry)
            } else {
                let value_region = self.new_region(region)?;
                let value_entry = self.new_block(value_region)?;
                (Some(value_region), value_entry)
            };
            if index + 1 == arms.len() {
                let edge = self.new_edge(decision, arm_entry, EdgeCondition::Always)?;
                self.terminate(decision, CleanupTerminator::Goto(edge))?;
            } else {
                let next_decision = self.new_block(region)?;
                let arm_index =
                    u32::try_from(index).map_err(|_| plan_error("too many match arms"))?;
                let selected = self.new_edge(
                    decision,
                    arm_entry,
                    EdgeCondition::ArmSelected {
                        scrutinee: scrutinee.id.clone(),
                        arm: arm_index,
                        selected: true,
                    },
                )?;
                let rejected = self.new_edge(
                    decision,
                    next_decision,
                    EdgeCondition::ArmSelected {
                        scrutinee: scrutinee.id.clone(),
                        arm: arm_index,
                        selected: false,
                    },
                )?;
                self.terminate(
                    decision,
                    CleanupTerminator::Branch(vec![selected, rejected]),
                )?;
                decision = next_decision;
            }
            let (value_block, value_state, value_region) = if let Some(guard) = &arm.guard {
                let guard_region = self.new_region(region)?;
                let guard_entry = self.new_block(guard_region)?;
                let guard_edge = self.new_edge(arm_entry, guard_entry, EdgeCondition::Always)?;
                self.terminate(arm_entry, CleanupTerminator::Goto(guard_edge))?;
                let evaluated_guard = self.lower_expr_recursive_reference(
                    guard.as_ref(),
                    guard_entry,
                    branch_state.clone(),
                    guard_region,
                )?;
                if evaluated_guard.owned_source.is_some() {
                    return Err(plan_error(
                        "scalar match guard owns a value, which no admitted program can express",
                    ));
                }
                let (guard_after, guard_state) =
                    self.exit_scope(evaluated_guard.block, evaluated_guard.state, guard_region)?;
                let true_edge = self.new_edge(
                    guard_after,
                    value_entry,
                    EdgeCondition::BooleanResult(guard.id.clone(), true),
                )?;
                let false_edge = self.new_edge(
                    guard_after,
                    decision,
                    EdgeCondition::BooleanResult(guard.id.clone(), false),
                )?;
                self.terminate(
                    guard_after,
                    CleanupTerminator::Branch(vec![true_edge, false_edge]),
                )?;
                (
                    value_entry,
                    guard_state,
                    Some(value_region.ok_or_else(|| {
                        plan_error("guarded scalar match has no arm cleanup region")
                    })?),
                )
            } else {
                if !direct_single_catchall {
                    let value_edge =
                        self.new_edge(arm_entry, value_entry, EdgeCondition::Always)?;
                    self.terminate(arm_entry, CleanupTerminator::Goto(value_edge))?;
                }
                (value_entry, branch_state.clone(), value_region)
            };
            let mut result = self.lower_expr_recursive_reference(
                &arm.value,
                value_block,
                value_state,
                value_region.unwrap_or(region),
            )?;
            if let Some(destination) = destination.clone() {
                let source = result
                    .owned_source
                    .take()
                    .ok_or_else(|| plan_error("owned scalar match arm has no cleanup source"))?;
                self.transfer(
                    result.block,
                    expression.id.clone(),
                    source,
                    destination,
                    &mut result.state,
                    true,
                )?;
            }
            if let Some(value_region) = value_region {
                (result.block, result.state) =
                    self.exit_scope(result.block, result.state, value_region)?;
            }
            arm_results.push(result);
        }
        let mut arm_results = arm_results.into_iter();
        let first = arm_results
            .next()
            .ok_or_else(|| plan_error("refutable match produced no arm result"))?;
        let mut merged_state = first.state.clone();
        let mut completed = vec![first];
        for result in arm_results {
            merged_state = self.merge_states(&merged_state, &result.state)?;
            completed.push(result);
        }
        let direct_single_catchall = arms.len() == 1 && arms[0].guard.is_none();
        if !direct_single_catchall && merged_state != entry_state {
            return Err(plan_error(
                "refutable match changes owned liveness, which the Refutable Match v1 \
                 admission profile forbids",
            ));
        }
        let join = self.new_block(region)?;
        for result in completed {
            let edge = self.new_edge(result.block, join, EdgeCondition::Always)?;
            self.terminate(result.block, CleanupTerminator::Goto(edge))?;
        }
        Ok(EvalResult {
            block: join,
            state: merged_state,
            owned_source: None,
        })
    }
}

/// Retained identity payload and the same per-entry collection allowance used
/// by the builder's other derived maps. This is test-only capacity telemetry;
/// replay's materialization-operation counter remains a separate contract.
#[cfg(test)]
pub(super) fn derived_metadata_capacity(builder: &PlanBuilder<'_>) -> usize {
    builder.string_appends.len()
        * (std::mem::size_of::<(ExpressionId, ExpressionId)>()
            + std::mem::size_of::<BTreeMap<ExpressionId, ExpressionId>>())
        + builder
            .string_appends
            .iter()
            .map(|(operand, call)| operand.as_str().len() + call.as_str().len())
            .sum::<usize>()
        + builder.string_condition_reads.len()
            * (std::mem::size_of::<ExpressionId>() + std::mem::size_of::<BTreeSet<ExpressionId>>())
        + builder
            .string_condition_reads
            .iter()
            .map(|id| id.as_str().len())
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_capacity_counts_derived_string_metadata() {
        let source = "module capacity.string; @id(\"app.main\") fn main() -> i64 { let mut text=\"a\"; while string_len(text)<2 { text=string_concat(text, \"b\"); 0 } 0 }";
        let program =
            crate::parse(source, std::path::Path::new("string-metadata-capacity.spx")).unwrap();
        assert!(crate::verify::verify(&program).is_empty());
        let resolved = crate::hir::resolve(&program).unwrap();
        let mut builder = PlanBuilder::new(&resolved, &resolved.functions[0]).unwrap();
        assert_eq!(builder.string_condition_reads.len(), 1);
        assert_eq!(builder.string_appends.len(), 1);
        let identity_payload = builder
            .string_condition_reads
            .iter()
            .map(|id| id.as_str().len())
            .sum::<usize>();
        let full = builder_nested_capacity(&builder);
        let reads = std::mem::take(&mut builder.string_condition_reads);
        let without_reads = builder_nested_capacity(&builder);
        assert!(full - without_reads > identity_payload);
        let append_payload = builder
            .string_appends
            .iter()
            .map(|(operand, call)| operand.as_str().len() + call.as_str().len())
            .sum::<usize>();
        builder.string_appends.clear();
        let without_metadata = builder_nested_capacity(&builder);
        assert!(without_reads - without_metadata > append_payload);
        builder.string_condition_reads = reads;
        assert_eq!(
            builder_nested_capacity(&builder) - without_metadata,
            full - without_reads
        );
    }
}
