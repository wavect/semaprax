use std::fmt;

use super::Type;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportResult {
    Unit,
    I64,
    Bool,
    /// Only an index-selected Rust API may synthesize this domain value.
    /// Bridge refusal remains a separate status, never a Result::Err.
    ResultI64I64,
}

impl fmt::Display for ImportResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::ResultI64I64 => "Result<i64, i64>",
        })
    }
}

impl ImportResult {
    pub fn value_type(self) -> Type {
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
        }
    }
}
