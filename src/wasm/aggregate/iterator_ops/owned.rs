//! Versioned host calls for the `Iter<Bytes>` ownership transfer protocol.
use super::super::{EXTENDED_VEC_IMPORT_COUNT, RECORD_VEC_IMPORT_COUNT, VEC_IMPORT_COUNT};
use super::*;

pub(super) const IMPORT_COUNT: u32 = 3;
pub(super) const RECORD_IMPORT_COUNT: u32 = 3;

pub(super) const fn import_names() -> [&'static str; IMPORT_COUNT as usize] {
    [
        "spx_iter_bytes_into_v2",
        "spx_iter_bytes_next_v2",
        "spx_iter_bytes_drop_v2",
    ]
}

pub(super) const fn record_import_names() -> [&'static str; RECORD_IMPORT_COUNT as usize] {
    [
        "spx_iter_record_into_v3",
        "spx_iter_record_next_v3",
        "spx_iter_record_drop_v3",
    ]
}

pub(super) fn import_base(program: &ResolvedProgram) -> u32 {
    vec_import_base(program)
        + VEC_IMPORT_COUNT
        + if crate::wasm::vec_ops::program_uses_extended_vec(program) {
            EXTENDED_VEC_IMPORT_COUNT
        } else {
            0
        }
        + if crate::wasm::vec_ops::program_uses_record_vec(program) {
            RECORD_VEC_IMPORT_COUNT
        } else {
            0
        }
        + u32::from(crate::vec_ops::resolved_program_uses_sort(program))
}

pub(super) fn record_import_base(program: &ResolvedProgram) -> u32 {
    import_base(program)
        + if crate::iterator_ops::resolved_program_uses_owned_iterator(program) {
            IMPORT_COUNT
        } else {
            0
        }
}

