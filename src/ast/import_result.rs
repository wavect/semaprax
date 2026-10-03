use std::fmt;

use super::Type;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportResult {
    Unit,
    I64,
    Bool,
    /// Only an index-selected Rust API may synthesize this domain value.
    /// Bridge refusal remains a separate status, never a Result::Err.
    ResultI64I64,
    /// An opaque resource returned by a native Rust constructor. The source
    /// verifier resolves `name` to an authored `resource` declaration.
    OwnedResource {
        name: String,
    },
}

impl fmt::Display for ImportResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::ResultI64I64 => "Result<i64, i64>",
            Self::OwnedResource { name } => name,
        })
    }
}

impl ImportResult {
    pub fn value_type(&self) -> Type {
        match self {
            Self::Unit => Type::Named {
                name: "\0native-rust-unit".to_owned(),
                arguments: Vec::new(),
            },
            Self::I64 => Type::I64,
            Self::Bool => Type::Bool,
            Self::ResultI64I64 => Type::Named {
                name: "Result".to_owned(),
                arguments: vec![Type::I64, Type::I64],
            },
            Self::OwnedResource { name } => Type::Named {
                name: name.clone(),
                arguments: Vec::new(),
            },
        }
    }
}
