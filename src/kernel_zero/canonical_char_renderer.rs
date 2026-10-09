//! Exact-source Kernel-0 component boundary for canonical `char` rendering.
//!
//! The embedded Semaprax module is parsed, resolved, bounded, translated, and
//! independently replayed before its Kernel-0 program can render a byte.  The
//! interface remains a scalar byte lane because Kernel-0 has no owned buffer:
//! one call obtains the length and subsequent calls obtain each byte.
//!
//! This component is not authoritative. The production formatter passes its
//! replayed candidate through the bounded authority adapter, which returns it
//! only after byte equality with Rust and otherwise preserves Rust bytes.
//! Tests additionally retain the shadow hook. A later rung still needs the
//! owned-buffer and hosted evidence required before authority can move.

use std::sync::OnceLock;

use crate::hir::DeclarationId;

use super::eval::eval_program;
use super::reify::{BoundTranslation, Refusal};
use super::term::KernelProgram;
use super::value::Value;

pub(crate) const SOURCE: &str = include_str!("canonical_char_renderer.spx");
const MAX_RENDERED_BYTES: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RendererRefusal {
    InvalidScalar,
    Profile,
    MissingEntry,
    Evaluation,
    InvalidLength,
    InvalidByte,
}

impl From<Refusal> for RendererRefusal {
    fn from(_: Refusal) -> Self {
        Self::Profile
    }
}

struct Renderer {
    /// This is deliberately retained instead of only retaining its first
    /// translated program.  A `source` byte-different from
    /// `validated_source` below (only the drift-detection test below passes
    /// one) still replays the binding against those bytes before evaluation,
    /// so the cache cannot turn an old successful translation into authority
    /// for changed component text.
    binding: BoundTranslation,
    /// The exact source bytes `same_source_program` below was independently
    /// replayed against at construction time.
    validated_source: String,
    /// `binding.replay(&validated_source, ..)`'s own already-proven result,
    /// reused whenever a later call's `source` is byte-identical to
    /// `validated_source` (`bytes` below): re-parsing, re-resolving, and
    /// re-validating this fixed, `include_str!`-embedded component source
    /// again for that exact same input is deterministic per
    /// `BoundTranslation::derive`'s own contract (same source bytes, same
    /// entry, in, same program out -- see `AGENTS.md`'s "Source formatting
    /// ... deterministic" invariant), so repeating it once per rendered
    /// `char` bought nothing beyond the first call and dominated this
    /// renderer's cost under any caller that renders many chars from one
    /// process (see issue #294).
    same_source_program: KernelProgram,
}

impl Renderer {
    fn derive() -> Result<Self, RendererRefusal> {
        Self::derive_from_source(SOURCE)
    }

    fn derive_from_source(source: &str) -> Result<Self, RendererRefusal> {
        let entry = DeclarationId::new("format.render-byte");
        let binding = BoundTranslation::derive(source, &entry)?;
        let same_source_program = binding.replay(source, &entry)?.clone();
        Ok(Self {
            binding,
            validated_source: source.to_owned(),
            same_source_program,
        })
    }

    fn int(
        &self,
        program: &KernelProgram,
        entry: &str,
        arguments: &[i64],
    ) -> Result<i64, RendererRefusal> {
        let entry = program
            .function(&DeclarationId::new(entry))
            .ok_or(RendererRefusal::MissingEntry)?;
        if arguments.len() > 2 {
            return Err(RendererRefusal::Evaluation);
        }
        let mut slots = [Value::Int(0); 2];
        for (slot, argument) in slots.iter_mut().zip(arguments) {
            *slot = Value::Int(*argument);
        }
        match eval_program(program, entry, &slots[..arguments.len()]) {
            Ok(Value::Int(value)) => Ok(value),
            Ok(Value::Bool(_)) | Err(_) => Err(RendererRefusal::Evaluation),
        }
    }

