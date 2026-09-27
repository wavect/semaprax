//! Exact-source Kernel-0 component boundary for canonical boolean rendering.
//!
//! The pure component renders one `bool` through a fixed, bounded byte lane.
//! It retains and replays its exact embedded source before every complete
//! evaluation. The production adapter byte-compares it with Rust before
//! copying to its caller-owned output; tests retain an additional shadow hook.

use std::sync::OnceLock;

use crate::hir::DeclarationId;

use super::eval::eval_program;
use super::reify::{BoundTranslation, Refusal};
use super::term::KernelProgram;
use super::value::Value;

pub(crate) const SOURCE: &str = include_str!("canonical_bool_renderer.spx");
const MIN_RENDERED_BYTES: usize = 4;
const MAX_RENDERED_BYTES: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RendererRefusal {
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
    /// Retain the source binding, rather than only a cached translated
    /// program, so a `source` differing from `validated_source` below (only
    /// the drift-detection test below passes one) can never authorize
    /// drifted source bytes.
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
    /// bool bought nothing beyond the first call and dominated this
    /// renderer's cost under any caller that renders many bools from one
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
        arguments: &[Value],
    ) -> Result<i64, RendererRefusal> {
        let entry = program
            .function(&DeclarationId::new(entry))
            .ok_or(RendererRefusal::MissingEntry)?;
        match eval_program(program, entry, arguments) {
            Ok(Value::Int(value)) => Ok(value),
            Ok(Value::Bool(_)) | Err(_) => Err(RendererRefusal::Evaluation),
        }
    }

    fn bytes(&self, source: &str, value: bool) -> Result<Vec<u8>, RendererRefusal> {
        let program = if source == self.validated_source {
            &self.same_source_program
        } else {
            let bound_entry = DeclarationId::new("format.render-byte");
            self.binding.replay(source, &bound_entry)?
        };
        let length = self.int(program, "format.render-length", &[Value::Bool(value)])?;
        let length = usize::try_from(length).map_err(|_| RendererRefusal::InvalidLength)?;
        if !(MIN_RENDERED_BYTES..=MAX_RENDERED_BYTES).contains(&length) {
            return Err(RendererRefusal::InvalidLength);
        }
        let mut bytes = Vec::with_capacity(length);
        for index in 0..length {
            let byte = self.int(
                program,
                "format.render-byte",
                &[Value::Bool(value), Value::Int(index as i64)],
            )?;
            bytes.push(u8::try_from(byte).map_err(|_| RendererRefusal::InvalidByte)?);
        }
        Ok(bytes)
    }

    fn render(&self, source: &str, value: bool) -> Result<String, RendererRefusal> {
        String::from_utf8(self.bytes(source, value)?).map_err(|_| RendererRefusal::InvalidByte)
    }
}

pub(crate) fn render(value: bool) -> Result<String, RendererRefusal> {
    static RENDERER: OnceLock<Result<Renderer, RendererRefusal>> = OnceLock::new();
    RENDERER
        .get_or_init(Renderer::derive)
        .as_ref()
        .map_err(|error| *error)?
        .render(SOURCE, value)
}

#[cfg(test)]
pub(crate) fn render_bytes(value: bool) -> Result<Vec<u8>, RendererRefusal> {
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
            renderer.bytes(SOURCE, true).unwrap(),
            b"true".to_vec(),
            "the exact embedded source must replay before byte-lane evaluation"
        );

        let drifted = SOURCE.replacen("if value {", "if !value {", 1);
        assert_ne!(drifted, SOURCE, "source-drift control must mutate bytes");
        assert_eq!(
            renderer.bytes(&drifted, false),
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

    pub(super) fn verify(value: bool, rust: &str) {
        if !ENABLED.get() {
            return;
        }
        let rendered = super::render(value)
            .unwrap_or_else(|error| panic!("Kernel-0 canonical-bool shadow refused: {error:?}"));
        assert_eq!(rendered, rust, "Kernel-0 canonical-bool shadow disagreed");
        COMPARISONS.set(COMPARISONS.get() + 1);
    }

    pub(super) fn run<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        ENABLED.with(|enabled| {
            assert!(!enabled.replace(true), "canonical-bool shadow cannot nest");
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
pub(crate) fn verify_shadow(value: bool, rust: &str) {
    shadow::verify(value, rust);
}

#[cfg(test)]
pub(crate) fn with_shadow<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    shadow::run(operation)
}
