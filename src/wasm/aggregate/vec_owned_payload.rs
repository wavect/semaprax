//! Versioned host boundary for a vector that owns Bytes payload handles.
use super::*;

pub(super) fn import_names(program: &ResolvedProgram) -> [&'static str; 9] {
    if crate::vec_ops::resolved_program_uses_owned_payload(program) {
        [
            "spx_vec_with_capacity_v2",
            "spx_vec_push_v2",
            "spx_vec_len_v2",
            "spx_vec_capacity_v2",
            "spx_vec_get_v2",
            "spx_vec_drop_v2",
            "spx_vec_reserve_exact_v2",
            "spx_vec_set_v2",
            "spx_vec_clear_v2",
        ]
    } else {
        [
            "spx_vec_with_capacity",
            "spx_vec_push",
            "spx_vec_len",
            "spx_vec_capacity",
            "spx_vec_get",
            "spx_vec_drop",
            "spx_vec_reserve_exact",
            "spx_vec_set",
            "spx_vec_clear",
        ]
    }
}

impl Emitter<'_> {
    fn vec_owned_argument(
        &self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
        index: usize,
    ) -> Result<Value, Diagnostic> {
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(),
            parameter_index: index as u32,
            value_expression: args[index].id.clone(),
        };
        Ok(Value::Scalar {
            local: self
                .plan
                .cleanup_call_argument_carriers
                .get(&epoch)
                .copied()
                .ok_or_else(|| error("owned Vec payload requires a checked staged argument"))?,
            ty: args[index].ty.clone(),
        })
    }

    pub(super) fn emit_vec_owned_payload(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        use crate::vec_ops::VecOp;
        if op == VecOp::Get {
            return Err(error("owned Vec payload cannot be copied"));
        }
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = if index == 0 && matches!(op, VecOp::Len | VecOp::Capacity) {
                self.emit_vec_borrow_place(argument, &ResolvedType::Bytes)?
            } else {
                self.emit_expr(argument)?
            };
            values.push(value);
        }
        // Child evaluation may already have moved its result into the canonical
        // call-argument epoch. Only that authenticated epoch crosses the host.
        for (index, value) in values.iter_mut().enumerate() {
            if op.param_ownership_for(index, &ResolvedType::Bytes) == crate::hir::OwnershipMode::Own
            {
                *value = self.vec_owned_argument(expr, args, index)?;
            }
        }
        let result = Value::Scalar {
            local: self.plan.expr_scalar(expr)?,
            ty: expr.ty.clone(),
        };
        if op == VecOp::WithCapacity {
            self.output.push(0x41);
            write_i64(self.output, 9);
            self.get_scalar(&values[0]);
        } else {
            self.get_scalar(&values[0]);
            self.output.push(0x41);
            write_i64(self.output, 9);
            for value in &values[1..] {
                self.get_scalar(value);
            }
        }
        let offset = match op {
            VecOp::WithCapacity => 0,
            VecOp::Push => 1,
            VecOp::Len => 2,
            VecOp::Capacity => 3,
            VecOp::ReserveExact => VEC_IMPORT_COUNT,
            VecOp::Set => VEC_IMPORT_COUNT + 1,
            VecOp::Clear => VEC_IMPORT_COUNT + 2,
            VecOp::Get => unreachable!(),
        };
        self.output.push(0x10);
        write_u32(self.output, vec_import_base(self.program) + offset);
        self.output.push(0x21);
        write_u32(self.output, scalar_local(&result)?);
        if op.returns_owner() {
            self.get_scalar(&result);
            self.output.push(0x50);
            match op {
                VecOp::WithCapacity | VecOp::ReserveExact => {
                    self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?
                }
                VecOp::Push => self.emit_vec_failure_if(expr, STATUS_VEC_PUSH_FULL)?,
                VecOp::Set => self.emit_vec_failure_if(expr, STATUS_VEC_GET_OUT_OF_BOUNDS)?,
                VecOp::Clear => self.trap_if(),
                _ => unreachable!(),
            }
        }
        if op.reopens_same_owner() {
            self.apply_call_commit(&expr.id)?;
            for (index, value) in values.iter().enumerate() {
                if op.param_ownership_for(index, &ResolvedType::Bytes)
                    == crate::hir::OwnershipMode::Own
                {
                    self.clear_scalar(value)?;
                }
            }
        }
        Ok(result)
    }
}
