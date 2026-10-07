//! Case-matched scalar guards precede selection and arm result settlement.
use super::*;
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn open_variant_guard(
        &mut self,
        guard: Option<&ResolvedExpr>,
        matched: &str,
    ) -> Result<(), Diagnostic> {
        if let Some(guard) = guard {
            if !crate::variant_guards::guard_shape(guard) {
                return Err(backend_error("invalid Copy variant guard"));
            }
            self.line(&format!("{matched} = false;"));
            let flag = self.emit_expr(guard)?;
            self.require_type(&flag.ty, &ResolvedType::Bool, "variant match guard")?;
            self.line(&format!("if ({}) {{", flag.code));
            self.indent += 1;
            self.line(&format!("{matched} = true;"));
        }
        Ok(())
    }
    pub(super) fn close_variant_guard(&mut self, guarded: bool) {
        if guarded {
            self.indent -= 1;
            self.line("}");
        }
    }
}
