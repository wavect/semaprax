//! Exact-source Kernel-0 component boundary for canonical operator tokens.
//!
//! This private component maps a closed, Rust-owned opcode inventory to the
//! canonical bytes for every `BinaryOp` and `UnaryOp` spelling. It has no
//! ambient authority and no output buffer: the host reads a bounded one- or
//! two-byte lane only after independently replaying the exact embedded source.
//! Rust remains the formatter authority; the production adapter requires full
//! byte equality before it copies a candidate to formatter output, while
//! test-only shadows make any mismatch independently observable.

use std::sync::OnceLock;

use crate::ast::{BinaryOp, UnaryOp};
use crate::hir::DeclarationId;

use super::eval::eval_program;
use super::reify::{BoundTranslation, Refusal};
use super::term::KernelProgram;
use super::value::Value;

pub(crate) const SOURCE: &str = include_str!("canonical_operator_renderer.spx");
const MIN_RENDERED_BYTES: usize = 1;
const MAX_RENDERED_BYTES: usize = 2;

const ADD: i64 = 0;
const SUB: i64 = 1;
const MUL: i64 = 2;
const DIV: i64 = 3;
const REM: i64 = 4;
const EQ: i64 = 5;
const NE: i64 = 6;
const LT: i64 = 7;
const LE: i64 = 8;
const GT: i64 = 9;
const GE: i64 = 10;
const AND: i64 = 11;
const OR: i64 = 12;
const NEG: i64 = 13;
const NOT: i64 = 14;
const MAX_OPCODE: i64 = NOT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RendererRefusal {
    Profile,
    MissingEntry,
    Evaluation,
    InvalidOpcode,
    InvalidLength,
    InvalidByte,
}

impl From<Refusal> for RendererRefusal {
    fn from(_: Refusal) -> Self {
        Self::Profile
    }
}

struct Renderer {
    /// Retain the exact binding, not merely its first translated program, so
    /// a `source` byte-different from `validated_source` below (only the
    /// drift-detection test below passes one) can never authorize changed
    /// component bytes.
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
    /// operator bought nothing beyond the first call and dominated this
    /// renderer's cost under any caller that renders many operators from one
    /// process (see issue #294).
    same_source_program: KernelProgram,
}

impl Renderer {
    fn derive() -> Result<Self, RendererRefusal> {
        Self::derive_from_source(SOURCE)
    }

    fn derive_from_source(source: &str) -> Result<Self, RendererRefusal> {
        let entry = DeclarationId::new("format.operator-render-byte");
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

    fn token(
        &self,
        source: &str,
        opcode: i64,
    ) -> Result<([u8; MAX_RENDERED_BYTES], usize), RendererRefusal> {
        if !(0..=MAX_OPCODE).contains(&opcode) {
            return Err(RendererRefusal::InvalidOpcode);
        }
        let program = if source == self.validated_source {
            &self.same_source_program
        } else {
            let bound_entry = DeclarationId::new("format.operator-render-byte");
            self.binding.replay(source, &bound_entry)?
        };
        let length = self.int(
            program,
            "format.operator-render-length",
            &[Value::Int(opcode)],
        )?;
        let length = usize::try_from(length).map_err(|_| RendererRefusal::InvalidLength)?;
        if !(MIN_RENDERED_BYTES..=MAX_RENDERED_BYTES).contains(&length) {
            return Err(RendererRefusal::InvalidLength);
        }
        let mut bytes = [0; MAX_RENDERED_BYTES];
        for index in 0..length {
            let byte = self.int(
                program,
                "format.operator-render-byte",
                &[Value::Int(opcode), Value::Int(index as i64)],
            )?;
            bytes[index] = u8::try_from(byte).map_err(|_| RendererRefusal::InvalidByte)?;
        }
        Ok((bytes, length))
    }

    #[cfg(test)]
    fn bytes(&self, source: &str, opcode: i64) -> Result<Vec<u8>, RendererRefusal> {
        let (bytes, length) = self.token(source, opcode)?;
        Ok(bytes[..length].to_vec())
    }

    fn render(&self, source: &str, opcode: i64) -> Result<&'static str, RendererRefusal> {
        let (bytes, length) = self.token(source, opcode)?;
        match &bytes[..length] {
            b"+" => Ok("+"),
            b"-" => Ok("-"),
            b"*" => Ok("*"),
            b"/" => Ok("/"),
            b"%" => Ok("%"),
            b"==" => Ok("=="),
            b"!=" => Ok("!="),
            b"<" => Ok("<"),
            b"<=" => Ok("<="),
            b">" => Ok(">"),
            b">=" => Ok(">="),
            b"&&" => Ok("&&"),
            b"||" => Ok("||"),
            b"!" => Ok("!"),
            _ => Err(RendererRefusal::InvalidByte),
        }
    }
}

