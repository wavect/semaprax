//! Lower only rooted view operands; the source and HIR boundaries authenticate them.
use super::*;

impl Resolver<'_> {
    pub(in crate::hir) fn resolved_view_place<'a>(
        &self,
        operation: crate::byte_ops::ByteOp,
        argument: &'a ResolvedExpr,
    ) -> Result<&'a Place, Diagnostic> {
        match &argument.kind {
            ResolvedExprKind::Place(place) => Ok(place),
            ResolvedExprKind::BorrowPlace {
                operation: inner,
                place,
            } if operation == crate::byte_ops::ByteOp::StrAsBytes
                && inner.as_str() == crate::byte_ops::STRING_AS_STR_ID =>
            {
                Ok(place)
            }
            _ => Err(self.error(
                "SPX-T266",
                format!(
                    "borrowed view `{}` requires an exact named storage root",
                    operation.name()
                ),
                argument.span,
            )),
        }
    }
}
