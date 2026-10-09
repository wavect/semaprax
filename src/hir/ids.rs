//! Deterministic HIR identity newtypes.
//!
//! Declaration, function-instance, execution, value, and expression
//! identities plus the exact string encodings that derive them.

use std::fmt;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub(super) Arc<str>, u64);

impl ValueId {
    pub(super) fn new(value: String) -> Self {
        let value = exact_string(value);
        let fingerprint = value.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        Self(Arc::from(value), fingerprint)
    }

    /// Synthetic identity for one compiler-owned intrinsic operation
    /// parameter; intrinsic operations have no authored declaration, so the
    /// identity only labels diagnostics and never indexes a binding.
    pub(crate) fn intrinsic_parameter(operation: &str, index: usize) -> Self {
        Self::new(format!("{operation}.param.{index}"))
    }

    pub(crate) fn parameter(function: &FunctionExecutionId, index: usize) -> Self {
        Self::new(scoped_identity(function, "value:param", &index.to_string()))
    }

    pub(crate) fn local(function: &FunctionExecutionId, path: &str) -> Self {
        Self::new(scoped_identity(function, "value:local", path))
    }

    pub(crate) fn result(function: &FunctionExecutionId) -> Self {
        Self::new(scoped_identity(function, "value:result", ""))
    }

    pub(super) fn matches_parameter(&self, function: &FunctionExecutionId, index: usize) -> bool {
        self.matches_scoped(function, "value:param", decimal_digits(index), index)
    }

    pub(super) fn matches_local(&self, function: &FunctionExecutionId, path: &str) -> bool {
        self.matches_scoped(function, "value:local", path.len(), path)
    }

    pub(super) fn matches_result(&self, function: &FunctionExecutionId) -> bool {
        self.matches_scoped(function, "value:result", 0, "")
    }

    fn matches_scoped(
        &self,
        function: &FunctionExecutionId,
        kind: &str,
        path_length: usize,
        path: impl fmt::Display,
    ) -> bool {
        // ValueId equality includes its cached hash as well as the exact
        // bytes. Preserve that check even for privately forged HIR carriers.
        let fingerprint = self.0.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        self.1 == fingerprint
            && matches_scoped_identity(self.as_str(), function, kind, path_length, path)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Hash for ValueId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.1);
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone)]
pub struct ExpressionId(Option<Arc<String>>);

impl ExpressionId {
    // Arc owns two reference counters beside the moved String carrier. Its
    // payload keeps the exact-capacity buffer produced by exact_string.
    pub(crate) const SHARED_ALLOCATION_CARRIER_BYTES: usize =
        2 * std::mem::size_of::<usize>() + std::mem::size_of::<String>();

    pub(crate) fn new(function: &FunctionExecutionId, path: &str) -> Self {
        Self::from_owned(scoped_identity(function, "expression", path))
    }

    pub(crate) fn from_owned(value: String) -> Self {
        if !crate::bounded_output::reserve_active_required(Self::SHARED_ALLOCATION_CARRIER_BYTES) {
            // The caller's existing overflow flag rejects the enclosing work.
            // Do not allocate even an empty shared carrier after refusal.
            return Self(None);
        }
        Self(Some(Arc::new(exact_string(value))))
    }

    pub(super) fn matches(&self, function: &FunctionExecutionId, path: &str) -> bool {
        matches_scoped_identity(self.as_str(), function, "expression", path.len(), path)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_deref().map_or("", String::as_str)
    }

    pub(super) fn cached_text(&self) -> Option<&String> {
        self.0.as_deref()
    }

    pub(crate) fn shared_allocation_key(&self) -> Option<usize> {
        self.0.as_ref().map(|backing| Arc::as_ptr(backing) as usize)
    }

    pub(crate) fn shared_allocation_bytes(&self) -> Option<usize> {
        self.0.as_ref().and_then(|backing| {
            Self::SHARED_ALLOCATION_CARRIER_BYTES.checked_add(backing.capacity())
        })
    }
}

impl fmt::Debug for ExpressionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ExpressionId")
            .field(&self.as_str())
            .finish()
    }
}

impl PartialEq for ExpressionId {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ExpressionId {}

impl PartialOrd for ExpressionId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExpressionId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for ExpressionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
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
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "ids/shared_identity_tests.rs"]
mod shared_identity_tests;

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

// Validation compares the canonical spelling directly with the retained bytes.
// The allocating encoder above remains the construction path and independent
// reference: no identity is interned, cached, truncated, or accepted by a hash.
fn matches_scoped_identity(
    actual: &str,
    owner: &FunctionExecutionId,
    kind: &str,
    path_length: usize,
    path: impl fmt::Display,
) -> bool {
    struct Compare<'a>(&'a [u8]);
    impl fmt::Write for Compare<'_> {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.0 = self.0.strip_prefix(value.as_bytes()).ok_or(fmt::Error)?;
            Ok(())
        }
    }
    let mut output = Compare(actual.as_bytes());
    let result = match owner {
        FunctionExecutionId::Monomorphic(owner) => write!(
            output,
            "declaration:{}:{}:{kind}:{path_length}:{path}",
            owner.as_str().len(),
            owner
        ),
        FunctionExecutionId::Generic(owner) => {
            const PREFIX: &str = "semaprax.function-execution.v1:generic:";
            let Some(length) = PREFIX
                .len()
                .checked_add(decimal_digits(owner.as_str().len()))
                .and_then(|length| length.checked_add(1))
                .and_then(|length| length.checked_add(owner.as_str().len()))
            else {
                return false;
            };
            write!(
                output,
                "function-execution:{length}:{PREFIX}{}:{}:{kind}:{path_length}:{path}",
                owner.as_str().len(),
                owner
            )
        }
    };
    result.is_ok() && output.0.is_empty()
}

fn decimal_digits(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    }
}

#[cfg(test)]
#[path = "ids/identity_comparison_tests.rs"]
mod identity_comparison_tests;
