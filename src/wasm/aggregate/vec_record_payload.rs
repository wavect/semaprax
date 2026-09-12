//! Consuming owned-record element boundaries for the Wasm owned-payload host.
//!
//! The SPX-AI-019 element is an ordinary flat record in linear memory whose two
//! `Bytes` leaves the canonical cleanup plan already tracks as projected
//! liveness. The carrier itself is not in linear memory at all: it is one `i64`
//! host handle, exactly as `Vec<Bytes>` already is. So the element crosses the
//! boundary as the three words it owns — two `Bytes` handles and one Copy
//! scalar — and the host owns them from that point.
//!
//! That keeps the whole boundary two-way rather than three-way. Every operation
//! but `push` reuses the existing owned-payload import with the record element
//! tag, because their signatures do not mention the element at all; only `push`
//! needs a wider one, so the profile adds exactly one import rather than a third
//! set of nine. The profile admits no `get`, so nothing ever reads an element
//! back out of the host and the placement is write-only bookkeeping.
//!
//! Failure settles the way the owned-payload boundary already settles: a host
//! that refuses an operation consumes nothing and returns the null handle, and
//! the caller's own `CleanupPlan` flags — still live, because the call commit
//! has not run yet — drop the carrier and the staged element exactly once.
//! Nothing here reorders or repairs a cleanup vector; the plan's own
//! transitions are replayed verbatim.
use super::*;

/// The one function the owned-record element adds to the owned-payload host
/// boundary. It is versioned with that boundary because it only ever appears
/// beside it.
pub(super) const RECORD_PUSH_IMPORT: &str = "spx_vec_record_push_v2";

/// Index of that import, which follows the owned-payload set and the extended
/// set when either is present.
pub(super) fn record_push_import_index(program: &ResolvedProgram) -> u32 {
    vec_import_base(program)
        + if super::super::program_uses_vec(program) {
            VEC_IMPORT_COUNT
        } else {
            0
        }
        + if super::super::vec_ops::program_uses_extended_vec(program) {
            EXTENDED_VEC_IMPORT_COUNT
        } else {
            0
        }
}

