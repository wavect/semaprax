//! Nonallocating reads from an independently authenticated owned-leaf row.
use super::super::super::{backend_error, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};

impl<O: COutput> CEmitter<'_, O> {
    pub(in crate::codegen::native_emit::expression) fn emit_vec_field_read(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        field: &DeclarationId,
        bytes: bool,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        let [source, index] = args else {
            return Err(backend_error(
                "Vec field read requires exactly two runtime children",
            ));
        };
        let ResolvedExprKind::Place(place) = &source.kind else {
            return Err(backend_error(
                "Vec field read requires one authenticated named place",
            ));
        };
        if !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
            || index.ownership != OwnershipMode::Value
        {
            return Err(backend_error("Vec field read has invalid child ownership"));
        }
        let selected = crate::hir::vec_field::field(&self.program.declarations, element, field)
            .ok_or_else(|| backend_error("Vec field identity or record descriptor is invalid"))?;
        let result_type = selected
            .result_type(bytes)
            .ok_or_else(|| backend_error("Vec field byte fusion or result type is invalid"))?;
        let position = selected.position;
        let field_type = selected.declaration.ty.clone();
        let expected_fields = selected.layout.fields.map_or(0, <[_]>::len);
        let result_mode = if matches!(result_type, ResolvedType::Str | ResolvedType::SliceU8) {
            OwnershipMode::Borrow
        } else {
            OwnershipMode::Value
        };
        if expr.ownership != result_mode {
            return Err(backend_error("Vec field read has invalid result ownership"));
        }
        self.require_type(&expr.ty, &result_type, "Vec field read result")?;
        self.require_type(
            &source.ty,
            &crate::vec_ops::resolved_vec(element.clone()),
            "Vec field source",
        )?;
        self.require_type(&index.ty, &ResolvedType::Usize, "Vec field index")?;
        let layout = crate::codegen::native_vec::owned_leaf::layout(self.program, element)
            .ok_or_else(|| backend_error("Vec field read has no native row descriptor"))?;
        if layout.kinds.len() != expected_fields || position >= layout.offsets.len() {
            return Err(backend_error(
                "Vec field declaration order disagrees with native layout",
            ));
        }
        // Borrow the named source before evaluating the index. Generic owning
        // expression lowering would transfer a carrier that this operation retains.
        let source_value = self.emit_place(place)?;
        self.require_type(&source_value.ty, &source.ty, "Vec field source place")?;
        let index_value = self.emit_expr(index)?;
        crate::codegen::native_bytes::NativeBytesPlan::authenticate_vec_field_commit(
            self.function,
            &expr.id,
        )?;
        let suffix = self.next_local;
        self.next_local += 1;
        let slot = format!("spx_leaf_field_{suffix}");
        self.line(&format!("const unsigned char *{slot} = NULL;"));
        self.line(&format!(
            "spx_status = spx_leaf_read_field(spx_ctx, &({}), &{}, {}, UINT32_C({position}), &{slot});",
            source_value.code, layout.symbol, index_value.code
        ));
        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        let output = self.temporary(&result_type)?;
        match field_type {
            ResolvedType::String => {
                let raw = format!("spx_leaf_string_{suffix}");
                self.line(&format!("const char *{raw} = NULL;"));
                self.line(&format!("memcpy(&{raw}, {slot}, sizeof({raw}));"));
                if bytes {
                    self.emit_str_as_bytes_view(
                        &output,
                        &CValue {
                            code: raw,
                            ty: ResolvedType::String,
                        },
                        expr,
                    )?;
                } else {
                    self.line(&format!("{output} = spx_string_as_str({raw});"));
                }
            }
            ResolvedType::Bytes => {
                let raw = format!("spx_leaf_bytes_{suffix}");
                self.line(&format!("spx_bytes_v1 {raw} = {{0}};"));
                self.line(&format!("memcpy(&{raw}, {slot}, sizeof({raw}));"));
                self.line(&format!("{output} = spx_bytes_as_slice(&{raw});"));
            }
            scalar => {
                let bits = self.temporary(&ResolvedType::Usize)?;
                self.line(&format!("memcpy(&{bits}, {slot}, sizeof({bits}));"));
                self.line(&format!(
                    "{output} = {};",
                    super::vec_bits_to_scalar(&bits, &scalar)?
                ));
            }
        }
        let value = CValue {
            code: output,
            ty: result_type,
        };
        self.apply_owned_plan_at_value(&expr.id, &value)?;
        Ok(value)
    }
}
