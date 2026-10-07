//! Typed Err reconstruction for an owned Result whose Ok type changes.

use super::*;
use crate::variant_layout::{VariantCaseLayout, VariantFieldLayout};

pub(super) fn is_admitted_owned_result_try(
    operand: &ResolvedExpr,
    residual_type: &ResolvedType,
    result: &DeclarationId,
) -> bool {
    if operand.ownership != crate::hir::OwnershipMode::Own
        || result.as_str() != crate::prelude::RESULT_ID
    {
        return false;
    }
    matches!((&operand.ty, residual_type),
        (ResolvedType::Nominal { declaration: source, arguments: source_args },
         ResolvedType::Nominal { declaration: target, arguments: target_args })
            if source == result && target == result
                && source_args.get(1) == target_args.get(1)
                && crate::hir::admitted_owned_byte_prelude_instance(source, source_args)
                && crate::hir::admitted_owned_byte_prelude_instance(target, target_args))
}

impl Emitter<'_> {
    pub(super) fn reconstruct_owned_result_error(
        &mut self,
        source: (Pointer, &VariantLayout, &VariantFieldLayout),
        target: (
            Pointer,
            &VariantLayout,
            &VariantCaseLayout,
            &VariantFieldLayout,
        ),
    ) -> Result<(), Diagnostic> {
        let (source_pointer, source_layout, source_field) = source;
        let (target_pointer, target_layout, target_case, target_field) = target;
        require_type(
            &source_field.ty,
            &target_field.ty,
            "owned Result Err payload",
        )?;
        self.emit_pointer(target_pointer);
        self.output.extend([0x41, 0x00, 0x41]);
        write_i64(self.output, i64::from(target_layout.size));
        self.output.extend([0xfc, 0x0b, 0x00]);
        let source = Value::ScalarMemory {
            pointer: Pointer {
                local: source_pointer.local,
                offset: source_pointer
                    .offset
                    .checked_add(source_layout.payload_offset)
                    .and_then(|offset| offset.checked_add(source_field.offset))
                    .ok_or_else(|| error("owned Result Err source pointer overflows u32"))?,
            },
            ty: source_field.ty.clone(),
        };
        let destination = Value::ScalarMemory {
            pointer: Pointer {
                local: target_pointer.local,
                offset: target_pointer
                    .offset
                    .checked_add(target_layout.payload_offset)
                    .and_then(|offset| offset.checked_add(target_field.offset))
                    .ok_or_else(|| error("owned Result Err destination pointer overflows u32"))?,
            },
            ty: target_field.ty.clone(),
        };
        self.copy_value(&destination, &source, "owned Result Err reconstruction")?;
        self.emit_pointer(target_pointer);
        self.output.push(0x41);
        write_i64(self.output, i64::from(target_case.tag));
        self.output.extend([0x36, 0x02, 0x00]);
        Ok(())
    }
}
