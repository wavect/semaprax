//! Core Wasm lowering for the closed owning scalar iterator protocol.
use super::*;

const ITER_HANDLE_OFFSET: u32 = 0;
const ITER_CURSOR_OFFSET: u32 = 8;
const STEP_TAG_OFFSET: u32 = 0;
const STEP_ITEM_OFFSET: u32 = 8;
const STEP_REST_OFFSET: u32 = 16;

impl Emitter<'_> {
    pub(super) fn bind_variant_match_fields(
        &mut self,
        fields: &[crate::hir::ResolvedMatchPatternField],
        case_layout: &crate::variant_layout::VariantCaseLayout,
        scrutinee: Pointer,
        payload_offset: u32,
        mode: crate::hir::ResolvedMatchMode,
    ) -> Result<(), Diagnostic> {
        for pattern_field in fields {
            let field = case_layout
                .field(&pattern_field.field)
                .cloned()
                .ok_or_else(|| {
                    error(format!(
                        "match case `{}` has no field `{}`",
                        case_layout.case, pattern_field.field
                    ))
                })?;
            require_type(
                &pattern_field.binding.ty,
                &field.ty,
                "match payload binding",
            )?;
            let pointer = Pointer {
                local: scrutinee.local,
                offset: scrutinee
                    .offset
                    .checked_add(payload_offset)
                    .and_then(|offset| offset.checked_add(field.offset))
                    .ok_or_else(|| error("match payload pointer overflows u32"))?,
            };
            let source = value_at(pointer, field.ty.clone(), self.program)?;
            if mode == crate::hir::ResolvedMatchMode::Borrow
                && crate::iterator_ops::is_iter(&field.ty)
            {
                // A borrowed iterator field is an alias into the authenticated
                // active Step payload. It must not pass through the consuming
                // iterator move path or clear the loop-carried remainder.
                self.bindings
                    .insert(pattern_field.binding.id.clone(), source);
                continue;
            }
            let destination = if is_aggregate(self.program, &field.ty)? {
                Value::Aggregate {
                    pointer: Pointer {
                        local: self.plan.frame_base,
                        offset: self
                            .plan
                            .aggregate_bindings
                            .get(&pattern_field.binding.id)
                            .copied()
                            .ok_or_else(|| {
                                error(format!(
                                    "missing aggregate match binding `{}`",
                                    pattern_field.binding.id
                                ))
                            })?,
                    },
                    ty: field.ty.clone(),
                }
            } else {
                Value::Scalar {
                    local: self
                        .plan
                        .scalar_bindings
                        .get(&pattern_field.binding.id)
                        .copied()
                        .ok_or_else(|| {
                            error(format!(
                                "missing match binding `{}`",
                                pattern_field.binding.id
                            ))
                        })?,
                    ty: field.ty.clone(),
                }
            };
            if field.ty == ResolvedType::Bytes && mode == crate::hir::ResolvedMatchMode::Borrow {
                self.copy_borrowed_scalar_alias(&destination, &source)?;
            } else {
                self.copy_value(&destination, &source, "variant match field binding")?;
            }
            self.bindings
                .insert(pattern_field.binding.id.clone(), destination);
        }
        Ok(())
    }

    pub(super) fn copy_iterator_value(
        &mut self,
        destination: Pointer,
        source: Pointer,
        ty: &ResolvedType,
    ) -> Result<bool, Diagnostic> {
        if destination.local == source.local && destination.offset == source.offset {
            if crate::iterator_ops::is_step(ty) {
                // Even an aliased materialization must authenticate the tag
                // before a later match is allowed to inspect its payload.
                self.emit_pointer(source);
                self.output.extend([0x28, 0x02, 0x00, 0x41, 0x02, 0x4f]);
                self.trap_if();
                return Ok(true);
            }
            if crate::iterator_ops::is_iter(ty) {
                return Ok(true);
            }
        }
        if crate::iterator_ops::is_iter(ty) {
            for offset in [ITER_HANDLE_OFFSET, ITER_CURSOR_OFFSET] {
                self.emit_pointer(Pointer {
                    offset: destination.offset + offset,
                    ..destination
                });
                self.emit_pointer(Pointer {
                    offset: source.offset + offset,
                    ..source
                });
                self.load_scalar(&ResolvedType::I64);
                self.store_scalar(&ResolvedType::I64);
            }
            self.clear_iterator(&Value::Aggregate {
                pointer: source,
                ty: ty.clone(),
            })?;
            return Ok(true);
        }
        if !crate::iterator_ops::is_step(ty) {
            return Ok(false);
        }
        // Authenticate the discriminant before reading the conditional owner.
        self.emit_pointer(source);
        self.output.extend([0x28, 0x02, 0x00, 0x41, 0x02, 0x4f]);
        self.trap_if();
        self.emit_pointer(destination);
        self.output
            .extend([0x41, 0x00, 0x41, 0x20, 0xfc, 0x0b, 0x00]);
        self.emit_pointer(source);
        self.output.extend([0x28, 0x02, 0x00, 0x04, 0x40]);
        for offset in [STEP_ITEM_OFFSET, STEP_REST_OFFSET, STEP_REST_OFFSET + 8] {
            self.emit_pointer(Pointer {
                offset: destination.offset + offset,
                ..destination
            });
            self.emit_pointer(Pointer {
                offset: source.offset + offset,
                ..source
            });
            self.load_scalar(&ResolvedType::I64);
            self.store_scalar(&ResolvedType::I64);
        }
        self.output.push(0x0b);
        self.emit_pointer(destination);
        self.emit_pointer(source);
        self.output.extend([0x28, 0x02, 0x00, 0x36, 0x02, 0x00]);
        self.emit_pointer(source);
        self.output
            .extend([0x41, 0x00, 0x41, 0x20, 0xfc, 0x0b, 0x00]);
        Ok(true)
    }

    pub(super) fn emit_iterator_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::iterator_ops::IteratorOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let [element] = type_arguments else {
            return Err(error("iterator operation requires one exact type argument"));
        };
        if !crate::iterator_ops::resolved_element_is_admitted(element) || args.len() != 1 {
            return Err(error(
                "iterator operation disagrees with its scalar profile",
            ));
        }
        require_type(
            &args[0].ty,
            &op.resolved_param_type(element),
            "iterator operation argument",
        )?;
        require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "iterator operation result",
        )?;
        match op {
            crate::iterator_ops::IteratorOp::VecIntoIter => {
                self.emit_vec_into_iter(expr, element, args)
            }
            crate::iterator_ops::IteratorOp::Next => self.emit_iter_next(expr, element, args),
        }
    }

    fn iterator_argument(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let value = self.emit_expr(&args[0])?;
        require_type(
            value_type(&value),
            &crate::iterator_ops::resolved_iter(element.clone()),
            "iterator owner",
        )?;
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(),
            parameter_index: 0,
            value_expression: args[0].id.clone(),
        };
        if self.plan.cleanup_storage_types.contains_key(&epoch)
            && self
                .call_argument_values
                .insert(epoch, value.clone())
                .is_some()
        {
            return Err(error("iterator call epoch carrier is not unique"));
        }
        Ok(value)
    }

    fn emit_vec_into_iter(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let _evaluated = self.emit_expr(&args[0])?;
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(),
            parameter_index: 0,
            value_expression: args[0].id.clone(),
        };
        let source = Value::Scalar {
            local: self
                .plan
                .cleanup_call_argument_carriers
                .get(&epoch)
                .copied()
                .ok_or_else(|| error("vec_into_iter has no authenticated argument carrier"))?,
            ty: crate::vec_ops::resolved_vec(element.clone()),
        };
        let result = Value::Aggregate {
            pointer: self.plan.expr_pointer(expr)?,
            ty: expr.ty.clone(),
        };
        let Value::Aggregate { pointer, .. } = &result else {
            unreachable!()
        };
        self.emit_pointer(Pointer {
            offset: pointer.offset + ITER_HANDLE_OFFSET,
            ..*pointer
        });
        self.get_scalar(&source);
        self.store_scalar(&ResolvedType::I64);
        self.emit_pointer(Pointer {
            offset: pointer.offset + ITER_CURSOR_OFFSET,
            ..*pointer
        });
        self.output.extend([0x42, 0x00]);
        self.store_scalar(&ResolvedType::Usize);
        self.apply_call_commit(&expr.id)?;
        self.clear_scalar(&source)?;
        Ok(result)
    }

    fn emit_iter_next(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let source = self.iterator_argument(expr, element, args)?;
        let Value::Aggregate {
            pointer: source_pointer,
            ..
        } = source
        else {
            return Err(error("iter_next argument is not aggregate storage"));
        };
        let result = Value::Aggregate {
            pointer: self.plan.expr_pointer(expr)?,
            ty: expr.ty.clone(),
        };
        let Value::Aggregate {
            pointer: result_pointer,
            ..
        } = &result
        else {
            unreachable!()
        };
        let tag = vec_element_tag(element)?;
        let handle = Pointer {
            offset: source_pointer.offset + ITER_HANDLE_OFFSET,
            ..source_pointer
        };
        let cursor = Pointer {
            offset: source_pointer.offset + ITER_CURSOR_OFFSET,
            ..source_pointer
        };

        // Validate/read the element before the deferred owner commit. A failed
        // read therefore leaves the staged iterator governed by CleanupPlan.
        self.emit_pointer(*result_pointer);
        self.output
            .extend([0x41, 0x00, 0x41, 0x20, 0xfc, 0x0b, 0x00]);
        self.emit_pointer(cursor);
        self.load_scalar(&ResolvedType::Usize);
        self.emit_pointer(handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x41);
        write_i64(self.output, i64::from(tag));
        self.output.push(0x10);
        write_u32(self.output, vec_import_base(self.program) + 2);
        self.output.push(0x5a); // cursor >= len (unsigned)
        self.output.extend([0x04, 0x40]);

        // Equality is exhaustion. A corrupted cursor beyond the exact length
        // follows the ordinary bounded-read failure exit before owner commit.
        self.emit_pointer(cursor);
        self.load_scalar(&ResolvedType::Usize);
        self.emit_pointer(handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x41);
        write_i64(self.output, i64::from(tag));
        self.output.push(0x10);
        write_u32(self.output, vec_import_base(self.program) + 2);
        self.output.push(0x52); // cursor != len
        self.emit_vec_failure_if(expr, STATUS_VEC_GET_OUT_OF_BOUNDS)?;

        // Done owns no payload and settles the vector now.
        self.emit_pointer(handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x10);
        write_u32(self.output, vec_import_base(self.program) + 5);
        self.emit_pointer(Pointer {
            offset: result_pointer.offset + STEP_TAG_OFFSET,
            ..*result_pointer
        });
        self.output.extend([0x41, 0x00, 0x36, 0x02, 0x00]);
        self.output.push(0x05); // else: Yield

        self.emit_pointer(Pointer {
            offset: result_pointer.offset + STEP_ITEM_OFFSET,
            ..*result_pointer
        });
        self.emit_pointer(handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x41);
        write_i64(self.output, i64::from(tag));
        self.emit_pointer(cursor);
        self.load_scalar(&ResolvedType::Usize);
        self.output.push(0x10);
        write_u32(self.output, vec_import_base(self.program) + 4);
        self.store_vec_element_memory_bits(element)?;
        self.emit_pointer(Pointer {
            offset: result_pointer.offset + STEP_REST_OFFSET,
            ..*result_pointer
        });
        self.emit_pointer(handle);
        self.load_scalar(&ResolvedType::I64);
        self.store_scalar(&ResolvedType::I64);
        self.emit_pointer(Pointer {
            offset: result_pointer.offset + STEP_REST_OFFSET + ITER_CURSOR_OFFSET,
            ..*result_pointer
        });
        self.emit_pointer(cursor);
        self.load_scalar(&ResolvedType::Usize);
        self.output.extend([0x42, 0x01, 0x7c]); // cursor + 1
        self.store_scalar(&ResolvedType::Usize);
        self.emit_pointer(Pointer {
            offset: result_pointer.offset + STEP_TAG_OFFSET,
            ..*result_pointer
        });
        self.output.extend([0x41, 0x01, 0x36, 0x02, 0x00, 0x0b]);

        self.apply_call_commit(&expr.id)?;
        self.clear_iterator(&Value::Aggregate {
            pointer: source_pointer,
            ty: crate::iterator_ops::resolved_iter(element.clone()),
        })?;
        Ok(result)
    }

    fn store_vec_element_memory_bits(&mut self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        match ty {
            ResolvedType::I64 | ResolvedType::Usize => self.store_scalar(ty),
            ResolvedType::F64 => {
                self.output.push(0xbf);
                self.store_scalar(ty);
            }
            ResolvedType::F32 => {
                self.output.extend([0xa7, 0xbe]);
                self.store_scalar(ty);
            }
            ResolvedType::I32 | ResolvedType::U8 | ResolvedType::Char | ResolvedType::Bool => {
                self.output.push(0xa7);
                self.store_scalar(ty);
            }
            _ => return Err(error("iterator element is outside the scalar profile")),
        }
        Ok(())
    }

    pub(super) fn clear_iterator(&mut self, value: &Value) -> Result<(), Diagnostic> {
        let Value::Aggregate { pointer, ty } = value else {
            return Err(error("iterator poison requires aggregate storage"));
        };
        if !crate::iterator_ops::is_iter(ty) {
            return Err(error("iterator poison requires exact Iter<T>"));
        }
        self.emit_pointer(*pointer);
        self.output
            .extend([0x41, 0x00, 0x41, 0x10, 0xfc, 0x0b, 0x00]);
        Ok(())
    }
}
