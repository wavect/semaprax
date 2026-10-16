//! Exact original key traversal with a one-use root completeness destination.
use super::{DeclarationId, ResolvedType};
use std::fmt::Write as _;

// Every formatted key has the same completeness destination as nominal child
// writes. Admission precedes all writer owners/completion references. If it
// fails, the original formatter still executes once, with publication refused.
macro_rules! checked_key_format {
    ($complete:expr, $($argument:tt)*) => {{
        if crate::bounded_output::reserve_active_required(
            crate::bounded_output::CheckedCappedString::CONTROLS,
        ) {
            let mut output = crate::bounded_output::CheckedCappedString::new(&mut *$complete);
            write!(output, $($argument)*).expect("writing to a string cannot fail");
            output.into_string()
        } else {
            *$complete = false;
            format!($($argument)*)
        }
    }};
}

impl ResolvedType {
    pub(in crate::hir) fn identity_key_checked(&self, complete: &mut bool) -> String {
        // Zero-argument nominal keys need no traversal or child-key storage.
        // Keep the exact length-delimited encoding used by the general path.
        if let Self::Nominal {
            declaration,
            arguments,
        } = self
        {
            if arguments.is_empty() {
                return checked_key_format!(
                    complete,
                    "nominal:{}:{}:0:",
                    declaration.as_str().len(),
                    declaration
                );
            }
        }
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a DeclarationId, usize),
            FinishFunction(usize),
        }
        let mut frames = vec![Frame::Enter(self)];
        let mut keys = Vec::<String>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty) => match ty {
                    Self::Function { parameters, result } => {
                        frames.push(Frame::FinishFunction(parameters.len()));
                        frames.push(Frame::Enter(result));
                        frames.extend(parameters.iter().rev().map(Frame::Enter));
                    }
                    Self::OnceFunction => keys.push("fn-once:bytes:i64:v1".to_owned()),
                    Self::OnceFunctionI64 => keys.push("fn-once:bytes+i64:i64:v2".to_owned()),
                    Self::OnceFunctionI64Pair => {
                        keys.push("fn-once:bytes+i64+i64:i64:v3".to_owned())
                    }
                    Self::MutFunctionI64 => keys.push("fn-mut:i64+i64:i64:v1".to_owned()),
                    Self::Never => keys.push("never:lexical-exit:v1".to_owned()),
                    Self::Unit => keys.push("unit".to_owned()),
                    Self::I64 => keys.push("i64".to_owned()),
                    Self::I32 => keys.push("i32".to_owned()),
                    Self::Char => keys.push("char".to_owned()),
                    Self::U8 => keys.push("u8".to_owned()),
                    Self::Usize => keys.push("usize".to_owned()),
                    Self::ArrayU8(length) => {
                        keys.push(checked_key_format!(complete, "array:u8:{length}"))
                    }
                    Self::F32 => keys.push("f32".to_owned()),
                    Self::F64 => keys.push("f64".to_owned()),
                    Self::Bool => keys.push("bool".to_owned()),
                    Self::String => keys.push("string".to_owned()),
                    Self::Bytes => keys.push("bytes".to_owned()),
                    Self::Str => keys.push("str".to_owned()),
                    Self::SliceU8 => keys.push("slice-u8".to_owned()),
                    Self::StringMap => keys.push("map:string:i64:v1".to_owned()),
                    Self::TypeParameter { owner, index } => keys.push(checked_key_format!(
                        complete,
                        "parameter:{}:{}:{index}",
                        owner.as_str().len(),
                        owner
                    )),
                    Self::Nominal {
                        declaration,
                        arguments,
                    } => {
                        frames.push(Frame::Finish(declaration, arguments.len()));
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::FinishFunction(count) => {
                    let split = keys
                        .len()
                        .checked_sub(count + 1)
                        .expect("function key children retained");
                    let mut key = checked_key_format!(complete, "function:v1:{count}:");
                    for child in keys.drain(split..) {
                        write!(key, "{}:{child}", child.len()).expect("string write");
                    }
                    keys.push(key);
                }
                Frame::Finish(declaration, count) => {
                    let split = keys
                        .len()
                        .checked_sub(count)
                        .expect("type-key traversal has one result per argument");
                    if crate::bounded_output::reserve_active_required(
                        crate::bounded_output::CheckedCappedString::CONTROLS
                            + std::mem::size_of::<String>(),
                    ) {
                        let mut encoded =
                            crate::bounded_output::CheckedCappedString::new(&mut *complete);
                        for key in keys.drain(split..) {
                            write!(encoded, "{}:{key}", key.len())
                                .expect("writing to a string cannot fail");
                        }
                        // Release the inner completion borrow before constructing
                        // the next checked writer; this named String slot is
                        // admitted with the constructor above.
                        let encoded = encoded.into_string();
                        keys.push(checked_key_format!(
                            complete,
                            "nominal:{}:{}:{}:{}",
                            declaration.as_str().len(),
                            declaration,
                            count,
                            encoded
                        ));
                    } else {
                        *complete = false;
                        let mut encoded = crate::bounded_output::CappedString::new();
                        for key in keys.drain(split..) {
                            write!(encoded, "{}:{key}", key.len())
                                .expect("writing to a string cannot fail");
                        }
                        keys.push(checked_key_format!(
                            complete,
                            "nominal:{}:{}:{}:{}",
                            declaration.as_str().len(),
                            declaration,
                            count,
                            encoded.into_string()
                        ));
                    }
                }
            }
        }
        keys.pop().expect("a type always produces an identity key")
    }
}

