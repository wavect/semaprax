//! Persistent function scope; diagnostics retain their original order.
use super::*;
impl HirValidator<'_> {
    pub(in crate::hir) fn validate_function(
        &mut self,
        function: &ResolvedFunction,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        frame_reuse::fresh_function_transport()?;
        frame_reuse::argument()?;
        self.validate_function_with_frames(function, execution, None)
    }

    #[cfg(test)]
    fn validate_expr_persistent(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut scope_checkpoint::Scope,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<effect_keys::Allowed<'_>>,
    ) -> Result<(), Diagnostic> {
        // The iterative driver retains its Publication guard and restoration
        // frames. Its checkpoint target returns the published root on Drop.
        self.validate_expr_iterative_effects(
            function,
            expression,
            scope,
            path.into(),
            allow_moves,
            allowed_effects,
        )
    }
}

#[cfg(test)]
mod collection_tests;
#[cfg(test)]
mod old;
#[cfg(test)]
mod tests;

pub(super) mod lifecycle;

#[cfg(test)]
mod backing_saved;

#[cfg(test)]
mod lifecycle_receiver_saved;

#[cfg(test)]
mod phase_tests;

#[cfg(test)]
mod empty_ensures;
