//! Exact enclosing return types for materialized template proof executions.
use super::*;

impl HirValidator<'_> {
    pub(super) fn execution_return_type(&self, id: &FunctionExecutionId) -> Option<&ResolvedType> {
        // A proof substitution need not have a call site or cached instance.
        // Its return type comes from the independently materialized template,
        // and is available only under that exact derived execution identity.
        if let (FunctionExecutionId::Generic(instance), Some((proof, ty))) =
            (id, &self.proof_return)
        {
            if self.proof_generic_calls && instance == proof {
                return Some(ty);
            }
        }
        match id {
            FunctionExecutionId::Monomorphic(declaration) => self
                .functions
                .get(declaration)
                .map(|function| &function.return_type),
            FunctionExecutionId::Generic(instance) => self
                .program
                .function_instances
                .iter()
                .find(|candidate| candidate.id == *instance)
                .map(|candidate| &candidate.function.return_type),
        }
    }
}
