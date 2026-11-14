//! Native lowering for the internal fixed-width owned byte-buffer fill.

use crate::diagnostic::Diagnostic;
use crate::hir::ExpressionId;

use super::{backend_error, CEmitter, COutput, CValue};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_owned_buffer_bounded_allocation(
        &mut self,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        self.line(&format!(
            "spx_status = spx_bytes_bounded_check_v1(spx_ctx, {}, {});",
            arguments[0].code, arguments[1].code
        ));
        self.emit_operation_failure_check()?;
        // Reserved-output profiles acquire their existing allocator only after
        // the portable check. No argument owner or result has committed yet.
        let (allocator, context) = self
            .output_profile
            .byte_allocator(crate::byte_ops::ByteOp::Zeroed);
        if self.output_profile == super::super::NativeOutputProfile::CheckedAllocation {
            let site = self
                .active_expression
                .clone()
                .ok_or_else(|| backend_error("checked bounded Bytes lacks its exact source"))?;
            self.checked_string_constructor(
                &site,
                crate::cleanup_plan::AllocationOperation::BytesZeroed,
                &format!(
                    "spx_checked_native_bytes_zeroed_v1(spx_ctx, {}, &{temporary})",
                    arguments[0].code
                ),
                false,
            )?;
        } else {
            self.line(&format!(
                "{temporary} = {allocator}({context}{});",
                arguments[0].code
            ));
        }
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set(
        &mut self,
        expression: &ExpressionId,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        // The bound is checked before the canonical owner transfer commits, so
        // a failed store leaves the buffer in its live call-argument slot and
        // the epilogue destroys it exactly once.
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned byte store has no canonical cleanup plan"))?;
        let (buffer, buffer_live, _) = plan.call_argument(expression, 0)?;
        if arguments[0].code != buffer {
            return Err(backend_error(
                "owned byte store argument was not staged in its canonical epoch",
            ));
        }
        let index = &arguments[1].code;
        let byte = &arguments[2].code;
        if self.output_profile == super::super::NativeOutputProfile::SelectedPartition {
            self.line(&format!(
                "spx_status = spx_shb_status(spx_ctx, spx_shb_set_check(spx_shb_held(spx_ctx), &{buffer}, {index}));"
            ));
            self.emit_operation_failure_check()?;
            self.line(&format!(
                "spx_shb_set(spx_shb_held(spx_ctx), &{buffer}, {index}, {byte}, &{temporary});"
            ));
        } else {
            self.line(&format!(
                "spx_status = spx_bytes_set_check_v1(spx_ctx, {buffer}, {index});"
            ));
            self.emit_operation_failure_check()?;
            self.line(&format!(
                "{temporary} = spx_bytes_set(spx_bytes_move(&{buffer}), {index}, {byte});"
            ));
        }
        // The store committed: the staged argument epoch no longer owns the
        // buffer, exactly as an ordinary owned call argument stops owning it
        // at its commit boundary. A loop-carried fill reuses this slot.
        self.line(&format!("{buffer_live} = false;"));
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set1_or5(
        &mut self,
        expression: &ExpressionId,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned byte store has no canonical cleanup plan"))?;
        let (buffer, buffer_live, _) = plan.call_argument(expression, 0)?;
        if arguments[0].code != buffer {
            return Err(backend_error(
                "owned byte store argument was not staged in its canonical epoch",
            ));
        }
        self.line(&format!(
            "spx_status = spx_bytes_set1_or5_check_v1(spx_ctx, {}, {}, {});",
            buffer, arguments[4].code, arguments[1].code
        ));
        self.emit_operation_failure_check()?;
        self.line(&format!(
            "{temporary} = spx_bytes_set1_or5(spx_bytes_move(&{buffer}), {}, {}, {}, {});",
            arguments[1].code, arguments[2].code, arguments[3].code, arguments[4].code
        ));
        self.line(&format!("{buffer_live} = false;"));
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set1_or6_or48(
        &mut self,
        expression: &ExpressionId,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned byte store has no canonical cleanup plan"))?;
        let (buffer, buffer_live, _) = plan.call_argument(expression, 0)?;
        if arguments[0].code != buffer {
            return Err(backend_error(
                "owned byte store argument was not staged in its canonical epoch",
            ));
        }
        self.line(&format!(
            "spx_status = spx_bytes_set1_or6_or48_check_v1(spx_ctx, {}, {}, {});",
            buffer, arguments[4].code, arguments[1].code
        ));
        self.emit_operation_failure_check()?;
        self.line(&format!(
            "{temporary} = spx_bytes_set1_or6_or48(spx_bytes_move(&{buffer}), {}, {}, {}, {});",
            arguments[1].code, arguments[2].code, arguments[3].code, arguments[4].code
        ));
        self.line(&format!("{buffer_live} = false;"));
        Ok(())
    }

    pub(super) fn emit_owned_buffer_set5(
        &mut self,
        expression: &ExpressionId,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned byte store has no canonical cleanup plan"))?;
        let (buffer, buffer_live, _) = plan.call_argument(expression, 0)?;
        if arguments[0].code != buffer {
            return Err(backend_error(
                "owned byte store argument was not staged in its canonical epoch",
            ));
        }
        self.line(&format!(
            "spx_status = spx_bytes_set5_check_v1(spx_ctx, {}, {});",
            buffer, arguments[1].code
        ));
        self.emit_operation_failure_check()?;
        self.line(&format!(
            "{temporary} = spx_bytes_set5(spx_bytes_move(&{buffer}), {}, {}, {}, {}, {}, {});",
            arguments[1].code,
            arguments[2].code,
            arguments[3].code,
            arguments[4].code,
            arguments[5].code,
            arguments[6].code
        ));
        self.line(&format!("{buffer_live} = false;"));
        Ok(())
    }
}
