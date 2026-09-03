//! V10 inline String ownership, separate from resource CleanupPlan semantics.
//!
//! String place reads clone (as in native C); only compiler temporaries move.
//! Each nonzero cell owns one tagged arena token, including empty strings.
//! Call-out memory is transport, never an independently swept owner cell.

use std::collections::{BTreeMap, BTreeSet};

use super::{error, write_u32, Diagnostic, ExpressionId, BYTE_DROP_IMPORT};

#[cfg(test)]
mod work_bounds;

#[derive(Default)]
pub(super) struct Cells {
    pub(super) owners: BTreeSet<u32>,
    scopes: BTreeMap<ExpressionId, std::ops::Range<u32>>,
}

impl Cells {
    pub(super) fn bounded_emission_work(&self) -> Option<usize> {
        let mut work = self
            .owners
            .len()
            .checked_mul(2)
            .filter(|value| *value <= 262_144)?;
        for range in self.scopes.values() {
            for _ in self.owners.range(range.clone()) {
                work = work.checked_add(1).filter(|value| *value <= 262_144)?;
            }
        }
        Some(work)
    }
    pub(super) fn insert(&mut self, local: u32) -> Result<(), Diagnostic> {
        if !self.owners.insert(local) {
            return Err(error("owned String local inventory repeats a cell"));
        }
        Ok(())
    }

    pub(super) fn scope(
        &mut self,
        id: &ExpressionId,
        first: u32,
        end: u32,
    ) -> Result<(), Diagnostic> {
        if first > end || self.scopes.insert(id.clone(), first..end).is_some() {
            return Err(error("owned String expression inventory is inconsistent"));
        }
        Ok(())
    }

    pub(super) fn emit_scope(
        &self,
        output: &mut Vec<u8>,
        id: &ExpressionId,
        escape: Option<u32>,
    ) -> Result<(), Diagnostic> {
        let range = self
            .scopes
            .get(id)
            .ok_or_else(|| error("owned String expression scope is absent"))?;
        for local in self.owners.range(range.clone()).copied() {
            if Some(local) != escape {
                emit_drop(output, local);
            }
        }
        Ok(())
    }

    pub(super) fn emit_all(&self, output: &mut Vec<u8>, escape: Option<u32>) {
        for local in self.owners.iter().copied() {
            if Some(local) != escape {
                emit_drop(output, local);
            }
        }
    }
}

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
    super::write_i64(&mut data, i64::from(super::OWNED_UTF8_LITERAL_BASE));
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
