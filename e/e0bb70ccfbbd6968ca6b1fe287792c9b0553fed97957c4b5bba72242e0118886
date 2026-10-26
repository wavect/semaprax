//! Complete immediate previous persistent contract receiver.
use super::super::*;
#[rustfmt::skip]
impl HirValidator<'_> {
    pub(super) fn validate_contracts_before_frame_reuse(
        &mut self,
        execution: &super::FunctionExecutionId,
        contracts: &[ResolvedExpr],
        outer: &super::scope_checkpoint::Scope,
        phase: &str,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        if contracts.is_empty() {
            return Ok(());
        }
        let baseline = super::scope_checkpoint::Scope::fresh_baseline_persistent(outer)?;
        for (index, contract) in contracts.iter().enumerate() {
            let mut scope = baseline.clone();
            self.validate_expr_before_frame_reuse(
                execution,
                contract,
                &mut scope,
                super::path_label::Label::child(phase, ".{}", [index, 0])?,
                false,
                None,
            )?;
            self.require_type(
                &contract.ty,
                &ResolvedType::Bool,
                if phase == "requires" {
                    "precondition"
                } else {
                    "postcondition"
                },
            )?;
        }
        Ok(())
    }

}
