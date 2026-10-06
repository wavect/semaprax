//! Owned String carrier operations shared by canonical CleanupPlan lowering.

use super::{write_u32, BYTE_DROP_IMPORT};

pub(super) fn emit_clear(output: &mut Vec<u8>, local: u32) {
    output.extend([0x42, 0x00, 0x21]);
    write_u32(output, local);
}

pub(super) fn emit_empty_guard(output: &mut Vec<u8>, local: u32) {
    output.push(0x20);
    write_u32(output, local);
    output.extend([0x50, 0x45, 0x04, 0x40, 0x00, 0x0b]);
}

/// Clear physical ownership before calling the finalizer. A host exception is
/// still a poisoned-instance fail-stop, never a recoverable cleanup promise.
pub(super) fn emit_drop(output: &mut Vec<u8>, local: u32) {
    output.push(0x20);
    write_u32(output, local);
    output.extend([0x50, 0x45, 0x04, 0x40, 0x20]); // i64.eqz; i32.eqz; if; get
    write_u32(output, local);
    emit_clear(output, local);
    output.push(0x10);
    write_u32(output, BYTE_DROP_IMPORT);
    output.push(0x0b);
}

/// Emit the interned owned UTF-8 literal table as the module's data section.
///
/// The aggregate profile places it at [`super::OWNED_UTF8_LITERAL_BASE`], three
/// pages in, so the memory section must already reserve four pages.
pub(super) fn emit_literal_data(
    module: &mut Vec<u8>,
    owned_utf8: bool,
    literals: &super::OwnedUtf8Literals,
) -> Result<(), crate::diagnostic::Diagnostic> {
    if !owned_utf8 {
        return Ok(());
    }
    let mut data = Vec::new();
    super::write_u32(&mut data, 1);
    data.push(0x00);
    data.push(0x41);
    super::write_i64(&mut data, i64::from(literals.base()));
    data.push(0x0b);
    super::write_u32(
        &mut data,
        u32::try_from(literals.bytes.len())
            .map_err(|_| super::error("owned UTF-8 literal table overflows u32"))?,
    );
    data.extend_from_slice(&literals.bytes);
    super::section(module, 11, data);
    Ok(())
}

impl super::Emitter<'_> {
    /// Owned String Loops v1: the first operand of a same-owner append moves
    /// the binding's carrier into the call instead of cloning it, exactly as
    /// the replayed CleanupPlan transfers it.
    pub(super) fn moves_string_owner(&self, expression: &crate::hir::ResolvedExpr) -> bool {
        crate::string_ops::same_owner_concat_operands(self.function).contains(&expression.id)
    }
}
