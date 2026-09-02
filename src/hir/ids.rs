//! Deterministic HIR identity newtypes.
//!
//! Declaration, function-instance, execution, value, and expression
//! identities plus the exact string encodings that derive them.

use std::fmt;
use std::fmt::Write as _;

use super::nodes::ResolvedType;

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeclarationId(pub(super) String);

impl DeclarationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(exact_string(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for DeclarationId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

impl fmt::Display for DeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionInstanceId(pub(super) String);

impl FunctionInstanceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for FunctionInstanceId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionExecutionId {
    Monomorphic(DeclarationId),
    Generic(FunctionInstanceId),
}

impl FunctionExecutionId {
    pub(super) fn diagnostic_text(&self) -> &str {
        match self {
            Self::Monomorphic(id) => id.as_str(),
            Self::Generic(id) => id.as_str(),
        }
    }

    pub fn identity_key(&self) -> String {
        match self {
            Self::Monomorphic(declaration) => format!(
                "semaprax.function-execution.v1:monomorphic:{}:{}",
                declaration.as_str().len(),
                declaration
            ),
            Self::Generic(instance) => format!(
                "semaprax.function-execution.v1:generic:{}:{}",
                instance.as_str().len(),
                instance
            ),
        }
    }

    pub fn instance(&self) -> Option<&FunctionInstanceId> {
        match self {
            Self::Monomorphic(_) => None,
            Self::Generic(instance) => Some(instance),
        }
    }

    pub fn monomorphic_declaration(&self) -> Option<&DeclarationId> {
        match self {
            Self::Monomorphic(declaration) => Some(declaration),
            Self::Generic(_) => None,
        }
    }
}

impl fmt::Display for FunctionExecutionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.diagnostic_text())
    }
}

impl fmt::Display for FunctionInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub(super) String);

impl ValueId {
    /// Synthetic identity for one compiler-owned intrinsic operation
    /// parameter; intrinsic operations have no authored declaration, so the
    /// identity only labels diagnostics and never indexes a binding.
    pub(crate) fn intrinsic_parameter(operation: &str, index: usize) -> Self {
        Self(exact_string(format!("{operation}.param.{index}")))
    }

    pub(super) fn parameter(function: &FunctionExecutionId, index: usize) -> Self {
        Self(exact_string(scoped_identity(
            function,
            "value:param",
            &index.to_string(),
        )))
    }

    pub(super) fn local(function: &FunctionExecutionId, path: &str) -> Self {
        Self(exact_string(scoped_identity(function, "value:local", path)))
    }

    pub(super) fn result(function: &FunctionExecutionId) -> Self {
        Self(exact_string(scoped_identity(function, "value:result", "")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for ValueId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExpressionId(pub(super) String);

impl ExpressionId {
    pub(crate) fn new(function: &FunctionExecutionId, path: &str) -> Self {
        Self(exact_string(scoped_identity(function, "expression", path)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for ExpressionId {
    fn clone(&self) -> Self {
        Self(exact_string(self.0.clone()))
    }
}

pub(super) fn exact_string(value: String) -> String {
    value.into_boxed_str().into_string()
}

pub(super) fn scoped_identity(owner: &FunctionExecutionId, kind: &str, path: &str) -> String {
    match owner {
        FunctionExecutionId::Monomorphic(owner) => format!(
            "declaration:{}:{}:{kind}:{}:{path}",
            owner.as_str().len(),
            owner,
            path.len()
        ),
        FunctionExecutionId::Generic(_) => {
            let owner = owner.identity_key();
            format!(
                "function-execution:{}:{}:{kind}:{}:{path}",
                owner.len(),
                owner,
                path.len()
            )
        }
    }
}

impl fmt::Display for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FunctionInstanceId {
    pub fn derive(template: &DeclarationId, arguments: &[ResolvedType]) -> Self {
        let mut encoded_arguments = crate::bounded_output::CappedString::new();
        for argument in arguments {
            let key = argument.identity_key();
            write!(encoded_arguments, "{}:{key}", key.len())
                .expect("writing to a string cannot fail");
        }
        Self(exact_string(format!(
            "semaprax.function-instance.v1:{}:{}:{}:{}",
            template.as_str().len(),
            template,
            arguments.len(),
            encoded_arguments.into_string()
        )))
    }
}
