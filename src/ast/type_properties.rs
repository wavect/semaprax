//! Primitive type and callable ownership classification.
use super::{fmt, Type};

impl Type {
    pub fn is_once_function(&self) -> bool {
        matches!(
            self,
            Self::OnceFunction | Self::OnceFunctionI64 | Self::OnceFunctionI64Pair
        )
    }

    pub fn is_named(&self) -> bool {
        matches!(self, Type::Named { .. })
    }

    /// Canonical ownership predicate. `Bytes` transfers uniquely without
    /// being misclassified as a user resource.
    pub fn is_uniquely_owned(&self) -> bool {
        self.is_once_function()
            || crate::map_ops::ast_collection(self)
            || matches!(
                self,
                Type::String | Type::Bytes | Type::StringMap | Type::MutFunctionI64
            )
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        enum Frame<'a> {
            Type(&'a Type),
            Arguments(&'a [Type], usize),
            FunctionParameters(&'a [Type], usize),
            FunctionResult(&'a Type),
        }
        let mut frames = vec![Frame::Type(self)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Type(Type::I64) => f.write_str("i64")?,
                Frame::Type(Type::I32) => f.write_str("i32")?,
                Frame::Type(Type::Char) => f.write_str("char")?,
                Frame::Type(Type::U8) => f.write_str("u8")?,
                Frame::Type(Type::Usize) => f.write_str("usize")?,
                Frame::Type(Type::ArrayU8(length)) => write!(f, "[u8; {length}]")?,
                Frame::Type(Type::F32) => f.write_str("f32")?,
                Frame::Type(Type::F64) => f.write_str("f64")?,
                Frame::Type(Type::Bool) => f.write_str("bool")?,
                Frame::Type(Type::String) => f.write_str("string")?,
                Frame::Type(Type::Bytes) => f.write_str("Bytes")?,
                Frame::Type(Type::OnceFunction) => f.write_str("FnOnce() -> i64")?,
                Frame::Type(Type::OnceFunctionI64) => f.write_str("FnOnceI64() -> i64")?,
                Frame::Type(Type::OnceFunctionI64Pair) => f.write_str("FnOnceI64Pair() -> i64")?,
                Frame::Type(Type::MutFunctionI64) => f.write_str("FnMutI64(i64) -> i64")?,
                Frame::Type(Type::Str) => f.write_str("str")?,
                Frame::Type(Type::SliceU8) => f.write_str("Slice<u8>")?,
                Frame::Type(Type::StringMap) => f.write_str("Map<string, i64>")?,
                Frame::Type(Type::Function { parameters, result }) => {
                    f.write_str("fn(")?;
                    frames.push(Frame::FunctionResult(result));
                    frames.push(Frame::FunctionParameters(parameters, 0));
                }
                Frame::Type(Type::Named { name, arguments }) => {
                    f.write_str(name)?;
                    if !arguments.is_empty() {
                        f.write_str("<")?;
                        frames.push(Frame::Arguments(arguments, 0));
                    }
                }
                Frame::Arguments(arguments, index) => {
                    if let Some(argument) = arguments.get(index) {
                        if index != 0 {
                            f.write_str(", ")?;
                        }
                        frames.push(Frame::Arguments(arguments, index + 1));
                        frames.push(Frame::Type(argument));
                    } else {
                        f.write_str(">")?;
                    }
                }
                Frame::FunctionParameters(parameters, index) => {
                    if let Some(parameter) = parameters.get(index) {
                        if index != 0 {
                            f.write_str(", ")?;
                        }
                        frames.push(Frame::FunctionParameters(parameters, index + 1));
                        frames.push(Frame::Type(parameter));
                    }
                }
                Frame::FunctionResult(result) => {
                    f.write_str(") -> ")?;
                    frames.push(Frame::Type(result));
                }
            }
        }
        Ok(())
    }
}
