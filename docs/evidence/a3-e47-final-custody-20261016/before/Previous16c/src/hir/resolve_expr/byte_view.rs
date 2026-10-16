//! Lower only rooted view operands; the source and HIR boundaries authenticate them.
use super::*;

impl Resolver<'_> {
    pub(in crate::hir) fn byte_range_source_shape(&self, source: &ResolvedExpr) -> bool {
        match &source.kind {
            ResolvedExprKind::Place(place) => place.projections.is_empty(),
            ResolvedExprKind::BorrowPlace { operation, place } => {
                operation.as_str() == crate::byte_ops::BYTES_AS_SLICE_ID
                    && place.projections.is_empty()
                    && source.ty == ResolvedType::SliceU8
                    && source.ownership == OwnershipMode::Borrow
            }
            _ => false,
        }
    }

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
            _ => {
                let requirement = match operation {
                    crate::byte_ops::ByteOp::StringAsStr => {
                        "a named String owner or an authenticated named-record path to a String field"
                    }
                    crate::byte_ops::ByteOp::StrAsBytes => {
                        "a named `str` view or a view derived from a named String field path"
                    }
                    _ => "an exact named storage root",
                };
                Err(self.error(
                    "SPX-T266",
                    format!(
                        "borrowed view `{}` requires {requirement}",
                        operation.name()
                    ),
                    argument.span,
                ))
            }
        }
    }
}
