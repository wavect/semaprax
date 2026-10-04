//! CleanupPlan finalization for owned Wasm carriers.

use super::*;

impl Emitter<'_> {
    pub(super) fn emit_cleanup_actions(
        &mut self,
        actions: &[crate::cleanup_plan::FinalizeAction],
    ) -> Result<(), Diagnostic> {
        for action in actions {
            if matches!(
                action.lifecycle_id.as_str(),
                crate::hir::closure::once::DROP_ID
                    | crate::hir::closure::once::MIXED_DROP_ID
                    | crate::hir::closure::once::PAIR_DROP_ID
            ) {
                self.emit_once_cleanup(action)?;
                continue;
            }
            let string_leaf =
                action.lifecycle_id.as_str() == crate::cleanup::STRING_DROP_LIFECYCLE_ID;
            let vec_leaf = action.lifecycle_id.as_str() == crate::cleanup::VEC_DROP_LIFECYCLE_ID;
            let box_leaf = action.lifecycle_id.as_str() == crate::cleanup::BOX_DROP_LIFECYCLE_ID;
            let iter_leaf = action.lifecycle_id.as_str() == crate::cleanup::ITER_DROP_LIFECYCLE_ID;
            if action.lifecycle_id.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID
                && !string_leaf
                && !vec_leaf
                && !box_leaf
                && !iter_leaf
            {
                return Err(error(
                    "WebAssembly cleanup has an unsupported compiler-owned leaf",
                ));
            }
            let value = self.cleanup_value_at(&action.source)?;
            if string_leaf {
                require_type(
                    value_type(&value),
                    &ResolvedType::String,
                    "CleanupPlan String finalizer",
                )?;
            } else if iter_leaf {
                if !crate::iterator_ops::is_iter(value_type(&value)) {
                    return Err(error(
                        "Iter CleanupPlan finalizer type disagrees with lifecycle",
                    ));
                }
            } else if vec_leaf {
                if !owned_vec(self.program, value_type(&value)) {
                    return Err(error(
                        "Vec CleanupPlan finalizer type disagrees with lifecycle",
                    ));
                }
            } else if box_leaf {
                if !crate::cleanup::is_owned_bounded_box_type(value_type(&value)) {
                    return Err(error(
                        "Box CleanupPlan finalizer type disagrees with lifecycle",
                    ));
                }
            } else {
                require_type(
                    value_type(&value),
                    &ResolvedType::Bytes,
                    "CleanupPlan finalizer",
                )?;
            }
            let flag = self
                .plan
                .cleanup_flags
                .get(&action.guard_flag)
                .copied()
                .ok_or_else(|| error("CleanupPlan finalizer guard has no exact Wasm local"))?;
            self.output.push(0x20);
            write_u32(self.output, flag);
            self.output.extend([0x04, 0x40]);
            if string_leaf {
                let carrier = self
                    .plan
                    .string_cleanup_carrier
                    .ok_or_else(|| error("String CleanupPlan leaf has no Wasm cleanup carrier"))?;
                self.get_scalar(&value);
                self.output.push(0x21);
                write_u32(self.output, carrier);
                self.clear_scalar(&value)?;
                owned_strings::emit_drop(self.output, carrier);
            } else {
                if iter_leaf {
                    let Value::Aggregate { pointer, ty } = &value else {
                        return Err(error("Iter cleanup leaf is not aggregate storage"));
                    };
                    self.emit_pointer(*pointer);
                    self.load_scalar(&ResolvedType::I64);
                    if *ty == crate::iterator_ops::resolved_iter(ResolvedType::Bytes) {
                        self.emit_pointer(Pointer {
                            offset: pointer.offset + iterator_ops::ITER_CURSOR_OFFSET,
                            ..*pointer
                        });
                        self.load_scalar(&ResolvedType::Usize);
                    }
                } else {
                    self.get_scalar(&value);
                }
                self.output.push(0x10);
                write_u32(
                    self.output,
                    if iter_leaf
                        && *value_type(&value)
                            == crate::iterator_ops::resolved_iter(ResolvedType::Bytes)
                    {
                        iterator_ops::owned_import_base(self.program) + 2
                    } else if vec_leaf || iter_leaf {
                        vec_import_base(self.program) + 5
                    } else if box_leaf {
                        box_import_base(self.program) + 3
                    } else {
                        BYTE_DROP_IMPORT
                    },
                );
                if iter_leaf {
                    self.clear_iterator(&value)?;
                } else {
                    self.clear_scalar(&value)?;
                }
            }
            self.semantic_cleanup_event(action.guard_flag.0)?;
            self.output.extend([0x41, 0x00, 0x21]);
            write_u32(self.output, flag);
            self.output.push(0x0b);
        }
        Ok(())
    }
}

