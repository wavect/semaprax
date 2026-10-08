//! Provenance-gated byte views for the Project-v28 native resource profile.

use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedType};

use super::super::{CEmitter, COutput, NativeOutputProfile};

impl<O: COutput> CEmitter<'_, O> {
    /// Select widened helpers only for retained HIR provenance produced from
    /// an immutable borrowed `str`. Carrier shape cannot upgrade Slice/Bytes.
    pub(super) fn is_resource_text_slice(&self, expression: &ResolvedExpr) -> bool {
        if self.output_profile != NativeOutputProfile::SourceResourceCommand {
            return false;
        }
        let provenance = match &expression.kind {
            ResolvedExprKind::Place(place)
                if place.projections.is_empty()
                    && matches!(expression.ty, ResolvedType::Str | ResolvedType::SliceU8)
                    && self.resource_text_views.contains(&place.root) =>
            {
                return true;
            }
            ResolvedExprKind::Place(place) => {
                self.program.declarations.byte_slice_provenance(&place.root)
            }
            ResolvedExprKind::ByteRange { source, .. } => {
                return self.is_resource_text_slice(source);
            }
            ResolvedExprKind::BorrowPlace { operation, place } => {
                if operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                    && place.projections.is_empty()
                    && self.resource_text_views.contains(&place.root)
                {
                    return true;
                }
                self.program
                    .declarations
                    .byte_slice_provenances()
                    .find_map(|(_, provenance)| {
                        (provenance.producer.as_ref() == Some(&expression.id)).then_some(provenance)
                    })
            }
            _ => None,
        };
        provenance.is_some_and(|provenance| {
            provenance.root_kind == hir::ByteSliceRootKind::BorrowedStr
                && provenance.projected_type == ResolvedType::Str
                && provenance.projections.is_empty()
        })
    }

    pub(super) fn produces_resource_text_view(&self, expression: &ResolvedExpr) -> bool {
        if self.output_profile != NativeOutputProfile::SourceResourceCommand {
            return false;
        }
        match &expression.kind {
            ResolvedExprKind::BorrowPlace { operation, .. }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID =>
            {
                true
            }
            _ => self.is_resource_text_slice(expression),
        }
    }

    pub(super) fn resource_text_len_helper(&self, expression: &ResolvedExpr) -> &'static str {
        if self.is_resource_text_slice(expression) {
            "spx_source_resource_byte_len_v1"
        } else {
            "spx_byte_len"
        }
    }

    pub(super) fn resource_text_validator(&self, expression: &ResolvedExpr) -> &'static str {
        if self.is_resource_text_slice(expression) {
            "spx_source_resource_slice_require_valid_v1"
        } else {
            "spx_slice_u8_require_valid"
        }
    }

    pub(super) fn resource_text_range_helper(&self, expression: &ResolvedExpr) -> &'static str {
        if self.is_resource_text_slice(expression) {
            "spx_source_resource_byte_range_v1"
        } else {
            "spx_byte_range_v1"
        }
    }

    pub(super) fn resource_text_stdout_helper(&self, expression: &ResolvedExpr) -> &'static str {
        if self.is_resource_text_slice(expression) {
            "spx_host_command_stdout_write_resource_str_v1"
        } else if self.output_profile.is_language_command() {
            "spx_host_command_stdout_write_v1"
        } else {
            "spx_host_stdout_write_v1"
        }
    }
}
