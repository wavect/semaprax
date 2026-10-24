//! Keep v18 variant Block transfers on their certified side of scope cleanup.
use super::*;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn lexical_variant_block(&self, block: &ResolvedExpr) -> Result<bool, Diagnostic> {
        Ok(matches!(
            self.function.cleanup_plan.schema,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
                | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
        ) && self.function.cleanup_plan.lexical_control.is_some()
            && block.ownership == hir::OwnershipMode::Own
            && variant_declaration_id(self.program, &block.ty)?.is_some())
    }

    pub(super) fn emit_lexical_variant_block_result(
        &mut self,
        block: &ResolvedExpr,
        value: &CValue,
    ) -> Result<(), Diagnostic> {
        let layout = self.variant_layout(&block.ty)?;
        let phase = self
            .bytes_plan
            .ok_or_else(|| backend_error("native lexical variant Block has no producer plan"))?
            .apply_variant_into_at(
                &block.id,
                &value.code,
                &layout,
                &crate::cleanup_plan::StorageId::Temporary(block.id.clone()),
            )?;
        for line in phase.lines() {
            self.line(line);
        }
        Ok(())
    }
}

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_block_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let saved = self.variables.clone();
                let saved_resource_text_views = self.resource_text_views.clone();
                for statement in statements {
                    match statement {
                        ResolvedStatement::CopyDiscard { value, .. } => {
                            let discarded = self.emit_expr(value)?;
                            if discarded.ty == ResolvedType::Never {
                                self.variables = saved;
                                self.resource_text_views = saved_resource_text_views;
                                return Ok(discarded);
                            }
                            self.line(&format!("(void)({});", discarded.code));
                        }
                        ResolvedStatement::Let {
                            binding,
                            mutable,
                            value,
                            ..
                        } => {
                            let resource_text_view = self.produces_resource_text_view(value);
                            let validated_str_view = matches!(&value.kind,
                                ResolvedExprKind::BorrowPlace { operation, .. }
                                    if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID);
                            if *mutable {
                                self.mutable_bindings.insert(binding.id.clone());
                            }
                            let value_id = value.id.clone();
                            let value = self.emit_expr(value)?;
                            // An abrupt initializer has no local carrier or
                            // continuation, including a certified Yield-only match.
                            if value.ty == ResolvedType::Never {
                                self.variables = saved;
                                self.resource_text_views = saved_resource_text_views;
                                return Ok(value);
                            }
                            self.require_type(&value.ty, &binding.ty, "local binding")?;
                            let local = if is_direct_plan_owned(self.program, &binding.ty) {
                                let plan = self.bytes_plan.ok_or_else(|| {
                                    backend_error(
                                        "owned Bytes binding has no canonical cleanup plan",
                                    )
                                })?;
                                let storage =
                                    crate::cleanup_plan::StorageId::Value(binding.id.clone());
                                let expected = plan.value(&storage)?.to_owned();
                                if value.code != expected {
                                    let transitions = plan.transfer_to(&storage, &value_id)?;
                                    for line in transitions.lines() {
                                        self.line(line);
                                    }
                                }
                                expected
                            } else if matches!(binding.ty, ResolvedType::String)
                                && self.owned_strings.is_some()
                            {
                                let local = self.temporary(&ResolvedType::String)?;
                                self.string_move(&local, &value.code);
                                local
                            } else if matches!(binding.ty, ResolvedType::ArrayU8(0)) {
                                // The expression has already been evaluated;
                                // its zero-sized Copy value has no C storage.
                                "UINT8_C(0)".to_owned()
                            } else if crate::iterator_ops::is_step(&binding.ty) {
                                // The cleanup plan has already transferred the
                                // conditional Iter leaf. Keep only the checked
                                // tag and Copy item in the loop carrier.
                                let local = format!("spx_local_{}", self.next_local);
                                self.next_local += 1;
                                self.line(&format!(
                                    "{} {local} = {{0}};",
                                    c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                ));
                                self.copy_variant_join_carrier(&local, &value.code, &binding.ty)?;
                                self.line(&format!("(void){local};"));
                                local
                            } else if self.record_contains_owned_bytes(&binding.ty)? {
                                let local = format!("spx_local_{}", self.next_local);
                                self.next_local += 1;
                                self.line(&format!(
                                    "{} {local} = {{0}};",
                                    c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                ));
                                self.move_owned_record_fields(&local, &value.code, &binding.ty)?;
                                self.line(&format!("(void){local};"));
                                local
                            } else {
                                let local = format!("spx_local_{}", self.next_local);
                                self.next_local += 1;
                                self.line(&format!(
                                    "{} {local} = {};",
                                    c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                    value.code
                                ));
                                self.line(&format!("(void){local};"));
                                local
                            };
                            if self
                                .variables
                                .insert(
                                    binding.id.clone(),
                                    CBinding {
                                        name: local,
                                        ty: binding.ty.clone(),
                                    },
                                )
                                .is_some()
                            {
                                return Err(backend_error(format!(
                                    "duplicate resolved local identity `{}`",
                                    binding.id
                                )));
                            }
                            if (resource_text_view || validated_str_view) && !*mutable {
                                self.resource_text_views
                                    .insert(binding.id.clone(), validated_str_view);
                            }
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value: assigned,
                            ..
                        } => {
                            self.resource_text_views.remove(&binding.id);
                            // The assigned value is emitted fully first; the
                            // store is a plain C11 assignment into the local
                            // or, for Field Mutation v1, into its one direct
                            // scalar field.
                            let value_id = assigned.id.clone();
                            let value = self.emit_expr(assigned)?;
                            if value.ty == ResolvedType::Never {
                                self.variables = saved;
                                self.resource_text_views = saved_resource_text_views;
                                return Ok(value);
                            }
                            match field {
                                Some(field_id) => {
                                    let layout = self.record_layout(&binding.ty)?;
                                    let field =
                                        layout.field(field_id).cloned().ok_or_else(|| {
                                            backend_error(format!(
                                                "native record `{}` has no assignment field `{field_id}`",
                                                layout.record
                                            ))
                                        })?;
                                    self.require_type(&value.ty, &field.ty, "field assignment")?;
                                    if matches!(field.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string field assignment has no admitted native lowering",
                                        ));
                                    }
                                    if matches!(field.ty, ResolvedType::Bytes) {
                                        return Err(backend_error(
                                            "owned Bytes field assignment has no admitted native lowering",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    if field.size != 0 {
                                        self.line(&format!(
                                            "{}.{} = {};",
                                            target.name,
                                            c_field_symbol(&field.field),
                                            value.code
                                        ));
                                    }
                                }
                                None => {
                                    self.require_type(&value.ty, &binding.ty, "assignment")?;
                                    let string_append = crate::string_ops::is_same_owner_concat_hir(
                                        assigned,
                                        &binding.id,
                                    );
                                    // Same-owner replacement: the canonical cleanup plan
                                    // already carries the one transfer that publishes the
                                    // next generation, for `vec_push`, the loop-carried
                                    // `bytes_set` fill, String append and Reader refill alike.
                                    if crate::codegen::native_emit::is_native_owned_vec_type(
                                        self.program,
                                        &binding.ty,
                                    ) || matches!(binding.ty, ResolvedType::Bytes)
                                        || crate::stdin_stream_ops::is_reader(&binding.ty)
                                        || string_append
                                        || crate::string_ops::replacement::admitted(
                                            binding, assigned,
                                        )
                                    {
                                        let plan = self.bytes_plan.ok_or_else(|| {
                                            backend_error(
                                                "owned same-owner assignment has no cleanup plan",
                                            )
                                        })?;
                                        let storage = crate::cleanup_plan::StorageId::Value(
                                            binding.id.clone(),
                                        );
                                        let expected = plan.value(&storage)?;
                                        if value.code != expected {
                                            for line in
                                                plan.transfer_to(&storage, &value_id)?.lines()
                                            {
                                                self.line(line);
                                            }
                                        }
                                        continue;
                                    }
                                    if crate::iterator_ops::is_step(&binding.ty) {
                                        let target_name = self
                                            .variables
                                            .get(&binding.id)
                                            .ok_or_else(|| {
                                                backend_error(format!(
                                                    "assignment target `{}` has no native local",
                                                    binding.id
                                                ))
                                            })?
                                            .name
                                            .clone();
                                        self.copy_variant_join_carrier(
                                            &target_name,
                                            &value.code,
                                            &binding.ty,
                                        )?;
                                        continue;
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    if !matches!(binding.ty, ResolvedType::ArrayU8(0)) {
                                        self.line(&format!("{} = {};", target.name, value.code));
                                    }
                                }
                            }
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            // Backends treat the boundary transparently: emit
                            // exactly the ordinary block body and discard its
                            // scalar Copy result.
                            let value = self.emit_expr(body)?;
                            if matches!(value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            if value.ty != ResolvedType::Never {
                                self.line(&format!("(void)({});", value.code));
                            }
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            self.emit_while(condition, body)?;
                        }
                        ResolvedStatement::LexicalWhile {
                            condition,
                            body,
                            latch,
                            ..
                        } => {
                            let certificate = self
                                .function
                                .cleanup_plan
                                .lexical_loop(&condition.id)
                                .ok_or_else(|| {
                                    backend_error("native lexical loop lacks exact certificate")
                                })?;
                            if certificate.body != body.id || &certificate.latch != latch {
                                return Err(backend_error("native lexical loop latch differs"));
                            }
                            self.emit_while(condition, body)?;
                        }
                    }
                }
                let mut tail = self.emit_expr(tail)?;
                self.require_flow_type(&tail.ty, &expr.ty, "block result")?;
                if tail.ty == ResolvedType::Never {
                    self.variables = saved;
                    self.resource_text_views = saved_resource_text_views;
                    return Ok(tail);
                }
                // Owned string locals introduced in this block free exactly
                // their own buffer when the block exits; outer bindings and
                // the tail value are untouched. The order is sorted so the
                // projection stays byte-deterministic.
                if self.bytes_plan.is_none() {
                    let mut introduced_strings: Vec<String> = self
                        .variables
                        .iter()
                        .filter(|(id, binding)| {
                            matches!(binding.ty, ResolvedType::String) && !saved.contains_key(*id)
                        })
                        .map(|(_, binding)| binding.name.clone())
                        .collect();
                    introduced_strings.sort();
                    for name in introduced_strings {
                        self.string_drop(&name);
                    }
                }
                if is_direct_plan_owned(self.program, &tail.ty) {
                    let plan = self.bytes_plan.ok_or_else(|| {
                        backend_error("owned Bytes block has no canonical cleanup plan")
                    })?;
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                    tail.code = plan
                        .result_at(&expr.id)
                        .ok_or_else(|| {
                            backend_error("owned Bytes block has no canonical result transfer")
                        })?
                        .to_owned();
                } else if is_aggregate_type(self.program, &tail.ty)? {
                    if self.lexical_variant_block(expr)? {
                        self.emit_lexical_variant_block_result(expr, &tail)?;
                    } else {
                        self.apply_owned_plan_at_value(&expr.id, &tail)?;
                    }
                }
                self.emit_block_plan_scope_exit(expr)?;
                if self.lexical_variant_block(expr)? {
                    self.finish_variant_join(expr, &tail.code)?;
                }
                self.emit_owned_break_join_label(expr)?;
                self.variables = saved;
                self.resource_text_views = saved_resource_text_views;
                tail
            }
            _ => unreachable!("non-Block expression reached emit_block_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }
}