impl Emitter<'_> {
    /// Settle one fresh CleanupPlan child region of a scalar match. For a
    /// guard, the Boolean result remains on the Wasm stack while these
    /// finalizers consume only the child region's String temporaries.
    pub(super) fn emit_scalar_match_guard_cleanup(
        &mut self,
        _scalar_match: &ResolvedExpr,
        guard: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let owners = self.string_temporary_owner_regions(guard, "guard")?;
        let regions = owners
            .into_iter()
            .map(|owner| self.guard_cleanup_region(owner, &guard.id))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        if regions.is_empty() {
            return Ok(());
        }
        // A `BTreeSet` with more than one member would mean the source guard
        // has temporaries in separate scalar-match child regions. That is not
        // a shape the authenticated plan can settle at one Wasm program point.
        let mut regions = regions.into_iter();
        let region = regions.next().expect("checked nonempty");
        if regions.next().is_some() {
            return Err(error(
                "String match guard maps to multiple CleanupPlan child regions",
            ));
        }
        self.emit_scalar_match_region_cleanup(region, "guard")
    }

    pub(super) fn emit_scalar_match_value_cleanup(
        &mut self,
        scalar_match: &ResolvedExpr,
        value: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let owners = self.string_temporary_owner_regions(value, "value")?;
        if owners.is_empty() {
            return Ok(());
        }
        let ResolvedExprKind::Match { scrutinee, .. } = &scalar_match.kind else {
            return Err(error("scalar match value cleanup has a non-match parent"));
        };
        let parent = self.scalar_match_parent_region(&scrutinee.id)?;
        let regions = owners
            .into_iter()
            .map(|owner| self.value_cleanup_region(owner, parent))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let mut regions = regions.into_iter();
        let region = regions.next().ok_or_else(|| {
            error("String match value maps to multiple CleanupPlan child regions")
        })?;
        if regions.next().is_some() {
            return Err(error(
                "String match value maps to multiple CleanupPlan child regions",
            ));
        }
        self.emit_scalar_match_region_cleanup(region, "value")
    }

    /// Return the direct owner regions for String temporaries authored under
    /// one scalar-match child. Nested blocks and matches have their own
    /// regions, so callers must select an ancestor rather than assuming all
    /// descendants share one slot list.
    fn string_temporary_owner_regions(
        &self,
        expression: &ResolvedExpr,
        child_kind: &str,
    ) -> Result<std::collections::BTreeSet<crate::cleanup_plan::CleanupRegionId>, Diagnostic> {
        let mut descendants = std::collections::BTreeSet::new();
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            descendants.insert(expression.id.clone());
            crate::hir::push_resolved_expression_children_in_authored_order(
                expression,
                &mut pending,
            );
        }
        let anchors = self
            .plan
            .cleanup_place_flags
            .keys()
            .filter_map(|place| match &place.storage {
                crate::cleanup_plan::StorageId::Temporary(expression)
                    if descendants.contains(expression)
                        && matches!(
                            self.plan.cleanup_storage_types.get(&place.storage),
                            Some(ty) if *ty == ResolvedType::String
                        ) =>
                {
                    Some(place.storage.clone())
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        if anchors.is_empty() {
            return Ok(std::collections::BTreeSet::new());
        }
        anchors
            .iter()
            .map(|anchor| self.exact_storage_region(anchor, child_kind))
            .collect()
    }

    fn exact_storage_region(
        &self,
        storage: &crate::cleanup_plan::StorageId,
        child_kind: &str,
    ) -> Result<crate::cleanup_plan::CleanupRegionId, Diagnostic> {
        let regions = self
            .cleanup_plan
            .regions
            .iter()
            .filter(|region| region.slots.contains(storage))
            .map(|region| region.id)
            .collect::<Vec<_>>();
        let [region] = regions.as_slice() else {
            return Err(error(format!(
                "String match {child_kind} temporary has ambiguous CleanupPlan ownership",
            )));
        };
        Ok(*region)
    }

    fn cleanup_region(
        &self,
        id: crate::cleanup_plan::CleanupRegionId,
    ) -> Result<&crate::cleanup_plan::CleanupRegion, Diagnostic> {
        self.cleanup_plan
            .regions
            .get(id.0 as usize)
            .filter(|region| region.id == id)
            .ok_or_else(|| error("CleanupPlan region id is not canonical"))
    }

    fn cleanup_block(
        &self,
        id: crate::cleanup_plan::BlockId,
    ) -> Result<&crate::cleanup_plan::CleanupBlock, Diagnostic> {
        self.cleanup_plan
            .blocks
            .get(id.0 as usize)
            .filter(|block| block.id == id)
            .ok_or_else(|| error("CleanupPlan block id is not canonical"))
    }

    fn cleanup_edge(
        &self,
        id: crate::cleanup_plan::EdgeId,
    ) -> Result<&crate::cleanup_plan::CleanupEdge, Diagnostic> {
        self.cleanup_plan
            .edges
            .get(id.0 as usize)
            .filter(|edge| edge.id == id)
            .ok_or_else(|| error("CleanupPlan edge id is not canonical"))
    }

    fn region_ancestors(
        &self,
        start: crate::cleanup_plan::CleanupRegionId,
    ) -> Result<Vec<crate::cleanup_plan::CleanupRegionId>, Diagnostic> {
        let mut regions = Vec::new();
        let mut current = Some(start);
        while let Some(id) = current {
            if regions.len() >= self.cleanup_plan.regions.len() {
                return Err(error("CleanupPlan region ancestry is cyclic"));
            }
            let region = self.cleanup_region(id)?;
            regions.push(region.id);
            current = region.parent;
        }
        Ok(regions)
    }

    fn normal_scope_continue(
        &self,
        region: crate::cleanup_plan::CleanupRegionId,
        child_kind: &str,
    ) -> Result<
        (
            crate::cleanup_plan::ExitTargetId,
            crate::cleanup_plan::EdgeId,
        ),
        Diagnostic,
    > {
        let region = self.cleanup_region(region)?;
        let exit = self
            .cleanup_plan
            .exits
            .get(region.normal_scope_end.0 as usize)
            .filter(|exit| exit.id == region.normal_scope_end)
            .ok_or_else(|| {
                error(format!(
                    "String match {child_kind} child has no normal-scope exit",
                ))
            })?;
        let crate::cleanup_plan::ExitContinuation::Continue(edge) = &exit.continuation else {
            return Err(error(format!(
                "String match {child_kind} child normal-scope exit does not continue",
            )));
        };
        if exit.leaves_regions.as_slice() != [region.id] {
            return Err(error(format!(
                "String match {child_kind} child normal-scope exit is not canonical",
            )));
        }
        Ok((exit.id, *edge))
    }

    fn guard_cleanup_region(
        &self,
        owner: crate::cleanup_plan::CleanupRegionId,
        guard: &crate::hir::ExpressionId,
    ) -> Result<crate::cleanup_plan::CleanupRegionId, Diagnostic> {
        for candidate in self.region_ancestors(owner)? {
            let Ok((_, continuation)) = self.normal_scope_continue(candidate, "guard") else {
                continue;
            };
            let edge = self.cleanup_edge(continuation)?;
            let after = self.cleanup_block(edge.to)?;
            let crate::cleanup_plan::CleanupTerminator::Branch(edges) = &after.terminator else {
                continue;
            };
            let mut true_edge = false;
            let mut false_edge = false;
            for edge in edges {
                match &self.cleanup_edge(*edge)?.condition {
                    crate::cleanup_plan::EdgeCondition::BooleanResult(result, true)
                        if result == guard =>
                    {
                        true_edge = true
                    }
                    crate::cleanup_plan::EdgeCondition::BooleanResult(result, false)
                        if result == guard =>
                    {
                        false_edge = true
                    }
                    _ => {}
                }
            }
            if true_edge && false_edge {
                return Ok(candidate);
            }
        }
        Err(error(
            "String match guard has no structural CleanupPlan child region",
        ))
    }

    fn scalar_match_parent_region(
        &self,
        scalar_match: &crate::hir::ExpressionId,
    ) -> Result<crate::cleanup_plan::CleanupRegionId, Diagnostic> {
        let mut parents = std::collections::BTreeSet::new();
        for edge in &self.cleanup_plan.edges {
            if let crate::cleanup_plan::EdgeCondition::ArmSelected { scrutinee, .. } =
                &edge.condition
            {
                if scrutinee == scalar_match {
                    parents.insert(self.cleanup_block(edge.from)?.region);
                }
            }
        }
        let mut parents = parents.into_iter();
        let parent = parents.next().ok_or_else(|| {
            error("scalar match has no unique structural CleanupPlan parent region")
        })?;
        if parents.next().is_some() {
            return Err(error(
                "scalar match has no unique structural CleanupPlan parent region",
            ));
        }
        Ok(parent)
    }

    fn value_cleanup_region(
        &self,
        owner: crate::cleanup_plan::CleanupRegionId,
        parent: crate::cleanup_plan::CleanupRegionId,
    ) -> Result<crate::cleanup_plan::CleanupRegionId, Diagnostic> {
        self.region_ancestors(owner)?
            .into_iter()
            .find(|candidate| {
                self.cleanup_region(*candidate)
                    .is_ok_and(|region| region.parent == Some(parent))
            })
            .ok_or_else(|| error("String match value has no structural CleanupPlan child region"))
    }

    fn emit_scalar_match_region_cleanup(
        &mut self,
        region: crate::cleanup_plan::CleanupRegionId,
        child_kind: &str,
    ) -> Result<(), Diagnostic> {
        let (exit_id, _) = self.normal_scope_continue(region, child_kind)?;
        let actions = self
            .cleanup_plan
            .exits
            .get(exit_id.0 as usize)
            .filter(|exit| exit.id == exit_id)
            .map(|exit| exit.finalize_in_order.clone())
            .ok_or_else(|| error("CleanupPlan normal-scope exit id is not canonical"))?;
        self.emit_cleanup_actions(&actions)
    }
}

impl Emitter<'_> {
    fn emit_once_cleanup(
        &mut self,
        action: &crate::cleanup_plan::FinalizeAction,
    ) -> Result<(), Diagnostic> {
        let value = self.cleanup_value_at(&action.source)?;
        let Value::Aggregate {
            pointer,
            ty:
                ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair,
        } = value
        else {
            return Err(error(
                "affine finalizer requires its exact aggregate carrier",
            ));
        };
        let flag = *self
            .plan
            .cleanup_flags
            .get(&action.guard_flag)
            .ok_or_else(|| error("affine finalizer lacks canonical liveness flag"))?;
        self.output.push(0x20);
        write_u32(self.output, flag);
        self.output.extend([0x04, 0x40]);
        self.emit_pointer(Pointer {
            offset: pointer.offset + 8,
            ..pointer
        });
        self.load_scalar(&ResolvedType::Bytes);
        self.output.push(0x10);
        write_u32(self.output, BYTE_DROP_IMPORT);
        self.emit_pointer(pointer);
        self.output.extend([0x41, 0x00, 0x41]);
        write_i64(self.output, 80);
        self.output.extend([0xfc, 0x0b, 0x00]);
        self.semantic_cleanup_event(action.guard_flag.0)?;
        self.output.extend([0x41, 0x00, 0x21]);
        write_u32(self.output, flag);
        self.output.push(0x0b);
        Ok(())
    }
}