fn binary_opcode(op: BinaryOp) -> i64 {
    match op {
        BinaryOp::Add => ADD,
        BinaryOp::Sub => SUB,
        BinaryOp::Mul => MUL,
        BinaryOp::Div => DIV,
        BinaryOp::Rem => REM,
        BinaryOp::Eq => EQ,
        BinaryOp::Ne => NE,
        BinaryOp::Lt => LT,
        BinaryOp::Le => LE,
        BinaryOp::Gt => GT,
        BinaryOp::Ge => GE,
        BinaryOp::And => AND,
        BinaryOp::Or => OR,
    }
}

fn unary_opcode(op: UnaryOp) -> i64 {
    match op {
        UnaryOp::Neg => NEG,
        UnaryOp::Not => NOT,
    }
}

fn render(opcode: i64) -> Result<&'static str, RendererRefusal> {
    static RENDERER: OnceLock<Result<Renderer, RendererRefusal>> = OnceLock::new();
    RENDERER
        .get_or_init(Renderer::derive)
        .as_ref()
        .map_err(|error| *error)?
        .render(SOURCE, opcode)
}

pub(crate) fn render_binary(op: BinaryOp) -> Result<String, RendererRefusal> {
    Ok(render(binary_opcode(op))?.to_owned())
}

pub(crate) fn render_unary(op: UnaryOp) -> Result<String, RendererRefusal> {
    Ok(render(unary_opcode(op))?.to_owned())
}

#[cfg(test)]
pub(crate) fn render_bytes(opcode: i64) -> Result<Vec<u8>, RendererRefusal> {
    static RENDERER: OnceLock<Result<Renderer, RendererRefusal>> = OnceLock::new();
    RENDERER
        .get_or_init(Renderer::derive)
        .as_ref()
        .map_err(|error| *error)?
        .bytes(SOURCE, opcode)
}

/// Test-only single-byte inspection of the same full, source-replayed lane.
/// The component never exposes an unchecked index: callers can observe only
/// bytes belonging to a successfully bounded complete rendering.
#[cfg(test)]
pub(crate) fn render_byte(opcode: i64, index: usize) -> Result<u8, RendererRefusal> {
    render_bytes(opcode)?
        .get(index)
        .copied()
        .ok_or(RendererRefusal::InvalidByte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_renderer_replays_the_exact_component_source_before_emitting_bytes() {
        let renderer = Renderer::derive().expect("embedded component must derive");
        assert_eq!(
            renderer.bytes(SOURCE, EQ).unwrap(),
            b"==".to_vec(),
            "the exact embedded source must replay before byte-lane evaluation"
        );

        let drifted = SOURCE.replacen("code == 5", "code == 15", 1);
        assert_ne!(drifted, SOURCE, "source-drift control must mutate bytes");
        assert_eq!(
            renderer.bytes(&drifted, EQ),
            Err(RendererRefusal::Profile),
            "a cached translation must not evaluate against changed component bytes"
        );
    }
}

#[cfg(test)]
mod shadow {
    use std::cell::Cell;

    use crate::ast::{BinaryOp, UnaryOp};

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static COMPARISONS: Cell<usize> = const { Cell::new(0) };
    }

    fn verify(opcode: i64, rust: &str) {
        if !ENABLED.get() {
            return;
        }
        let rendered = super::render(opcode).unwrap_or_else(|error| {
            panic!("Kernel-0 canonical-operator shadow refused: {error:?}")
        });
        assert_eq!(
            rendered, rust,
            "Kernel-0 canonical-operator shadow disagreed"
        );
        COMPARISONS.set(COMPARISONS.get() + 1);
    }

    pub(super) fn verify_binary(op: BinaryOp, rust: &str) {
        verify(super::binary_opcode(op), rust);
    }

    pub(super) fn verify_unary(op: UnaryOp, rust: &str) {
        verify(super::unary_opcode(op), rust);
    }

    pub(super) fn run<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        ENABLED.with(|enabled| {
            assert!(
                !enabled.replace(true),
                "canonical-operator shadow cannot nest"
            );
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
pub(crate) fn verify_binary_shadow(op: BinaryOp, rust: &str) {
    shadow::verify_binary(op, rust);
}

#[cfg(test)]
pub(crate) fn verify_unary_shadow(op: UnaryOp, rust: &str) {
    shadow::verify_unary(op, rust);
}

#[cfg(test)]
pub(crate) fn with_shadow<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    shadow::run(operation)
}
