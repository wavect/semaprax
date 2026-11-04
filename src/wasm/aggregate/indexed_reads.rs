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
    pub(super) fn emit_zero_prefix(
        &mut self,
        expr: &ResolvedExpr,
        values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let local = self.plan.expr_scalar(expr)?;
        self.output.extend([0x42, 0x00, 0x21]);
        write_u32(self.output, local);
        self.output.extend([0x02, 0x40, 0x03, 0x40, 0x20]);
        write_u32(self.output, local);
        self.get_scalar(&values[0]);
        self.output.push(0x10);
        write_u32(self.output, BYTE_AS_SLICE_IMPORT);
        self.output.extend([0xa7, 0xad, 0x5a, 0x0d, 0x01]);
        self.get_scalar(&values[0]);
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.push(0x10);
        write_u32(self.output, self.byte_get_import);
        self.output.extend([0x41, 0x00, 0x47, 0x0d, 0x01, 0x20]);
        write_u32(self.output, local);
        self.output.extend([0x42, 0x01, 0x7c, 0x21]);
        write_u32(self.output, local);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        Ok(Value::Scalar {
            local,
            ty: ResolvedType::Usize,
        })
    }
    pub(super) fn emit_indexed_word_get(
        &mut self,
        expr: &ResolvedExpr,
        values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let pointer = self.plan.expr_pointer(expr)?;
        let layout = variant_layout(self.variant_layouts, &expr.ty)?;
        let none = layout
            .case(&DeclarationId::new(crate::prelude::OPTION_NONE_ID))
            .ok_or_else(|| error("byte_read_le Option has no None case"))?;
        let some = layout
            .case(&DeclarationId::new(crate::prelude::OPTION_SOME_ID))
            .ok_or_else(|| error("byte_read_le Option has no Some case"))?;
        let field = some
            .field(&DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID))
            .ok_or_else(|| error("byte_read_le Option has no Some payload"))?;
        if field.ty != ResolvedType::Usize {
            return Err(error("byte_read_le Option payload is not usize"));
        }
        let payload = Pointer {
            local: pointer.local,
            offset: pointer.offset + layout.payload_offset + field.offset,
        };
        self.emit_pointer(pointer);
        self.output.extend([0x41, 0x00, 0x41]);
        write_i64(self.output, i64::from(layout.size));
        self.output.extend([0xfc, 0x0b, 0x00]);
        self.emit_pointer(pointer);
        self.output.push(0x41);
        write_i64(self.output, i64::from(none.tag));
        self.output.extend([0x36, 0x02, 0x00]);
        self.get_scalar(&values[0]);
        self.output.push(0x10);
        write_u32(self.output, BYTE_AS_SLICE_IMPORT);
        self.output.extend([0xa7, 0x21]);
        write_u32(self.output, self.plan.status);
        self.get_scalar(&values[2]);
        self.output.push(0x42);
        write_i64(self.output, 1);
        self.output.push(0x5a);
        self.get_scalar(&values[2]);
        self.output.push(0x42);
        write_i64(self.output, 8);
        self.output.extend([0x58, 0x71]);
        self.get_scalar(&values[1]);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0xad, 0x58, 0x71, 0x04, 0x40]);
        self.get_scalar(&values[2]);
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.push(0xad);
        self.get_scalar(&values[1]);
        self.output.extend([0x7d, 0x58, 0x04, 0x40]);
        self.emit_pointer(pointer);
        self.output.push(0x41);
        write_i64(self.output, i64::from(some.tag));
        self.output.extend([0x36, 0x02, 0x00]);
        for byte in 0..8 {
            self.get_scalar(&values[2]);
            self.output.push(0x42);
            write_i64(self.output, byte);
            self.output.push(0x56);
            self.emit_pointer(pointer);
            self.output.extend([0x28, 0x02, 0x00, 0x41]);
            write_i64(self.output, i64::from(some.tag));
            self.output.extend([0x46, 0x71, 0x04, 0x40]);
            self.get_scalar(&values[0]);
            self.get_scalar(&values[1]);
            self.output.push(0x42);
            write_i64(self.output, byte);
            self.output.extend([0x7c, 0x10]);
            write_u32(self.output, self.byte_get_import);
            self.output.push(0x22);
            write_u32(self.output, self.plan.status);
            self.output.extend([0x41, 0x00, 0x4e, 0x04, 0x40]);
            self.emit_pointer(payload);
            self.emit_pointer(payload);
            self.output.extend([0x29, 0x00, 0x00, 0x20]);
            write_u32(self.output, self.plan.status);
            self.output.extend([0xad, 0x42]);
            write_i64(self.output, byte * 8);
            self.output.extend([0x86, 0x84, 0x37, 0x00, 0x00, 0x05]);
            self.emit_pointer(payload);
            self.output.push(0x42);
            write_i64(self.output, 0);
            self.output.extend([0x37, 0x00, 0x00]);
            self.emit_pointer(pointer);
            self.output.push(0x41);
            write_i64(self.output, i64::from(none.tag));
            self.output.extend([0x36, 0x02, 0x00, 0x0b, 0x0b]);
        }
        self.output.extend([0x0b, 0x0b, 0x41, 0x00, 0x21]);
        write_u32(self.output, self.plan.status);
        Ok(Value::Aggregate {
            pointer,
            ty: expr.ty.clone(),
        })
    }
}
