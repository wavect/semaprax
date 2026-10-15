//! A single private scalar store with the ordinary fallible owning commit.
use super::*;
pub(in crate::wasm::aggregate) const IMPORT: &str = "spx_vec_update_field_v1";
fn import_index(program: &ResolvedProgram) -> u32 {
    super::sort_keys::import_index(program)
        + u32::from(super::binds_record_keys(program))
        + u32::from(crate::vec_ops::field_unique::uses(program))
}
impl Emitter<'_> {
    pub(in crate::wasm::aggregate) fn emit_field_update(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let call = crate::vec_ops::field_update::inspect(expression)
            .ok_or_else(|| error("invalid checked field update call"))?;
        let (position, scalar) = crate::vec_ops::field_update::position(
            &self.program.declarations,
            call.element,
            call.identity,
        )
        .ok_or_else(|| error("field update descriptor failed independent replay"))?;
        require_type(&call.value.ty, &scalar, "field update scalar")?;
        let descriptor = descriptor(self.program, call.element)?;
        if descriptor
            .fields
            .get(position)
            .is_none_or(|(_, ty)| ty != &scalar)
        {
            return Err(error(
                "field update ordinal/type disagrees with Wasm descriptor",
            ));
        }
        let _ = self.emit_expr(call.source)?;
        let index = self.emit_expr(call.index)?;
        self.require_scalar(&index, &ResolvedType::Usize, "field update index")?;
        let value = self.emit_expr(call.value)?;
        self.require_scalar(&value, &scalar, "field update value")?;
        let epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expression.id.clone(),
            parameter_index: 0,
            value_expression: call.source.id.clone(),
        };
        let owner = Value::Scalar {
            local: *self
                .plan
                .cleanup_call_argument_carriers
                .get(&epoch)
                .ok_or_else(|| error("field update owner has no checked staged epoch"))?,
            ty: call.source.ty.clone(),
        };
        let result = self.leaf_result(expression)?;
        // The RHS has completed before the checked index failure can select.
        self.leaf_preflight_index(expression, &owner, descriptor.tag, &index)?;
        self.get_scalar(&owner);
        self.leaf_metadata(&descriptor);
        self.get_scalar(&index);
        self.leaf_const(i64::try_from(position).map_err(|_| error("field ordinal overflow"))?);
        self.emit_vec_element_bits(&value, &scalar)?;
        self.output.push(0x10);
        write_u32(self.output, import_index(self.program));
        self.leaf_null_result(expression, &result, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
        self.apply_call_commit(&expression.id)?;
        self.clear_scalar(&owner)?;
        Ok(result)
    }
}
