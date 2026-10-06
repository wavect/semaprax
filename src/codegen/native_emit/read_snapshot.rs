//! Left-to-right reads of `let mut` bindings (issue #561).
//!
//! A place expression lowers to the C name of its binding. Consumers such as
//! binary operators and call argument lists evaluate later operands before
//! they spell the earlier operand into the operation, so an alias to a
//! mutable binding would observe a store made by a later operand's nested
//! block. A Copy read of a `let mut` binding is therefore copied into a fresh
//! temporary at its own evaluation point. Immutable bindings cannot change
//! after initialization and stay plain aliases, so their emitted C is
//! unchanged; owned and borrowed carriers are never copied here.

use super::{record_declaration_id, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedType, ValueId};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn snapshot_mutable_copy_read(
        &mut self,
        root: &ValueId,
        value: CValue,
    ) -> Result<CValue, Diagnostic> {
        if !self.mutable_bindings.contains(root) || !self.is_copy_value(&value.ty)? {
            return Ok(value);
        }
        let snapshot = self.temporary(&value.ty)?;
        self.line(&format!("{snapshot} = {};", value.code));
        Ok(CValue {
            code: snapshot,
            ty: value.ty,
        })
    }

    /// A checked Copy scalar, or a record or class whose fields are all such
    /// values (Field Mutation v1 stores into a direct scalar field of one).
    fn is_copy_value(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        if hir::is_scalar_resolved_type(ty) {
            return Ok(true);
        }
        if record_declaration_id(self.program, ty)?.is_none() {
            return Ok(false);
        }
        for field in self.record_layout(ty)?.fields {
            if !self.is_copy_value(&field.ty)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