    fn bytes(&self, source: &str, value: u32) -> Result<Vec<u8>, RendererRefusal> {
        if char::from_u32(value).is_none() {
            return Err(RendererRefusal::InvalidScalar);
        }
        let program = if source == self.validated_source {
            &self.same_source_program
        } else {
            let bound_entry = DeclarationId::new("format.render-byte");
            self.binding.replay(source, &bound_entry)?
        };
        let value = i64::from(value);
        let length = self.int(program, "format.render-length", &[value])?;
        let length = usize::try_from(length).map_err(|_| RendererRefusal::InvalidLength)?;
        if !(3..=MAX_RENDERED_BYTES).contains(&length) {
            return Err(RendererRefusal::InvalidLength);
        }
        let mut bytes = Vec::with_capacity(length);
        for index in 0..length {
            let byte = self.int(program, "format.render-byte", &[value, index as i64])?;
            bytes.push(u8::try_from(byte).map_err(|_| RendererRefusal::InvalidByte)?);
        }
        Ok(bytes)
    }

    fn render(&self, source: &str, value: u32) -> Result<String, RendererRefusal> {
        String::from_utf8(self.bytes(source, value)?).map_err(|_| RendererRefusal::InvalidByte)
    }
}

pub(crate) fn render(value: u32) -> Result<String, RendererRefusal> {
    static RENDERER: OnceLock<Result<Renderer, RendererRefusal>> = OnceLock::new();
    RENDERER
        .get_or_init(Renderer::derive)
        .as_ref()
        .map_err(|error| *error)?
        .render(SOURCE, value)
}

/// Test-only access to the independently evaluated byte lane.  The ordinary
/// interface intentionally remains text-shaped: this does not make a
/// Kernel-0-owned formatter buffer or grant it production authority.
#[cfg(test)]
pub(crate) fn render_bytes(value: u32) -> Result<Vec<u8>, RendererRefusal> {
    static RENDERER: OnceLock<Result<Renderer, RendererRefusal>> = OnceLock::new();
    RENDERER
        .get_or_init(Renderer::derive)
        .as_ref()
        .map_err(|error| *error)?
        .bytes(SOURCE, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_renderer_replays_the_exact_component_source_before_emitting_bytes() {
        let renderer = Renderer::derive().expect("embedded component must derive");
        assert_eq!(
            renderer.bytes(SOURCE, u32::from('A')).unwrap(),
            b"'A'".to_vec(),
            "the exact embedded source must replay before byte-lane evaluation"
        );

        let drifted = SOURCE.replacen("value == 0", "value == 1", 1);
        assert_ne!(drifted, SOURCE, "source-drift control must mutate bytes");
        assert_eq!(
            renderer.bytes(&drifted, u32::from('A')),
            Err(RendererRefusal::Profile),
            "a cached translation must not evaluate against changed component bytes"
        );
    }
}

#[cfg(test)]
mod shadow {
    use std::cell::Cell;

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static COMPARISONS: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn verify(value: u32, rust: &str) {
        if !ENABLED.get() {
            return;
        }
        let rendered = super::render(value)
            .unwrap_or_else(|error| panic!("Kernel-0 canonical-char shadow refused: {error:?}"));
        assert_eq!(rendered, rust, "Kernel-0 canonical-char shadow disagreed");
        COMPARISONS.set(COMPARISONS.get() + 1);
    }

    pub(super) fn run<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        ENABLED.with(|enabled| {
            assert!(!enabled.replace(true), "canonical-char shadow cannot nest");
        });
        COMPARISONS.set(0);
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                ENABLED.set(false);
            }
        }
        let restore = Restore;
        let output = operation();
        let comparisons = COMPARISONS.get();
        drop(restore);
        (output, comparisons)
    }
}

#[cfg(test)]
pub(crate) fn verify_shadow(value: u32, rust: &str) {
    shadow::verify(value, rust);
}

#[cfg(test)]
pub(crate) fn with_shadow<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    shadow::run(operation)
}
