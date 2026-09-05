use crate::diagnostic::Diagnostic;
use crate::hir::{self, ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedType};

use super::{backend_error, variant_declaration_id, CEmitter, COutput, CValue};

// These helpers apply the authenticated owned-Bytes plan while expression
// lowering remains in the parent module.
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn apply_owned_plan_at_value(
        &mut self,
        at: &ExpressionId,
        value: &CValue,
    ) -> Result<(), Diagnostic> {
        let Some(plan) = self.bytes_plan else {
            return Ok(());
        };
        let transitions = if variant_declaration_id(self.program, &value.ty)?.is_some() {
            let layout = self.variant_layout(&value.ty)?;
            plan.apply_variant_at(at, &value.code, &layout)?
        } else {
            plan.apply_at(at)?
        };
        for line in transitions.lines() {
            self.line(line);
        }
        Ok(())
    }

    pub(super) fn stage_bytes_call_argument(
        &mut self,
        call: &ResolvedExpr,
        index: usize,
        argument: &ResolvedExpr,
        ownership: hir::OwnershipMode,
        mut value: CValue,
    ) -> Result<CValue, Diagnostic> {
        if !matches!(value.ty, ResolvedType::Bytes) {
            return Ok(value);
        }
        if ownership == hir::OwnershipMode::Borrow {
            if !matches!(argument.kind, ResolvedExprKind::Place(_)) {
                return Err(backend_error(
                    "borrowed Bytes call argument is not one authenticated place",
                ));
            }
            return Ok(value);
        }
        if ownership != hir::OwnershipMode::Own {
            return Err(backend_error(
                "Bytes call argument lacks validated ownership classification",
            ));
        }
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned Bytes call has no canonical cleanup plan"))?;
        let index = u32::try_from(index)
            .map_err(|_| backend_error("native call has too many parameters"))?;
        let storage = plan.call_argument_storage(&call.id, index)?;
        // Producers can already have transferred into this exact epoch. The
        // plan authenticates both that case and the one remaining transfer;
        // replaying every transition at the argument would initialize twice.
        let transitions = plan.transfer_field_at(
            &argument.id,
            &value.code,
            &crate::cleanup_plan::CleanupPlace {
                storage,
                projections: Vec::new(),
            },
        )?;
        for line in transitions.lines() {
            self.line(line);
        }
        value.code = plan.call_argument(&call.id, index)?.0.to_owned();
        Ok(value)
    }
}