// Complete previous checked encoder, test-only; method identity is the sole edit.
#[cfg(test)]
impl ResolvedType {
    pub(in crate::hir) fn identity_key_before_format_completion(
        &self,
        complete: &mut bool,
    ) -> String {
        // Zero-argument nominal keys need no traversal or child-key storage.
        // Keep the exact length-delimited encoding used by the general path.
        if let Self::Nominal {
            declaration,
            arguments,
        } = self
        {
            if arguments.is_empty() {
                return format!("nominal:{}:{}:0:", declaration.as_str().len(), declaration);
            }
        }
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a DeclarationId, usize),
            FinishFunction(usize),
        }
        let mut frames = vec![Frame::Enter(self)];
        let mut keys = Vec::<String>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty) => match ty {
                    Self::Function { parameters, result } => {
                        frames.push(Frame::FinishFunction(parameters.len()));
                        frames.push(Frame::Enter(result));
                        frames.extend(parameters.iter().rev().map(Frame::Enter));
                    }
                    Self::OnceFunction => keys.push("fn-once:bytes:i64:v1".to_owned()),
                    Self::OnceFunctionI64 => keys.push("fn-once:bytes+i64:i64:v2".to_owned()),
                    Self::OnceFunctionI64Pair => {
                        keys.push("fn-once:bytes+i64+i64:i64:v3".to_owned())
                    }
                    Self::MutFunctionI64 => keys.push("fn-mut:i64+i64:i64:v1".to_owned()),
                    Self::Never => keys.push("never:lexical-exit:v1".to_owned()),
                    Self::Unit => keys.push("unit".to_owned()),
                    Self::I64 => keys.push("i64".to_owned()),
                    Self::I32 => keys.push("i32".to_owned()),
                    Self::Char => keys.push("char".to_owned()),
                    Self::U8 => keys.push("u8".to_owned()),
                    Self::Usize => keys.push("usize".to_owned()),
                    Self::ArrayU8(length) => keys.push(format!("array:u8:{length}")),
                    Self::F32 => keys.push("f32".to_owned()),
                    Self::F64 => keys.push("f64".to_owned()),
                    Self::Bool => keys.push("bool".to_owned()),
                    Self::String => keys.push("string".to_owned()),
                    Self::Bytes => keys.push("bytes".to_owned()),
                    Self::Str => keys.push("str".to_owned()),
                    Self::SliceU8 => keys.push("slice-u8".to_owned()),
                    Self::StringMap => keys.push("map:string:i64:v1".to_owned()),
                    Self::TypeParameter { owner, index } => keys.push(format!(
                        "parameter:{}:{}:{index}",
                        owner.as_str().len(),
                        owner
                    )),
                    Self::Nominal {
                        declaration,
                        arguments,
                    } => {
                        frames.push(Frame::Finish(declaration, arguments.len()));
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::FinishFunction(count) => {
                    let split = keys
                        .len()
                        .checked_sub(count + 1)
                        .expect("function key children retained");
                    let mut key = format!("function:v1:{count}:");
                    for child in keys.drain(split..) {
                        write!(key, "{}:{child}", child.len()).expect("string write");
                    }
                    keys.push(key);
                }
                Frame::Finish(declaration, count) => {
                    let split = keys
                        .len()
                        .checked_sub(count)
                        .expect("type-key traversal has one result per argument");
                    if crate::bounded_output::reserve_active_required(
                        crate::bounded_output::CheckedCappedString::CONTROLS,
                    ) {
                        let mut encoded =
                            crate::bounded_output::CheckedCappedString::new(&mut *complete);
                        for key in keys.drain(split..) {
                            write!(encoded, "{}:{key}", key.len())
                                .expect("writing to a string cannot fail");
                        }
                        keys.push(format!(
                            "nominal:{}:{}:{}:{}",
                            declaration.as_str().len(),
                            declaration,
                            count,
                            encoded.into_string()
                        ));
                    } else {
                        *complete = false;
                        let mut encoded = crate::bounded_output::CappedString::new();
                        for key in keys.drain(split..) {
                            write!(encoded, "{}:{key}", key.len())
                                .expect("writing to a string cannot fail");
                        }
                        keys.push(format!(
                            "nominal:{}:{}:{}:{}",
                            declaration.as_str().len(),
                            declaration,
                            count,
                            encoded.into_string()
                        ));
                    }
                }
            }
        }
        keys.pop().expect("a type always produces an identity key")
    }
}
