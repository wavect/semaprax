//! Copy only the active tag and Copy payload; canonical plans move owner leaves.
use super::*;
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn copy_variant_join_carrier(
        &mut self,
        target: &str,
        source: &str,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        let layout = self.variant_layout(ty)?;
        self.line(&format!("memset(&{target}, 0, sizeof({target}));"));
        self.line(&format!("{target}.spx_tag = {source}.spx_tag;"));
        for case in &layout.cases {
            self.line(&format!(
                "if ({source}.spx_tag == UINT32_C({})) {{",
                case.tag
            ));
            self.indent += 1;
            for field in &case.fields {
                if field.ty != ResolvedType::Bytes && field.size != 0 {
                    let member = format!(
                        "spx_payload.{}.{}",
                        c_case_symbol(&case.case),
                        c_field_symbol(&field.field)
                    );
                    self.line(&format!("{target}.{member} = {source}.{member};"));
                }
            }
            self.indent -= 1;
            self.line("}");
        }
        Ok(())
    }
    pub(super) fn finish_variant_join(
        &mut self,
        expression: &ResolvedExpr,
        carrier: &str,
    ) -> Result<(), Diagnostic> {
        if expression.ownership != crate::hir::OwnershipMode::Own
            || variant_declaration_id(self.program, &expression.ty)?.is_none()
        {
            return Ok(());
        }
        let layout = self.variant_layout(&expression.ty)?;
        let emitted = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned variant join lacks cleanup plan"))?
            .apply_variant_join_continuation(&expression.id, carrier, &layout)?;
        for line in emitted.lines() {
            self.line(line);
        }
        Ok(())
    }
}
