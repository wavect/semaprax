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
            let mut pending = vec![guard];
            while let Some(expression) = pending.pop() {
                if self
                    .program
                    .declarations
                    .type_facts(&expression.ty)
                    .is_some_and(|facts| facts.contains_resource)
                {
                    return Err(backend_error(
                        "Copy variant guard resource settlement is outside the native profile",
                    ));
                }
                pending.extend(super::resolved_expr_children(expression));
            }
            self.line(&format!("{matched} = false;"));
            let flag = self.emit_expr(guard)?;
            self.require_type(&flag.ty, &ResolvedType::Bool, "variant match guard")?;
            self.emit_variant_guard_scope_exit(guard)?;
            self.line(&format!("if ({}) {{", flag.code));
            self.indent += 1;
            self.line(&format!("{matched} = true;"));
        }
        Ok(())
    }
    // Choose only exact owned carriers in this emitter's authenticated plan.
    // Ancestor selection and finalizer ordering remain canonical plan data.
    fn emit_variant_guard_scope_exit(&mut self, guard: &ResolvedExpr) -> Result<(), Diagnostic> {
        let Some(plan) = self.bytes_plan else {
            return Ok(());
        };
        let mut anchors = BTreeSet::new();
        let mut pending = vec![guard];
        while let Some(expression) = pending.pop() {
            let storage = crate::cleanup_plan::StorageId::Temporary(expression.id.clone());
            if (plan.value(&storage).is_ok() || plan.has_projected_leaves(&storage))
                && plan.has_runtime_lifecycle(&storage)
            {
                anchors.insert(storage);
            }
            pending.extend(super::resolved_expr_children(expression));
        }
        if anchors.is_empty() {
            return Ok(());
        }
        let cleanup = plan.scalar_match_guard_scope_exit(&guard.id, &anchors)?;
        for line in cleanup.lines() {
            self.line(line);
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
