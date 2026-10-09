//! Flat Copy-record vectors over the existing authenticated scalar-word host.
//! Each semantic element is a fixed number of scalar words. Intermediate host
//! generations remain in the live call-argument epoch until the grouped commit.
use super::*;

impl Emitter<'_> {
    fn cr_get(&mut self, local: u32) {
        self.output.push(0x20);
        write_u32(self.output, local);
    }
    fn cr_set(&mut self, local: u32) {
        self.output.push(0x21);
        write_u32(self.output, local);
    }
    fn cr_const(&mut self, value: u64) {
        self.output.push(0x42);
        write_i64(self.output, value as i64);
    }
    fn cr_call(&mut self, index: u32) {
        self.output.push(0x10);
        write_u32(self.output, index);
    }
    fn cr_tag(&mut self) {
        self.output.extend([0x41, 0x01]);
    }
    fn cr_count(&mut self, owner: u32, base: u32, capacity: bool) {
        self.cr_get(owner);
        self.cr_tag();
        self.cr_call(base + if capacity { 3 } else { 2 });
    }
    fn cr_index(&mut self, index: u32, width: usize, field: usize) {
        self.cr_get(index);
        self.cr_const(width as u64);
        self.output.push(0x7e);
        self.cr_const(field as u64);
        self.output.push(0x7c);
    }
    fn cr_word(&mut self, owner: u32, index: u32, width: usize, field: usize, base: u32) {
        self.cr_get(owner);
        self.cr_tag();
        self.cr_index(index, width, field);
        self.cr_call(base + 4);
    }
    fn cr_host_result(
        &mut self,
        expr: &ResolvedExpr,
        owner: u32,
        temp: u32,
        status: Option<i32>,
    ) -> Result<(), Diagnostic> {
        self.cr_set(temp);
        self.cr_get(temp);
        self.output.push(0x50);
        if let Some(status) = status {
            self.emit_vec_failure_if(expr, status)?;
        } else {
            self.trap_if();
        }
        self.cr_get(temp);
        self.cr_set(owner);
        Ok(())
    }
    fn cr_order_key(&mut self, ty: &ResolvedType, scratch: u32) {
        match ty {
            ResolvedType::I64 => {
                self.cr_const(0x8000000000000000);
                self.output.push(0x85);
            }
            ResolvedType::I32 => {
                self.cr_const(0xffffffff);
                self.output.push(0x83);
                self.cr_const(0x80000000);
                self.output.push(0x85);
            }
            ResolvedType::F32 | ResolvedType::F64 => {
                let (mask, sign) = if *ty == ResolvedType::F32 {
                    (0xffffffff, 0x80000000)
                } else {
                    (u64::MAX, 0x8000000000000000)
                };
                self.cr_const(mask);
                self.output.push(0x83);
                self.cr_set(scratch);
                self.cr_get(scratch);
                self.cr_const(sign);
                self.output.push(0x83);
                self.output.push(0x50);
                self.output.extend([0x04, I64]);
                self.cr_get(scratch);
                self.cr_const(sign);
                self.output.push(0x85);
                self.output.push(0x05);
                self.cr_get(scratch);
                self.cr_const(mask);
                self.output.push(0x85);
                self.output.push(0x0b);
            }
            _ => {}
        }
    }

    pub(super) fn emit_copy_record_vec(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        use crate::vec_ops::VecOp;
        let fields =
            crate::hir::copy_record_collection::fields(&self.program.declarations, element)
                .ok_or_else(|| error("Copy record Vec element is not admitted"))?
                .to_vec();
        let width = fields.len();
        let base = vec_import_base(self.program);
        let scratch = self
            .plan
            .copy_record_scratch
            .ok_or_else(|| error("Copy record Vec scratch is absent"))?;
        if args.len() != op.arity()
            || args
                .iter()
                .enumerate()
                .any(|(i, a)| !op.accepts_resolved(i, &a.ty, element))
        {
            return Err(error("Copy record Vec arguments are invalid"));
        }
        require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "Copy record Vec result",
        )?;
        let mut values = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            values.push(
                if index == 0 && matches!(op, VecOp::Len | VecOp::Capacity | VecOp::Get) {
                    self.emit_vec_borrow_place(arg, element)?
                } else {
                    self.emit_expr(arg)?
                },
            );
        }
        if op == VecOp::WithCapacity {
            let result = Value::Scalar {
                local: self.plan.expr_scalar(expr)?,
                ty: expr.ty.clone(),
            };
            self.get_scalar(&values[0]);
            self.cr_const(8192 / width as u64);
            self.output.push(0x56);
            self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?;
            self.cr_tag();
            self.get_scalar(&values[0]);
            self.cr_const(width as u64);
            self.output.push(0x7e);
            self.cr_call(base);
            self.cr_set(scalar_local(&result)?);
            self.get_scalar(&result);
            self.output.push(0x50);
            self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?;
            return Ok(result);
        }
        if matches!(op, VecOp::Len | VecOp::Capacity | VecOp::Get) {
            let owner = scalar_local(&values[0])?;
            if op != VecOp::Get {
                self.cr_count(owner, base, op == VecOp::Capacity);
                self.cr_const(width as u64);
                self.output.push(0x80);
                let local = self.plan.expr_scalar(expr)?;
                self.cr_set(local);
                return Ok(Value::Scalar {
                    local,
                    ty: ResolvedType::Usize,
                });
            }
            self.get_scalar(&values[1]);
            self.cr_set(scratch[0]);
            self.cr_get(scratch[0]);
            self.cr_count(owner, base, false);
            self.cr_const(width as u64);
            self.output.push(0x80);
            self.output.push(0x5a);
            self.emit_vec_failure_if(expr, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
            let result = Value::Aggregate {
                pointer: self.plan.expr_pointer(expr)?,
                ty: element.clone(),
            };
            for (index, field) in fields.iter().enumerate() {
                let Value::ScalarMemory { pointer, ty } = self.project_value(&result, &field.id)?
                else {
                    return Err(error("Copy record field is not scalar memory"));
                };
                self.emit_pointer(pointer);
                self.cr_word(owner, scratch[0], width, index, base);
                match ty {
                    ResolvedType::F64 => self.output.push(0xbf),
                    ResolvedType::F32 => self.output.extend([0xa7, 0xbe]),
                    ResolvedType::I64 | ResolvedType::Usize => {}
                    _ => self.output.push(0xa7),
                }
                self.store_scalar(&ty);
            }
            return Ok(result);
        }
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(),
            parameter_index: 0,
            value_expression: args[0].id.clone(),
        };
        let owner = *self
            .plan
            .cleanup_call_argument_carriers
            .get(&epoch)
            .ok_or_else(|| error("Copy record Vec owner epoch is absent"))?;
        let result_local = self.plan.expr_scalar(expr)?;
        match op {
            VecOp::Push | VecOp::Set => {
                if op == VecOp::Push {
                    self.cr_count(owner, base, false);
                    self.cr_count(owner, base, true);
                    self.output.push(0x51);
                    self.emit_vec_failure_if(expr, STATUS_VEC_PUSH_FULL)?;
                } else {
                    self.get_scalar(&values[1]);
                    self.cr_set(scratch[0]);
                    self.cr_get(scratch[0]);
                    self.cr_count(owner, base, false);
                    self.cr_const(width as u64);
                    self.output.push(0x80);
                    self.output.push(0x5a);
                    self.emit_vec_failure_if(expr, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
                }
                let item = &values[if op == VecOp::Push { 1 } else { 2 }];
                for (index, field) in fields.iter().enumerate() {
                    self.cr_get(owner);
                    self.cr_tag();
                    if op == VecOp::Set {
                        self.cr_index(scratch[0], width, index);
                    }
                    self.emit_vec_element_bits(&self.project_value(item, &field.id)?, &field.ty)?;
                    self.cr_call(if op == VecOp::Push {
                        base + 1
                    } else {
                        base + VEC_IMPORT_COUNT + 1
                    });
                    self.cr_host_result(
                        expr,
                        owner,
                        scratch[5],
                        Some(if op == VecOp::Push {
                            STATUS_VEC_PUSH_FULL
                        } else {
                            STATUS_VEC_GET_OUT_OF_BOUNDS
                        }),
                    )?;
                }
            }
            VecOp::ReserveExact => {
                self.get_scalar(&values[1]);
                self.cr_const(8192 / width as u64);
                self.output.push(0x56);
                self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?;
                self.cr_get(owner);
                self.cr_tag();
                self.get_scalar(&values[1]);
                self.cr_const(width as u64);
                self.output.push(0x7e);
                self.cr_call(base + VEC_IMPORT_COUNT);
                self.cr_host_result(expr, owner, scratch[5], Some(STATUS_VEC_ALLOCATION_FAILURE))?;
            }
            VecOp::Clear => {
                self.cr_get(owner);
                self.cr_tag();
                self.cr_call(base + VEC_IMPORT_COUNT + 2);
                self.cr_host_result(expr, owner, scratch[5], None)?;
            }
            VecOp::Sort => {
                self.emit_copy_record_sort(expr, owner, width, &fields, base, scratch)?
            }
            _ => return Err(error("invalid Copy record Vec owner operation")),
        }
        self.cr_get(owner);
        self.cr_set(result_local);
        self.apply_call_commit(&expr.id)?;
        self.cr_const(0);
        self.cr_set(owner);
        Ok(Value::Scalar {
            local: result_local,
            ty: expr.ty.clone(),
        })
    }

    fn emit_copy_record_sort(
        &mut self,
        expr: &ResolvedExpr,
        owner: u32,
        width: usize,
        fields: &[crate::hir::ResolvedFieldDeclaration],
        base: u32,
        s: [u32; 14],
    ) -> Result<(), Diagnostic> {
        self.cr_count(owner, base, false);
        self.cr_const(width as u64);
        self.output.push(0x80);
        self.cr_set(s[2]);
        self.cr_const(1);
        self.cr_set(s[0]);
        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.cr_get(s[0]);
        self.cr_get(s[2]);
        self.output.push(0x5a);
        self.output.extend([0x0d, 0x01]);
        for index in 0..width {
            self.cr_word(owner, s[0], width, index, base);
            self.cr_set(s[6 + index]);
        }
        self.cr_get(s[0]);
        self.cr_set(s[1]);
        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.cr_get(s[1]);
        self.output.push(0x50);
        self.output.extend([0x0d, 0x01]);
        // Form one lexicographic greater-than value. Only the first unequal
        // key decides; floating keys use the scalar total-order bit transform.
        for (index, field) in fields.iter().enumerate() {
            self.cr_get(owner);
            self.cr_tag();
            self.cr_get(s[1]);
            self.cr_const(1);
            self.output.push(0x7d);
            self.cr_const(width as u64);
            self.output.push(0x7e);
            self.cr_const(index as u64);
            self.output.push(0x7c);
            self.cr_call(base + 4);
            self.cr_order_key(&field.ty, s[3]);
            self.cr_set(s[3]);
            self.cr_get(s[6 + index]);
            self.cr_order_key(&field.ty, s[4]);
            self.cr_set(s[4]);
            self.cr_get(s[3]);
            self.cr_get(s[4]);
            self.output.push(0x52);
            self.output.extend([0x04, I32]);
            self.cr_get(s[3]);
            self.cr_get(s[4]);
            self.output.push(0x56);
            self.output.push(0x05);
        }
        self.output.extend([0x41, 0x00]);
        for _ in fields {
            self.output.push(0x0b);
        }
        self.output.push(0x45);
        self.output.extend([0x0d, 0x01]);
        for index in 0..width {
            self.cr_get(owner);
            self.cr_tag();
            self.cr_index(s[1], width, index);
            self.cr_get(owner);
            self.cr_tag();
            self.cr_get(s[1]);
            self.cr_const(1);
            self.output.push(0x7d);
            self.cr_const(width as u64);
            self.output.push(0x7e);
            self.cr_const(index as u64);
            self.output.push(0x7c);
            self.cr_call(base + 4);
            self.cr_call(base + VEC_IMPORT_COUNT + 1);
            self.cr_host_result(expr, owner, s[5], None)?;
        }
        self.cr_get(s[1]);
        self.cr_const(1);
        self.output.push(0x7d);
        self.cr_set(s[1]);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.control_depth -= 2;
        for index in 0..width {
            self.cr_get(owner);
            self.cr_tag();
            self.cr_index(s[1], width, index);
            self.cr_get(s[6 + index]);
            self.cr_call(base + VEC_IMPORT_COUNT + 1);
            self.cr_host_result(expr, owner, s[5], None)?;
        }
        self.cr_get(s[0]);
        self.cr_const(1);
        self.output.push(0x7c);
        self.cr_set(s[0]);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.control_depth -= 2;
        // Renew even an empty/already ordered owner without changing payload.
        self.cr_get(owner);
        self.cr_tag();
        self.cr_const(0);
        self.cr_call(base + VEC_IMPORT_COUNT);
        self.cr_host_result(expr, owner, s[5], None)
    }
}
