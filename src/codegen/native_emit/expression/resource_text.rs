//! Provenance-gated byte views for the Project-v28 native resource profile.

use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedType};

use super::super::{CEmitter, COutput, CValue};

impl<O: COutput> CEmitter<'_, O> {
    // Only a direct immutable String-as-Str local carries this positive bit.
    // Its producer emitted the full UTF-8 check before the local was published.
    // Aliases, mutable values, projected inputs and foreign parameters keep
    // their full check. Scope restoration and assignments revoke these facts.
    pub(super) fn str_byte_check(&mut self, expression: &ResolvedExpr, code: &str) {
        let checked = expression.ty == ResolvedType::Str
            && matches!(&expression.kind, ResolvedExprKind::Place(place)
                if place.projections.is_empty()
                    && self.resource_text_views.get(&place.root) == Some(&true)
                    && !self.mutable_bindings.contains(&place.root));
        if !checked {
            self.line(&format!("spx_str_require_valid({code});"));
        }
    }

    pub(super) fn emit_str_as_bytes_view(
        &mut self,
        destination: &str,
        source: &CValue,
        expression: &ResolvedExpr,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        let source_code = if source.ty == ResolvedType::String {
            let text = self.temporary(&ResolvedType::Str)?;
            self.line(&format!("{text} = spx_string_as_str({});", source.code));
            text
        } else {
            if source.ty != ResolvedType::Str {
                return Err(super::super::backend_error(
                    "borrowed UTF-8 byte view source has the wrong type",
                ));
            }
            source.code.clone()
        };
        self.line(&format!("spx_str_require_valid({source_code});"));
        self.line(&format!(
            "{destination} = (spx_slice_u8_v1) {{ .ptr = ({}).len == UINT64_C(0) ? NULL : (const uint8_t *)({}).data, .len = ({}).len }};",
            source_code, source_code, source_code
        ));
        let validator = if self.owned_str_views {
            self.resource_text_read_validator(expression)
        } else {
            self.resource_text_validator(expression)
        };
        self.line(&format!("{validator}({destination});"));
        Ok(())
    }

    /// Select widened helpers only for retained HIR provenance produced from
    /// an immutable borrowed `str`. Carrier shape cannot upgrade Slice/Bytes.
    pub(super) fn is_resource_text_slice(&self, expression: &ResolvedExpr) -> bool {
        if !self.output_profile.is_source_resource() {
            return false;
        }
        let provenance = match &expression.kind {
            ResolvedExprKind::Place(place)
                if place.projections.is_empty()
                    && matches!(expression.ty, ResolvedType::Str | ResolvedType::SliceU8)
                    && self.resource_text_views.contains_key(&place.root) =>
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
                    && self.resource_text_views.contains_key(&place.root)
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
            provenance.projections.is_empty()
                && matches!(
                    (provenance.root_kind, &provenance.projected_type),
                    (hir::ByteSliceRootKind::BorrowedStr, ResolvedType::Str)
                        | (hir::ByteSliceRootKind::OwnedString, ResolvedType::String)
                )
        })
    }

    pub(super) fn produces_resource_text_view(&self, expression: &ResolvedExpr) -> bool {
        if !self.output_profile.is_source_resource() {
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

    /// Reads retain the authenticated internal root's existing capacity. The
    /// function-entry guard separately admits and charges foreign roots once;
    /// using its smaller bound here reclassified forwarded owned Bytes views.
    pub(super) fn resource_text_read_validator(&self, expression: &ResolvedExpr) -> &'static str {
        if self.is_resource_text_slice(expression) {
            "spx_source_resource_slice_require_valid_v1"
        } else {
            "spx_slice_u8_require_owned_view_valid"
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

    pub(super) fn emit_stdout_write(
        &mut self,
        expression: &ResolvedExpr,
        value: &str,
        result: &str,
    ) -> Result<(), Diagnostic> {
        if self.output_profile.is_source_resource() {
            let helper = if self.is_resource_text_slice(expression) {
                "spx_host_command_stdout_write_resource_str_checked_v1"
            } else {
                "spx_host_command_stdout_write_checked_v1"
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {value}, &{result});"
            ));
            self.emit_operation_failure_check()?;
        } else {
            let helper = if self.output_profile.is_language_command() {
                "spx_host_command_stdout_write_v1"
            } else {
                "spx_host_stdout_write_v1"
            };
            self.line(&format!("{result} = {helper}(spx_ctx, {value});"));
        }
        Ok(())
    }

    pub(super) fn emit_stderr_write(
        &mut self,
        expression: &ResolvedExpr,
        value: &str,
        result: &str,
    ) -> Result<(), Diagnostic> {
        if self.output_profile.is_source_resource() {
            let helper = if self.is_resource_text_slice(expression) {
                "spx_host_command_stderr_write_resource_str_checked_v1"
            } else {
                "spx_host_command_stderr_write_checked_v1"
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {value}, &{result});"
            ));
            self.emit_operation_failure_check()?;
        } else {
            self.line(&format!(
                "{result} = spx_host_command_stderr_write_v1(spx_ctx, {value});"
            ));
        }
        Ok(())
    }
}
