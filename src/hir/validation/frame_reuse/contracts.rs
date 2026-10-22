//! Fresh contract-local scope roots, sharing only drained continuation backing.
use super::super::*;
impl<'a> HirValidator<'a> {
    pub(in crate::hir::validation) fn validate_contracts_with_frames<'e>(
        &mut self,
        execution: &FunctionExecutionId,
        contracts: &'e [ResolvedExpr],
        outer: &scope_checkpoint::Scope,
        phase: &'e str,
        mut scratch: Option<&mut frame_reuse::Scratch<'e>>,
    ) -> Result<(), crate::diagnostic::Diagnostic> 
    where
        'a: 'e,
    {
        if contracts.is_empty() {
            return Ok(());
        }
        let baseline = scope_checkpoint::Scope::fresh_baseline_persistent(outer)?;
        for (index, contract) in contracts.iter().enumerate() {
            let mut scope = baseline.clone();
            frame_reuse::argument()?;
            self.validate_expr_with_frames(
                execution,
                contract,
                &mut scope,
                path_label::Label::child(phase, ".{}", [index, 0])?,
                false,
                None,
                scratch.as_deref_mut(),
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
