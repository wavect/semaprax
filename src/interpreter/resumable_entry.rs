//! Narrow checked-value entry used only by the aggregate resumable channel.

use super::{Evaluator, Flow, Value};
use crate::hir::{ResolvedFunction, ValueId};

impl Evaluator<'_> {
    /// Enter a checked resumable function with values already reconstructed
    /// by its narrow channel boundary. This is not a general public-value
    /// API: the caller has already checked the exact parameter identities and
    /// flat Copy aggregate shape before this frame is created.
    pub(crate) fn evaluate_entry_values(
        &mut self,
        function: &ResolvedFunction,
        arguments: Vec<(ValueId, Value)>,
    ) -> Result<Value, Flow> {
        if arguments.len() != function.params.len()
            || arguments
                .iter()
                .zip(&function.params)
                .any(|((id, value), param)| {
                    id != &param.id || !self.value_has_type(value, &param.ty)
                })
        {
            return Err(Flow::Guard("argument/parameter binding mismatch"));
        }
        self.call_frame(function, arguments, 0)
    }
}
