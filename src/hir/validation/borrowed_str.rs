//! Root authentication for immutable borrowed-string local bindings.

use super::*;

impl HirValidator<'_> {
    pub(super) fn borrowed_str_let_origin(
        &self,
        value: &ResolvedExpr,
    ) -> Result<(Place, bool), Diagnostic> {
        match &value.kind {
            ResolvedExprKind::Place(place) if place.projections.is_empty() => self
                .borrowed_str_aliases
                .get(&place.root)
                .cloned()
                .map(|origin| (origin, false))
                .ok_or_else(|| {
                    hir_error("borrowed-str local alias lacks authenticated root provenance")
                }),
            ResolvedExprKind::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID
                    && place.projections.is_empty() =>
            {
                Ok((place.clone(), true))
            }
            ResolvedExprKind::HostCommandCall(call)
                if call.operation == ResolvedHostCommandOperation::ArgUtf8 =>
            {
                Ok((
                    Place {
                        root: ValueId::intrinsic_parameter(
                            crate::command_io_ops::ARG_UTF8_ID,
                            usize::MAX,
                        ),
                        projections: Vec::new(),
                    },
                    false,
                ))
            }
            _ => Err(hir_error(
                "borrowed-str local must be an exact alias or authenticated owning String view",
            )),
        }
    }
}
