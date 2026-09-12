//! Consuming owned-record element boundaries for the native Vec v2 runtime.
//!
//! The SPX-AI-019 element is an ordinary generated C record struct whose two
//! `Bytes` leaves the canonical cleanup plan already tracks. This lowering
//! stages it exactly the way an owned-record argument to a user call is
//! staged — the plan materializes its leaves into the carrier at the declared
//! commit boundary — and then places those leaves, in declaration order, into
//! the runtime's one canonical element slot. Nothing here reinterprets or
//! reorders a cleanup vector; the plan's own transitions are emitted verbatim.
use super::*;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_vec_record_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        // Re-derive admission from declaration facts rather than trusting the
        // caller's dispatch, so a forged or widened HIR gets a diagnostic.
        if !crate::hir::owned_record_collection::admits_vec_operation_element(
            &self.program.declarations,
            op,
            element,
        ) {
            return Err(backend_error(
                "owned record Vec operation is outside the admitted profile",
            ));
        }
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            values.push(self.stage_bytes_call_argument(
                &expr.id,
                index,
                argument,
                op.param_ownership_for(index, element),
                value,
            )?);
        }
        let return_type = op.resolved_return_type(element);
        self.require_type(&expr.ty, &return_type, "owned record Vec operation result")?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned record Vec operation has no cleanup plan"))?;
        let carrier = crate::vec_ops::resolved_vec(element.clone());
        let destination = if op.returns_owner() {
            plan.value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        } else {
            String::new()
        };
        match op {
            crate::vec_ops::VecOp::WithCapacity => {
                self.require_type(&values[0].ty, &ResolvedType::Usize, "Vec capacity")?;
                self.line(&format!(
                    "spx_status = spx_vec_record_with_capacity(spx_ctx, {}, &{destination});",
                    values[0].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            crate::vec_ops::VecOp::Push => {
                self.require_type(&values[0].ty, &carrier, "Vec push owner")?;
                self.require_type(&values[1].ty, element, "Vec push element")?;
                let element_slot = self.stage_record_element(expr, element, &values[1])?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                self.line(&format!(
                    "spx_status = spx_vec_record_push(spx_ctx, &{source}, &{element_slot}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
            }
            crate::vec_ops::VecOp::Clear => {
                self.require_type(&values[0].ty, &carrier, "Vec clear owner")?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                self.line(&format!(
                    "spx_status = spx_vec_record_clear(spx_ctx, &{source}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
            }
            crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity => {
                self.require_type(&values[0].ty, &carrier, "Vec borrow")?;
                let temporary = self.temporary(&ResolvedType::Usize)?;
                let helper = if op == crate::vec_ops::VecOp::Len {
                    "spx_vec_len"
                } else {
                    "spx_vec_capacity"
                };
                self.line(&format!(
                    "{temporary} = {helper}(spx_ctx, &({}), UINT32_C(10));",
                    values[0].code
                ));
                return Ok(CValue {
                    code: temporary,
                    ty: ResolvedType::Usize,
                });
            }
            crate::vec_ops::VecOp::Get
            | crate::vec_ops::VecOp::Set
            | crate::vec_ops::VecOp::ReserveExact => {
                return Err(backend_error(
                    "owned record Vec operation is outside the admitted profile",
                ))
            }
        }
        for line in plan.apply_at(&expr.id)?.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
            ty: return_type,
        })
    }

    /// Move one staged element into the runtime's canonical slot and return
    /// the C name of that slot.
    ///
    /// The plan's own `materialize_record_carrier` transitions commit the
    /// element's `Bytes` leaves into the generated record struct first, exactly
    /// as an owned-record argument to a user call does; only then are the two
    /// leaves moved into the slot in declaration order. From that point the
    /// runtime owns them on every path, including a refused push.
    fn stage_record_element(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        value: &CValue,
    ) -> Result<String, Diagnostic> {
        let fields = crate::hir::owned_record_collection::owned_record_element_fields(
            &self.program.declarations,
            element,
        )
        .ok_or_else(|| backend_error("owned record Vec element is not the admitted shape"))?;
        let owned = [
            super::super::super::c_field_symbol(&fields.owned[0].id),
            super::super::super::c_field_symbol(&fields.owned[1].id),
        ];
        let scalar = super::super::super::c_field_symbol(&fields.scalar.id);
        let scalar_bits = super::vec_scalar_to_bits(&CValue {
            code: format!("({}).{scalar}", value.code),
            ty: fields.scalar.ty.clone(),
        });
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned record Vec push has no cleanup plan"))?;
        let storage = plan.call_argument_storage(&expr.id, 1)?;
        for line in plan
            .materialize_record_carrier(&storage, &value.code)?
            .lines()
        {
            self.line(line);
        }
        let slot = format!("spx_vec_record_element_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!("spx_vec_record_v1 {slot} = {{0}};"));
        for (index, field) in owned.iter().enumerate() {
            self.line(&format!(
                "{slot}.spx_owned[{index}] = spx_bytes_move(&({}).{field});",
                value.code
            ));
        }
        self.line(&format!("{slot}.spx_scalar = {scalar_bits};"));
        Ok(slot)
    }
}
