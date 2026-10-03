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
    /// Rust String retained by the bounded native owned-value bridge.
    OwnedString,
    OwnedOptionString,
    OwnedResultStringI64,
    OwnedResultStringOptionI64,
    /// A selected native constructor whose success arm is one owned resource
    /// and whose error arm is a stable `i64` domain code.
    OwnedResultResourceI64 {
        name: String,
    },
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
            Self::OwnedString => "string",
            Self::OwnedOptionString => "Option<string>",
            Self::OwnedResultStringI64 => "Result<string, i64>",
            Self::OwnedResultStringOptionI64 => "Result<string, Option<i64>>",
            Self::OwnedResultResourceI64 { name } => return write!(f, "Result<{name}, i64>"),
            Self::ResultI64I64 => "Result<i64, i64>",
            Self::OwnedResource { name } => name,
        })
    }
}

impl ImportResult {
    pub fn is_owned(&self) -> bool {
        matches!(
            self,
            Self::OwnedResource { .. }
                | Self::OwnedString
                | Self::OwnedOptionString
                | Self::OwnedResultStringI64
                | Self::OwnedResultStringOptionI64
                | Self::OwnedResultResourceI64 { .. }
        )
    }
    pub fn container_for_type(ty: &Type) -> Option<Self> {
        match ty {
            Type::Named { name, arguments } if name == "Option" && arguments == &[Type::String] => {
                Some(Self::OwnedOptionString)
            }
            Type::Named { name, arguments }
                if name == "Result" && arguments == &[Type::String, Type::I64] =>
            {
                Some(Self::OwnedResultStringI64)
            }
            Type::Named { name, arguments }
                if name == "Result"
                    && matches!(arguments.as_slice(), [Type::String, Type::Named { name, arguments }] if name == "Option" && arguments == &[Type::I64]) =>
            {
                Some(Self::OwnedResultStringOptionI64)
            }
            _ => None,
        }
    }

    /// Closed native container admission never widens ordinary generic programs.
    pub(crate) fn container_enabled(program: &super::Program, ty: &Type) -> bool {
        if let Some(kind) = Self::container_for_type(ty) {
            return program
                .interfaces
                .iter()
                .flat_map(|i| &i.imports)
                .any(|i| i.native_rust && i.result == kind);
        }
        let Type::Named { name, arguments } = ty else {
            return false;
        };
        let [Type::Named {
            name: resource,
            arguments: resource_arguments,
        }, Type::I64] = arguments.as_slice()
        else {
            return false;
        };
        name == "Result"
            && resource_arguments.is_empty()
            && program
                .interfaces
                .iter()
                .flat_map(|interface| &interface.imports)
                .any(|import| {
                    import.native_rust
                        && matches!(
                            &import.result,
                            Self::OwnedResultResourceI64 { name }
                                if name == resource
                        )
                })
    }

    pub fn value_type(&self) -> Type {
        match self {
            Self::Unit => Type::Named {
                name: "\0native-rust-unit".to_owned(),
                arguments: Vec::new(),
            },
            Self::I64 => Type::I64,
            Self::Bool => Type::Bool,
            Self::OwnedString => Type::String,
            Self::OwnedOptionString => Type::Named {
                name: "Option".into(),
                arguments: vec![Type::String],
            },
            Self::OwnedResultStringI64 => Type::Named {
                name: "Result".into(),
                arguments: vec![Type::String, Type::I64],
            },
            Self::OwnedResultStringOptionI64 => Type::Named {
                name: "Result".into(),
                arguments: vec![
                    Type::String,
                    Type::Named {
                        name: "Option".into(),
                        arguments: vec![Type::I64],
                    },
                ],
            },
            Self::OwnedResultResourceI64 { name } => Type::Named {
                name: "Result".to_owned(),
                arguments: vec![
                    Type::Named {
                        name: name.clone(),
                        arguments: Vec::new(),
                    },
                    Type::I64,
                ],
            },
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
