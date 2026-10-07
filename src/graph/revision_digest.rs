//! Revision v2 hashing without retaining the canonical source projection.
//!
//! Bounded callers retain the historical formatter reservations, overflow
//! behavior and prelude selection from the materialized (possibly capped)
//! source. Ordinary callers need neither that legacy temporary-byte census nor
//! a source-sized allocation/reparse just to hash a retained AST.

use crate::ast::Program;
use sha2::{Digest, Sha256};

pub(super) fn revision(program: &Program) -> String {
    if crate::bounded_output::active_limit().is_some() {
        let source = crate::format::canonical(program);
        return super::revision_from_canonical_source(&source);
    }

    // Revision v2 length-delimits source before its bytes. Both passes use
    // the authoritative formatter, including ordinary Kernel-0 candidate
    // comparison/shadow hooks and statement-if provenance authentication.
    let mut length = Length(0);
    crate::format::write_canonical(program, &mut length);
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.graph-revision.v2\0");
    hasher.update((length.0 as u64).to_le_bytes());
    let mut output = HashWriter::new(&mut hasher);
    crate::format::write_canonical(program, &mut output);
    output.flush();

    // The existing canonical-program route uses the same selector. This
    // avoids reparsing the projection solely to recover prelude usage that
    // is already present in the AST. No digest or spelling is cached across
    // invocations: an edited AST is traversed afresh on every call.
    let (schema, contract, _) = crate::prelude::selected_for_program(program);
    super::prelude_binding::finish_revision(hasher, schema, &contract)
}

struct Length(usize);

impl std::fmt::Write for Length {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0 = self
            .0
            .checked_add(value.len())
            .expect("canonical source length overflow");
        Ok(())
    }
}

// Formatting emits many short fragments, including one escaped string scalar
// at a time. Batch SHA-256 updates in fixed storage rather than making every
// fragment a separate hash update or retaining the whole canonical String.
struct HashWriter<'a> {
    hasher: &'a mut Sha256,
    buffer: [u8; 4096],
    used: usize,
}

impl<'a> HashWriter<'a> {
    fn new(hasher: &'a mut Sha256) -> Self {
        Self {
            hasher,
            buffer: [0; 4096],
            used: 0,
        }
    }

    fn flush(&mut self) {
        self.hasher.update(&self.buffer[..self.used]);
        self.used = 0;
    }
}

impl std::fmt::Write for HashWriter<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let mut bytes = value.as_bytes();
        while !bytes.is_empty() {
            if self.used == 0 && bytes.len() >= self.buffer.len() {
                self.hasher.update(bytes);
                break;
            }
            let count = bytes.len().min(self.buffer.len() - self.used);
            self.buffer[self.used..self.used + count].copy_from_slice(&bytes[..count]);
            self.used += count;
            bytes = &bytes[count..];
            if self.used == self.buffer.len() {
                self.flush();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "revision_digest/tests.rs"]
mod tests;