impl Emitter<'_> {
    pub(super) fn emit_record_vec_into_iter(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let _ = self.emit_expr(&args[0])?;
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
                .ok_or_else(|| error("record iterator has no staged Vec carrier"))?,
            ty: crate::vec_ops::resolved_vec(element.clone()),
        };
        let result = Value::Aggregate {
            pointer: self.plan.expr_pointer(expr)?,
            ty: expr.ty.clone(),
        };
        let Value::Aggregate { pointer, .. } = result else {
            unreachable!()
        };
        self.poison_iterator_frame(pointer);
        self.get_scalar(&source);
        self.emit_pointer(pointer);
        self.output.push(0x10);
        write_u32(self.output, record_import_base(self.program));
        self.emit_owned_iterator_status(expr)?;
        self.validate_iterator_frame(pointer);
        self.apply_call_commit(&expr.id)?;
        self.clear_scalar(&source)?;
        Ok(Value::Aggregate {
            pointer,
            ty: expr.ty.clone(),
        })
    }

    pub(super) fn emit_record_iter_next(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let source = self.iterator_argument(expr, element, args)?;
        let Value::Aggregate {
            pointer: source, ..
        } = source
        else {
            return Err(error("record iter_next argument is not aggregate storage"));
        };
        let result_pointer = self.plan.expr_pointer(expr)?;
        let variant = variant_layout(self.variant_layouts, &expr.ty)?;
        let yielded = variant
            .cases
            .iter()
            .find(|case| case.case.as_str() == crate::iterator_ops::YIELD_ID)
            .ok_or_else(|| error("record iterator Yield layout is missing"))?;
        let item = yielded
            .field(&DeclarationId::new(crate::iterator_ops::ITEM_ID))
            .ok_or_else(|| error("record iterator item layout is missing"))?;
        let rest = yielded
            .field(&DeclarationId::new(crate::iterator_ops::REST_ID))
            .ok_or_else(|| error("record iterator rest layout is missing"))?;
        let record = layout(self.program, element)?;
        let fields = crate::hir::owned_record_collection::owned_record_element_fields(
            &self.program.declarations,
            element,
        )
        .ok_or_else(|| error("record iterator element shape changed"))?;
        let field_offset = |id: &DeclarationId| {
            record
                .fields
                .iter()
                .find(|field| &field.field == id)
                .map(|field| field.offset)
                .ok_or_else(|| error("record iterator field layout is missing"))
        };
        let item_base = variant
            .payload_offset
            .checked_add(item.offset)
            .ok_or_else(|| error("record iterator item offset overflows"))?;
        let byte0 = item_base
            .checked_add(field_offset(&fields.owned[0].id)?)
            .ok_or_else(|| error("record iterator field offset overflows"))?;
        let byte1 = item_base
            .checked_add(field_offset(&fields.owned[1].id)?)
            .ok_or_else(|| error("record iterator field offset overflows"))?;
        let scalar = item_base
            .checked_add(field_offset(&fields.scalar.id)?)
            .ok_or_else(|| error("record iterator scalar offset overflows"))?;
        let rest = variant
            .payload_offset
            .checked_add(rest.offset)
            .ok_or_else(|| error("record iterator rest offset overflows"))?;
        self.poison_frame(result_pointer, i64::from(variant.size));
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_HANDLE_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::I64);
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_CURSOR_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::Usize);
        self.emit_pointer(result_pointer);
        for offset in [byte0, byte1, scalar, rest] {
            self.output.push(0x41);
            write_i64(self.output, i64::from(offset));
        }
        self.output.push(0x10);
        write_u32(self.output, record_import_base(self.program) + 1);
        self.emit_owned_iterator_status(expr)?;
        self.validate_record_step_frame(
            result_pointer,
            source,
            byte0,
            byte1,
            scalar,
            &fields.scalar.ty,
            rest,
        )?;
        self.apply_call_commit(&expr.id)?;
        self.clear_iterator(&Value::Aggregate {
            pointer: source,
            ty: crate::iterator_ops::resolved_iter(element.clone()),
        })?;
        Ok(Value::Aggregate {
            pointer: result_pointer,
            ty: expr.ty.clone(),
        })
    }

    pub(super) fn emit_owned_vec_into_iter(
        &mut self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let _ = self.emit_expr(&args[0])?;
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
                .ok_or_else(|| error("owned iterator has no staged Vec carrier"))?,
            ty: crate::vec_ops::resolved_vec(ResolvedType::Bytes),
        };
        let result = Value::Aggregate {
            pointer: self.plan.expr_pointer(expr)?,
            ty: expr.ty.clone(),
        };
        let Value::Aggregate { pointer, .. } = result else {
            unreachable!()
        };
        self.poison_iterator_frame(pointer);
        self.get_scalar(&source);
        self.emit_pointer(pointer);
        self.output.push(0x10);
        write_u32(self.output, import_base(self.program));
        self.emit_owned_iterator_status(expr)?;
        self.validate_iterator_frame(pointer);
        self.apply_call_commit(&expr.id)?;
        self.clear_scalar(&source)?;
        Ok(Value::Aggregate {
            pointer,
            ty: expr.ty.clone(),
        })
    }

    pub(super) fn emit_owned_iter_next(
        &mut self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let source = self.iterator_argument(expr, &ResolvedType::Bytes, args)?;
        let Value::Aggregate {
            pointer: source, ..
        } = source
        else {
            return Err(error("owned iter_next argument is not aggregate storage"));
        };
        let result = Value::Aggregate {
            pointer: self.plan.expr_pointer(expr)?,
            ty: expr.ty.clone(),
        };
        let Value::Aggregate {
            pointer: result_pointer,
            ..
        } = result
        else {
            unreachable!()
        };
        self.poison_step_frame(result_pointer);
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_HANDLE_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::I64);
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_CURSOR_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::Usize);
        self.emit_pointer(result_pointer);
        self.output.push(0x10);
        write_u32(self.output, import_base(self.program) + 1);
        self.emit_owned_iterator_status(expr)?;
        self.validate_step_frame(result_pointer, source);
        self.apply_call_commit(&expr.id)?;
        self.clear_iterator(&Value::Aggregate {
            pointer: source,
            ty: crate::iterator_ops::resolved_iter(ResolvedType::Bytes),
        })?;
        Ok(Value::Aggregate {
            pointer: result_pointer,
            ty: expr.ty.clone(),
        })
    }

    fn poison_iterator_frame(&mut self, pointer: Pointer) {
        self.poison_frame(pointer, 16);
    }

    fn poison_step_frame(&mut self, pointer: Pointer) {
        self.poison_frame(pointer, 32);
    }

    fn poison_frame(&mut self, pointer: Pointer, size: i64) {
        self.emit_pointer(pointer);
        self.output.push(0x41);
        write_i64(self.output, 0xa5);
        self.output.push(0x41);
        write_i64(self.output, size);
        self.output.extend([0xfc, 0x0b, 0x00]); // memory.fill
    }

    fn trap_if_i64_nonzero_at(&mut self, pointer: Pointer) {
        self.emit_pointer(pointer);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x50); // i64.eqz
        self.output.push(0x45); // i32.eqz
        self.trap_if();
    }

    fn validate_iterator_frame(&mut self, pointer: Pointer) {
        // A success must publish one fresh iterator at cursor zero. The poison
        // fill above makes a provider that returns success without writing fail
        // closed before the Vec owner commits.
        self.emit_pointer(Pointer {
            offset: pointer.offset + ITER_HANDLE_OFFSET,
            ..pointer
        });
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x50); // handle == 0
        self.trap_if();
        self.emit_pointer(Pointer {
            offset: pointer.offset + ITER_HANDLE_OFFSET,
            ..pointer
        });
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x42);
        write_i64(self.output, i64::from_le_bytes([0xa5; 8]));
        self.output.push(0x51); // handle is unchanged poison
        self.trap_if();
        self.trap_if_i64_nonzero_at(Pointer {
            offset: pointer.offset + ITER_CURSOR_OFFSET,
            ..pointer
        });
    }

    fn validate_step_frame(&mut self, pointer: Pointer, source: Pointer) {
        let tag = Pointer {
            offset: pointer.offset + STEP_TAG_OFFSET,
            ..pointer
        };
        let reserved = Pointer {
            offset: pointer.offset + 4,
            ..pointer
        };
        let item = Pointer {
            offset: pointer.offset + STEP_ITEM_OFFSET,
            ..pointer
        };
        let rest_handle = Pointer {
            offset: pointer.offset + STEP_REST_OFFSET,
            ..pointer
        };
        let rest_cursor = Pointer {
            offset: pointer.offset + STEP_REST_OFFSET + 8,
            ..pointer
        };
        self.emit_pointer(tag);
        self.output.extend([0x28, 0x02, 0x00, 0x41, 0x01, 0x4b]); // tag > 1
        self.trap_if();
        self.emit_pointer(reserved);
        self.output.extend([0x28, 0x02, 0x00, 0x45, 0x45]); // reserved != 0
        self.trap_if();

        self.emit_pointer(tag);
        self.output.extend([0x28, 0x02, 0x00, 0x45, 0x04, 0x40]); // if Done
        self.trap_if_i64_nonzero_at(item);
        self.trap_if_i64_nonzero_at(rest_handle);
        self.trap_if_i64_nonzero_at(rest_cursor);
        self.output.push(0x05); // else Yield
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_CURSOR_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::Usize);
        self.output.extend([0x42, 0x7f, 0x51]); // source cursor == u64::MAX
        self.trap_if();
        self.emit_pointer(rest_handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x50); // successor handle == 0
        self.trap_if();
        self.emit_pointer(rest_handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x42);
        write_i64(self.output, i64::from_le_bytes([0xa5; 8]));
        self.output.push(0x51); // successor handle is unchanged poison
        self.trap_if();
        self.emit_pointer(rest_cursor);
        self.load_scalar(&ResolvedType::Usize);
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_CURSOR_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::Usize);
        self.output.extend([0x42, 0x01, 0x7c, 0x52]); // rest != source + 1
        self.trap_if();
        // Zero is the admitted empty Bytes carrier. Every nonzero payload
        // must be tagged as owned before the established byte authority seam
        // authenticates it, including an owned empty payload.
        self.emit_pointer(item);
        self.load_scalar(&ResolvedType::Bytes);
        self.output.extend([0x50, 0x04, 0x40, 0x05]); // if carrier == 0
        self.emit_pointer(item);
        self.load_scalar(&ResolvedType::Bytes);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.extend([0x83, 0x50]); // missing owned high-bit tag
        self.trap_if();
        self.emit_pointer(item);
        self.load_scalar(&ResolvedType::Bytes);
        self.output.push(0x10);
        write_u32(self.output, BYTE_AS_SLICE_IMPORT);
        self.output.push(0x1a); // authenticate even an owned empty carrier
        self.emit_pointer(item);
        self.load_scalar(&ResolvedType::Bytes);
        self.output.extend([0xa7, 0x45, 0x04, 0x40, 0x05]);
        self.emit_pointer(item);
        self.load_scalar(&ResolvedType::Bytes);
        self.output.extend([0x42, 0x00, 0x10]);
        write_u32(self.output, BYTE_GET_IMPORT);
        self.output.extend([0x41]);
        write_i64(self.output, 255);
        self.output.push(0x4b); // byte_get < 0 is unsigned > 255
        self.trap_if();
        self.output.push(0x0b); // nonempty byte check
        self.output.push(0x0b); // nonzero carrier branch
        self.output.push(0x0b); // Done/Yield frame branch
    }

    fn validate_record_step_frame(
        &mut self,
        pointer: Pointer,
        source: Pointer,
        byte0: u32,
        byte1: u32,
        scalar: u32,
        scalar_type: &ResolvedType,
        rest: u32,
    ) -> Result<(), Diagnostic> {
        let tag = Pointer {
            offset: pointer.offset + STEP_TAG_OFFSET,
            ..pointer
        };
        self.emit_pointer(tag);
        self.output.extend([0x28, 0x02, 0x00, 0x41, 0x01, 0x4b]);
        self.trap_if();
        self.emit_pointer(tag);
        self.output.extend([0x28, 0x02, 0x00, 0x45, 0x04, 0x40]);
        for offset in [byte0, byte1, scalar, rest, rest + ITER_CURSOR_OFFSET] {
            self.trap_if_i64_nonzero_at(Pointer {
                offset: pointer.offset + offset,
                ..pointer
            });
        }
        self.output.push(0x05);
        for offset in [byte0, byte1] {
            let item = Pointer {
                offset: pointer.offset + offset,
                ..pointer
            };
            self.emit_pointer(item);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.extend([0x50, 0x04, 0x40, 0x05]);
            self.emit_pointer(item);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.push(0x42);
            write_i64(self.output, i64::MIN);
            self.output.extend([0x83, 0x50]);
            self.trap_if();
            self.emit_pointer(item);
            self.load_scalar(&ResolvedType::Bytes);
            self.output.push(0x10);
            write_u32(self.output, BYTE_AS_SLICE_IMPORT);
            self.output.push(0x1a);
            self.output.push(0x0b);
        }
        self.emit_pointer(Pointer {
            offset: pointer.offset + byte0,
            ..pointer
        });
        self.load_scalar(&ResolvedType::Bytes);
        self.output.push(0x50);
        self.output.push(0x45);
        self.emit_pointer(Pointer {
            offset: pointer.offset + byte0,
            ..pointer
        });
        self.load_scalar(&ResolvedType::Bytes);
        self.emit_pointer(Pointer {
            offset: pointer.offset + byte1,
            ..pointer
        });
        self.load_scalar(&ResolvedType::Bytes);
        self.output.extend([0x51, 0x71]);
        self.trap_if();
        self.validate_record_scalar_bits(
            Pointer {
                offset: pointer.offset + scalar,
                ..pointer
            },
            scalar_type,
        )?;
        let rest_handle = Pointer {
            offset: pointer.offset + rest,
            ..pointer
        };
        self.emit_pointer(rest_handle);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x50);
        self.trap_if();
        // The v3 host remints the successor authority. Reusing the consumed
        // handle with an incremented cursor would publish a stale pair after
        // commit even though the cursor relation alone looks well formed.
        self.emit_pointer(rest_handle);
        self.load_scalar(&ResolvedType::I64);
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_HANDLE_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x51); // successor handle == consumed handle
        self.trap_if();
        self.emit_pointer(Pointer {
            offset: pointer.offset + rest + ITER_CURSOR_OFFSET,
            ..pointer
        });
        self.load_scalar(&ResolvedType::Usize);
        self.emit_pointer(Pointer {
            offset: source.offset + ITER_CURSOR_OFFSET,
            ..source
        });
        self.load_scalar(&ResolvedType::Usize);
        self.output.extend([0x42, 0x01, 0x7c, 0x52]);
        self.trap_if();
        self.output.push(0x0b);
        Ok(())
    }

    fn validate_record_scalar_bits(
        &mut self,
        pointer: Pointer,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        match ty {
            ResolvedType::I64 | ResolvedType::Usize | ResolvedType::F64 => {}
            ResolvedType::I32 => {
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.extend([0xa7, 0xac]); // wrap then sign-extend
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.push(0x52); // noncanonical sign extension
                self.trap_if();
            }
            ResolvedType::U8 | ResolvedType::Bool => {
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.push(0x42);
                write_i64(self.output, if *ty == ResolvedType::Bool { 1 } else { 255 });
                self.output.push(0x56); // i64.gt_u
                self.trap_if();
            }
            ResolvedType::Char => {
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.push(0x42);
                write_i64(self.output, 0x10ffff);
                self.output.push(0x56); // above Unicode range
                self.trap_if();
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.push(0x42);
                write_i64(self.output, 0xd800);
                self.output.push(0x5a); // >= surrogate start
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.push(0x42);
                write_i64(self.output, 0xdfff);
                self.output.push(0x58); // <= surrogate end
                self.output.push(0x71); // inside surrogate range
                self.trap_if();
            }
            ResolvedType::F32 => {
                self.emit_pointer(pointer);
                self.load_scalar(&ResolvedType::I64);
                self.output.extend([0x42, 0x20, 0x88, 0x50, 0x45]); // high 32 != 0
                self.trap_if();
            }
            _ => return Err(error("record iterator scalar type changed")),
        }
        Ok(())
    }

    fn emit_owned_iterator_status(&mut self, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
        // Preserve the frozen Vec status vocabulary exactly. Keep the host
        // status in the function status local while testing each admitted
        // failure, so no owner commits before a failed import has selected
        // cleanup. An unknown nonzero status is a host-contract violation.
        self.output.push(0x22); // local.tee status
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x01, 0x46]); // status == 1
        self.emit_vec_failure_if(expression, STATUS_VEC_PUSH_FULL)?;
        self.output.push(0x20); // local.get status
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x02, 0x46]); // status == 2
        self.emit_vec_failure_if(expression, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
        self.output.push(0x20); // local.get status
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x03, 0x46]); // status == 3
        self.emit_vec_failure_if(expression, STATUS_VEC_ALLOCATION_FAILURE)?;
        self.output.push(0x20); // local.get status
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x00, 0x47]); // status != 0 after the closed range above
        self.trap_if();
        Ok(())
    }
}
