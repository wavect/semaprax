//! Shared aggregate result lowering for total byte-slice and borrowed-str reads.
use super::*;

impl Emitter<'_> {
    pub(super) fn emit_indexed_byte_get(
        &mut self,
        expr: &ResolvedExpr,
        values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let pointer = self.plan.expr_pointer(expr)?;
        let layout = variant_layout(self.variant_layouts, &expr.ty)?;
        let none = layout
            .case(&crate::hir::DeclarationId::new(
                crate::prelude::OPTION_NONE_ID,
            ))
            .ok_or_else(|| error("Option<u8> layout has no None case"))?;
        let some_id = crate::hir::DeclarationId::new(crate::prelude::OPTION_SOME_ID);
        let some = layout
            .case(&some_id)
            .ok_or_else(|| error("Option<u8> layout has no Some case"))?;
        let field = some
            .field(&crate::hir::DeclarationId::new(
                crate::prelude::OPTION_SOME_VALUE_ID,
            ))
            .ok_or_else(|| error("Option<u8> layout has no Some payload"))?;
        self.emit_pointer(pointer);
        self.output.extend([0x41, 0x00, 0x41]);
        write_i64(self.output, i64::from(layout.size));
        self.output.extend([0xfc, 0x0b, 0x00]);
        self.get_scalar(&values[0]);
        self.get_scalar(&values[1]);
        self.output.push(0x10);
        write_u32(self.output, self.byte_get_import);
        self.output.extend([0x22]);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x00, 0x4e, 0x04, 0x40]);
        self.emit_pointer(Pointer {
            local: pointer.local,
            offset: pointer.offset + layout.payload_offset + field.offset,
        });
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x3a, 0x00, 0x00]);
        self.emit_pointer(pointer);
        self.output.push(0x41);
        write_i64(self.output, i64::from(some.tag));
        self.output.extend([0x36, 0x02, 0x00, 0x05]);
        self.emit_pointer(pointer);
        self.output.push(0x41);
        write_i64(self.output, i64::from(none.tag));
        self.output.extend([0x36, 0x02, 0x00, 0x0b]);
        self.output.extend([0x41, 0x00, 0x21]);
        write_u32(self.output, self.plan.status);
        Ok(Value::Aggregate {
            pointer,
            ty: expr.ty.clone(),
        })
    }
}
