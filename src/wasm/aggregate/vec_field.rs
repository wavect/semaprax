//! Scoped read of one authenticated field; no owning argument epoch or result.
use crate::hir;

use super::*;

pub(super) fn uses(program: &ResolvedProgram) -> bool {
    program.functions.iter().chain(program.function_instances.iter().map(|i| &i.function))
        .any(|function| std::iter::once(&function.body).chain(&function.requires).chain(&function.ensures)
            .any(|root| {
                let mut pending = vec![root];
                while let Some(expression) = pending.pop() {
                    if matches!(expression.kind, ResolvedExprKind::VecFieldRead { .. }) {
                        return true;
                    }
                    hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
                }
                false
            }))
}

/// Only additive read programs construct legacy storage with its descriptor
/// already installed. Their existing tag-10 push/length/drop routes stay exact.
pub(super) fn typed_legacy_constructor(program: &ResolvedProgram, op: crate::vec_ops::VecOp) -> bool {
    op == crate::vec_ops::VecOp::WithCapacity && uses(program)
}

impl FunctionPlan {
    pub(super) fn collect_vec_field_scratch(
        &mut self, expression: &ResolvedExpr, frame: &mut FrameAllocator,
    ) -> Result<(), Diagnostic> {
        if matches!(expression.kind, ResolvedExprKind::VecFieldRead { .. })
            && self.owned_leaf_scratch.insert(expression.id.clone(), frame.allocate(8, 8)?).is_some()
        {
            return Err(error("scoped Vec field scratch identity repeats"));
        }
        Ok(())
    }
}

impl Emitter<'_> {
    pub(super) fn emit_vec_field_read(
        &mut self, expression: &ResolvedExpr, element: &ResolvedType,
        selected: &DeclarationId, bytes: bool, args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let field = hir::vec_field::field(&self.program.declarations, element, selected)
            .ok_or_else(|| error("scoped Vec field declaration is unauthenticated"))?;
        let result_type = field.result_type(bytes)
            .ok_or_else(|| error("scoped Vec field byte-view flag is invalid"))?;
        let borrowed = matches!(result_type, ResolvedType::Str | ResolvedType::SliceU8);
        if args.len() != 2
            || args[0].ty != crate::vec_ops::resolved_vec(element.clone())
            || !matches!(args[0].ownership, hir::OwnershipMode::Own | hir::OwnershipMode::Borrow)
            || args[1].ty != ResolvedType::Usize || args[1].ownership != hir::OwnershipMode::Value
            || expression.ty != result_type
            || expression.ownership != if borrowed { hir::OwnershipMode::Borrow } else { hir::OwnershipMode::Value }
        {
            return Err(error("scoped Vec field children or result disagree with checked HIR"));
        }
        let position = field.position;
        let stored_type = field.declaration.ty.clone();
        let descriptor = vec_owned_leaf::descriptor(self.program, element)?;
        if descriptor.fields.get(position).is_none_or(|(id, ty)|
            id.as_ref() != Some(selected) || ty != &stored_type)
        {
            return Err(error("scoped Vec field descriptor differs from its declaration"));
        }
        // A Place is inspected before the index. It keeps its Own/Borrow
        // binding mode; the operation borrows it and never stages a transfer.
        let owner = self.emit_vec_borrow_place(&args[0], element)?;
        let index = self.emit_expr(&args[1])?;
        self.require_scalar(&index, &ResolvedType::Usize, "scoped Vec field index")?;
        self.apply_call_commit(&expression.id)?; // canonical zero-owned commit before the operation
        let scratch = Pointer { local: self.plan.frame_base,
            offset: *self.plan.owned_leaf_scratch.get(&expression.id)
                .ok_or_else(|| error("scoped Vec field scratch is absent"))? };
        self.get_scalar(&owner);
        for constant in [descriptor.identity, descriptor.shape] {
            self.output.push(0x42); write_i64(self.output, constant);
        }
        self.get_scalar(&index);
        self.output.push(0x42); write_i64(self.output, position as i64);
        self.emit_pointer(scratch);
        self.output.push(0x10);
        write_u32(self.output, vec_owned_leaf::import_base(self.program) + vec_owned_leaf::IMPORT_COUNT);
        self.output.push(0x22); write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x02, 0x46]); // authenticated index refusal
        self.emit_vec_failure_if(expression, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
        self.output.push(0x20); write_u32(self.output, self.plan.status);
        self.output.extend([0x45, 0x45]); // all other nonzero statuses break the private contract
        self.trap_if();
        if borrowed {
            self.emit_pointer(scratch); self.load_scalar(&ResolvedType::I64);
            self.output.extend([0x50, 0x04, 0x40, 0x05]); // zero is the empty owned payload
            self.emit_pointer(scratch); self.load_scalar(&ResolvedType::I64);
            self.output.push(0x42); write_i64(self.output, i64::MIN);
            self.output.extend([0x83, 0x50]);
            self.trap_if(); // nonempty result must retain the original owned child
            self.emit_pointer(scratch); self.load_scalar(&ResolvedType::I64);
            self.output.push(0x10); write_u32(self.output, BYTE_AS_SLICE_IMPORT);
            self.output.push(0x1a); self.output.push(0x0b);
        } else {
            self.validate_record_scalar_bits(scratch, &stored_type)?;
        }
        let result = Value::Scalar { local: self.plan.expr_scalar(expression)?, ty: result_type };
        self.emit_pointer(scratch); self.load_scalar(&ResolvedType::I64);
        if borrowed {
            self.output.push(0x21); write_u32(self.output, scalar_local(&result)?);
        } else {
            self.store_vec_element_bits(&result, &stored_type)?;
        }
        Ok(result)
    }
}
