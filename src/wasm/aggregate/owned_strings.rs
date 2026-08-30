//! V10 inline String ownership, separate from resource CleanupPlan semantics.
//!
//! String place reads clone (as in native C); only compiler temporaries move.
//! Each nonzero cell owns one tagged arena token, including empty strings.
//! Call-out memory is transport, never an independently swept owner cell.

use std::collections::{BTreeMap, BTreeSet};

use super::{error, write_u32, Diagnostic, ExpressionId, BYTE_DROP_IMPORT};

#[derive(Default)]
pub(super) struct Cells {
    pub(super) owners: BTreeSet<u32>,
    scopes: BTreeMap<ExpressionId, std::ops::Range<u32>>,
}

impl Cells {
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
fn emit_drop(output: &mut Vec<u8>, local: u32) {
    output.push(0x20);
    write_u32(output, local);
    output.extend([0x50, 0x45, 0x04, 0x40, 0x20]); // i64.eqz; i32.eqz; if; get
    write_u32(output, local);
    emit_clear(output, local);
    output.push(0x10);
    write_u32(output, BYTE_DROP_IMPORT);
    output.push(0x0b);
}