impl Emitter<'_> {
    /// Lower one admitted bounded `Vec` operation over the owned-record element.
    pub(super) fn emit_vec_record_payload(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        use crate::vec_ops::VecOp;
        // Admission is re-derived from declaration facts here rather than
        // trusted from the caller's dispatch, so forged or widened HIR gets a
        // diagnostic instead of reaching a lowering with no layout for it.
        if !crate::hir::owned_record_collection::admits_vec_operation_element(
            &self.program.declarations,
            op,
            element,
        ) || args.len() != op.arity()
        {
            return Err(error(
                "owned record Vec operation is outside the admitted profile",
            ));
        }
        for (index, argument) in args.iter().enumerate() {
            if !op.accepts_resolved(index, &argument.ty, element) {
                return Err(error(
                    "Vec operation argument type disagrees with resolved HIR",
                ));
            }
        }
        let carrier = crate::vec_ops::resolved_vec(element.clone());
        require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "Vec operation result",
        )?;
        let tag = i64::from(crate::wasm::vec_ops::RECORD_ELEMENT_TAG);
        let base = vec_import_base(self.program);
        match op {
            VecOp::WithCapacity => {
                let capacity = self.emit_expr(&args[0])?;
                self.require_scalar(&capacity, &ResolvedType::Usize, "Vec capacity")?;
                let result = self.record_result(expr)?;
                self.output.push(0x41);
                write_i64(self.output, tag);
                self.get_scalar(&capacity);
                self.output.push(0x10);
                write_u32(self.output, base);
                self.store_record_result(&result)?;
                self.emit_vec_failure_if(expr, STATUS_VEC_ALLOCATION_FAILURE)?;
                Ok(result)
            }
            VecOp::Push => {
                let owner = self.emit_expr(&args[0])?;
                let element_value = self.emit_expr(&args[1])?;
                self.require_scalar(&owner, &carrier, "Vec push owner")?;
                require_type(value_type(&element_value), element, "Vec push element")?;
                // The staged element is an aggregate epoch, so cleanup replay
                // resolves its projected leaves through the same authenticated
                // carrier an ordinary owned-record call argument registers.
                let element_epoch = crate::cleanup_plan::StorageId::CallArgument {
                    call: expr.id.clone(),
                    parameter_index: 1,
                    value_expression: args[1].id.clone(),
                };
                if self.plan.cleanup_storage_types.contains_key(&element_epoch)
                    && self
                        .call_argument_values
                        .insert(element_epoch, element_value.clone())
                        .is_some()
                {
                    return Err(error("projected call epoch carrier is not unique"));
                }
                let source = self.record_owner_epoch(expr, args, &carrier)?;
                let fields = crate::hir::owned_record_collection::owned_record_element_fields(
                    &self.program.declarations,
                    element,
                )
                .ok_or_else(|| error("owned record Vec element is not the admitted shape"))?;
                let owned = [fields.owned[0].id.clone(), fields.owned[1].id.clone()];
                let (scalar_field, scalar_type) =
                    (fields.scalar.id.clone(), fields.scalar.ty.clone());
                let result = self.record_result(expr)?;
                self.get_scalar(&source);
                self.output.push(0x41);
                write_i64(self.output, tag);
                for field in &owned {
                    let leaf = self.project_value(&element_value, field)?;
                    self.get_scalar(&leaf);
                }
                let scalar = self.project_value(&element_value, &scalar_field)?;
                self.emit_vec_element_bits(&scalar, &scalar_type)?;
                self.output.push(0x10);
                write_u32(self.output, record_push_import_index(self.program));
                self.store_record_result(&result)?;
                self.emit_vec_failure_if(expr, STATUS_VEC_PUSH_FULL)?;
                // The host now owns the carrier and both leaves. The commit
                // clears the plan's own flags first; only then is the moved-out
                // memory poisoned, so no path can read a transferred handle.
                self.apply_call_commit(&expr.id)?;
                self.clear_scalar(&source)?;
                for field in &owned {
                    let leaf = self.project_value(&element_value, field)?;
                    self.clear_scalar(&leaf)?;
                }
                Ok(result)
            }
            VecOp::Clear => {
                let owner = self.emit_expr(&args[0])?;
                self.require_scalar(&owner, &carrier, "Vec clear owner")?;
                let source = self.record_owner_epoch(expr, args, &carrier)?;
                let result = self.record_result(expr)?;
                self.get_scalar(&source);
                self.output.push(0x41);
                write_i64(self.output, tag);
                self.output.push(0x10);
                write_u32(self.output, base + VEC_IMPORT_COUNT + 2);
                self.store_record_result(&result)?;
                // `clear` cannot refuse: it drops what is already owned and
                // keeps the capacity. A null handle is a broken host, not a
                // selectable status.
                self.trap_if();
                self.apply_call_commit(&expr.id)?;
                self.clear_scalar(&source)?;
                Ok(result)
            }
            VecOp::Len | VecOp::Capacity => {
                let borrowed = self.emit_vec_borrow_place(&args[0], element)?;
                let result = self.record_result(expr)?;
                self.get_scalar(&borrowed);
                self.output.push(0x41);
                write_i64(self.output, tag);
                self.output.push(0x10);
                write_u32(self.output, base + if op == VecOp::Len { 2 } else { 3 });
                self.output.push(0x21);
                write_u32(self.output, scalar_local(&result)?);
                Ok(result)
            }
            VecOp::Get | VecOp::Set | VecOp::ReserveExact => Err(error(
                "owned record Vec operation is outside the admitted profile",
            )),
        }
    }

    /// The result local this operation publishes into.
    fn record_result(&mut self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        Ok(Value::Scalar {
            local: self.plan.expr_scalar(expr)?,
            ty: expr.ty.clone(),
        })
    }

    /// Store the host's returned handle and leave the null test on the stack.
    fn store_record_result(&mut self, result: &Value) -> Result<(), Diagnostic> {
        self.output.push(0x21);
        write_u32(self.output, scalar_local(result)?);
        self.get_scalar(result);
        self.output.push(0x50); // i64.eqz
        Ok(())
    }

    /// Only the authenticated call-argument epoch crosses the host boundary.
    fn record_owner_epoch(
        &self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
        carrier: &ResolvedType,
    ) -> Result<Value, Diagnostic> {
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(),
            parameter_index: 0,
            value_expression: args[0].id.clone(),
        };
        Ok(Value::Scalar {
            local: self
                .plan
                .cleanup_call_argument_carriers
                .get(&epoch)
                .copied()
                .ok_or_else(|| {
                    error("owned record Vec operation requires a checked staged owner")
                })?,
            ty: carrier.clone(),
        })
    }
}
