//! Deterministic HIR identity newtypes.
//!
//! Declaration, function-instance, execution, value, and expression
//! identities plus the exact string encodings that derive them.

use std::fmt;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::nodes::ResolvedType;

mod compiler;
pub(crate) use compiler::CompilerDeclaration;

/// A valid declaration identity. Every value holds real text: either a fixed
/// compiler constant borrowed for the process lifetime, or one immutable
/// exact-length shared backing that every clone references. There is no
/// refused, empty-sentinel or cached-fingerprint state. A funded construction
/// that is refused returns `Err` from its constructor, so a refusal never
/// reaches HIR, maps, comparison or the cache wire.
///
/// Equality, order, hash, `Borrow<str>`, display, debug and the cache wire are
/// exactly those of the owned text. Neither the variant nor the pointer is
/// consulted, so a compiler identity, a shared identity and a clone of either
/// are interchangeable values with no authority beyond their bytes.
#[derive(Clone)]
pub struct DeclarationId(DeclarationText);

#[derive(Clone)]
enum DeclarationText {
    CompilerStatic(CompilerDeclaration),
    Shared(Arc<str>),
}

// The whole former 24-byte contract remains compiled in the independent test
// oracle. This candidate promises no target-specific reduced width until its
// actual target layout is observed; all typed reservation formulas keep using
// size_of of the actual receiver. No backing, text or bookkeeping fee changes.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<DeclarationId>() <= std::mem::size_of::<String>());
    assert!(std::mem::align_of::<DeclarationId>() <= std::mem::align_of::<String>());
    assert!(std::mem::size_of::<Option<DeclarationId>>() <= std::mem::size_of::<Option<String>>());
};

impl DeclarationId {
    /// Legacy infallible construction. It is not budgeted and not funded: it
    /// consults no payer, active or ambient, and its shared backing is
    /// charged to nobody. A borrowed argument is first copied into a
    /// temporary `String`, which is then copied into the backing. Funded
    /// callers use `paid_borrowed` or `paid_owned`; compiler constants use
    /// `compiler`. A source control keeps this constructor out of every
    /// qualified funded domain.
    pub fn new(value: impl Into<String>) -> Self {
        let value: String = value.into();
        Self(DeclarationText::Shared(Arc::from(value)))
    }

    /// One fixed compiler identity. It allocates nothing, consults no payer
    /// and grants no prelude or admission authority; only the closed
    /// `CompilerDeclaration` set can reach it.
    pub(crate) const fn compiler(id: CompilerDeclaration) -> Self {
        Self(DeclarationText::CompilerStatic(id))
    }

    /// Funded construction from borrowed text. The caller's own payer is
    /// asked for exactly the shared backing before it exists; no temporary
    /// `String` is made, so the backing is the only allocation. A refusal is
    /// returned unchanged and nothing is allocated.
    pub(crate) fn paid_borrowed<E>(
        value: &str,
        reserve: impl FnOnce(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        Self::reserve_backing(value.len(), reserve)?;
        Ok(Self(DeclarationText::Shared(Arc::from(value))))
    }

    /// Funded construction from text the caller already owns and has already
    /// paid for. The payer is asked for exactly the shared backing; the
    /// caller's `String` stays live beside it while its bytes are copied and
    /// is released afterward. The overlap is therefore that `String` plus the
    /// backing, and nothing about the `String` is charged or refunded here. A
    /// refusal returns the `String` to nobody: it is dropped unconstructed.
    // No qualified production caller holds pre-paid owned identity text yet;
    // the open legacy formatting sites are its intended migrations.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "explicit paid-owned migration seam; current qualified source constructions borrow text"
        )
    )]
    pub(crate) fn paid_owned<E>(
        value: String,
        reserve: impl FnOnce(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        Self::reserve_backing(value.len(), reserve)?;
        Ok(Self(DeclarationText::Shared(Arc::from(value))))
    }

    fn reserve_backing<E>(
        len: usize,
        reserve: impl FnOnce(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Some(backing) = Self::shared_backing_bytes(len) else {
            // An unaddressable spelling cannot be admitted by any payer.
            reserve(usize::MAX)?;
            panic!("declaration identity length exceeds the address space");
        };
        reserve(backing)
    }

    pub fn as_str(&self) -> &str {
        match &self.0 {
            DeclarationText::CompilerStatic(id) => id.text(),
            DeclarationText::Shared(text) => text,
        }
    }

    /// Requested allocation bytes of one shared backing for `len` text bytes:
    /// two reference counters followed by the text, padded to their alignment.
    pub(crate) fn shared_backing_bytes(len: usize) -> Option<usize> {
        let counters = std::alloc::Layout::new::<[usize; 2]>();
        let text = std::alloc::Layout::array::<u8>(len).ok()?;
        Some(counters.extend(text).ok()?.0.pad_to_align().size())
    }

    /// Heap bytes this identity references: zero for a compiler identity,
    /// otherwise its one shared backing, which every clone also references.
    pub fn owned_allocation_bytes(&self) -> usize {
        match &self.0 {
            DeclarationText::CompilerStatic(_) => 0,
            DeclarationText::Shared(text) => Self::shared_backing_bytes(text.len())
                .expect("an allocated backing has an addressable layout"),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_compiler_static(&self) -> bool {
        matches!(self.0, DeclarationText::CompilerStatic(_))
    }

    #[cfg(test)]
    pub(crate) fn shared_allocation_key(&self) -> Option<usize> {
        match &self.0 {
            DeclarationText::CompilerStatic(_) => None,
            DeclarationText::Shared(text) => Some(Arc::as_ptr(text).cast::<u8>() as usize),
        }
    }
}

impl fmt::Debug for DeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DeclarationId")
            .field(&self.as_str())
            .finish()
    }
}

impl PartialEq for DeclarationId {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for DeclarationId {}

impl PartialOrd for DeclarationId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeclarationId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for DeclarationId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

// Sound because Eq, Ord and Hash above are exactly those of the text.
impl std::borrow::Borrow<str> for DeclarationId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for DeclarationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
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

    /// Selected replay reserves both the exact temporary string and Arc backing.
    /// This constructor keeps the ordinary identity bytes without format spare.
    pub(crate) fn intrinsic_parameter_exact(operation: &str, index: usize) -> Self {
        let digits = decimal_digits(index);
        let mut value = String::with_capacity(operation.len() + 7 + digits);
        value.push_str(operation);
        value.push_str(".param.");
        write!(&mut value, "{index}").expect("write into reserved String");
        Self::new(value)
    }

    /// Exact guarded constructor: each decimal, copy, hash and Arc transfer
    /// visit is debited by the caller's existing preparation/replay ledger.
    #[cfg(test)]
    pub(crate) fn intrinsic_parameter_with_work<E>(
        operation: &str,
        mut index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut decimal = [0u8; 3 * std::mem::size_of::<usize>()];
        let mut start = decimal.len();
        loop {
            charge(1)?;
            start -= 1;
            decimal[start] = b'0' + (index % 10) as u8;
            index /= 10;
            if index == 0 {
                break;
            }
        }
        for _ in &decimal[start..] {
            charge(1)?;
        }
        let digits = std::str::from_utf8(&decimal[start..]).expect("decimal bytes");
        let mut value = String::with_capacity(operation.len() + 7 + digits.len());
        for part in [operation, ".param.", digits] {
            for _ in part.bytes() {
                charge(1)?;
            }
            value.push_str(part);
        }
        let mut fingerprint = 0xcbf29ce484222325u64;
        for byte in value.bytes() {
            charge(1)?;
            fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        for _ in value.bytes() {
            charge(1)?;
        }
        Ok(Self(Arc::from(value), fingerprint))
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

    pub(super) fn matches_local_display(
        &self,
        function: &FunctionExecutionId,
        path_length: usize,
        path: impl fmt::Display,
    ) -> bool {
        self.matches_scoped(function, "value:local", path_length, path)
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

    /// Byte equality is checked independently by the guarded receiving probe.
    /// Retain the second component of ordinary ValueId equality as well.
    #[cfg(test)]
    pub(crate) fn same_cached_fingerprint(&self, other: &Self) -> bool {
        self.1 == other.1
    }

    #[cfg(test)]
    pub(crate) fn invalidate_fingerprint_for_test(&mut self) {
        self.1 ^= 1;
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

    /// Fixed allocation bytes around an expression identity's owned text.
    /// The value is public so external bounded-accounting code can reserve
    /// carrier overhead without exposing an allocation pointer.
    pub const OWNED_ALLOCATION_CARRIER_BYTES: usize = Self::SHARED_ALLOCATION_CARRIER_BYTES;

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

    /// Hostile accounting fixture: preserve a deliberately oversized buffer.
    /// Ordinary constructors continue to normalize identity capacity exactly.
    #[cfg(test)]
    pub(crate) fn from_untrimmed_backing_for_test(value: String) -> Self {
        if !crate::bounded_output::reserve_active_required(Self::SHARED_ALLOCATION_CARRIER_BYTES) {
            return Self(None);
        }
        Self(Some(Arc::new(value)))
    }

    pub(super) fn matches(&self, function: &FunctionExecutionId, path: &str) -> bool {
        matches_scoped_identity(self.as_str(), function, "expression", path.len(), path)
    }

    pub(super) fn matches_display(
        &self,
        function: &FunctionExecutionId,
        path_length: usize,
        path: impl fmt::Display,
    ) -> bool {
        matches_scoped_identity(self.as_str(), function, "expression", path_length, path)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_deref().map_or("", String::as_str)
    }

    /// Returns the owned backing allocation size retained by this identity.
    ///
    /// The count includes the shared carrier and the exact-capacity string
    /// buffer. A missing value means construction was refused by the active
    /// allocation budget; callers that account retained memory must fail
    /// closed in that case.
    pub fn owned_allocation_bytes(&self) -> Option<usize> {
        self.shared_allocation_bytes()
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

#[cfg(test)]
#[path = "ids/declaration_backing_tests.rs"]
mod declaration_backing_tests;

#[cfg(test)]
#[path = "ids/legacy_constructor_boundary_tests.rs"]
mod legacy_constructor_boundary_tests;

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

/// Borrow the canonical path only after independently checking its full owner/domain/length encoding.
pub(super) fn checked_scoped_path<'a>(
    value: &'a str,
    owner: &FunctionExecutionId,
    kind: &str,
) -> Option<&'a str> {
    let empty = scoped_identity(owner, kind, "");
    let prefix = empty.strip_suffix("0:")?;
    let rest = value.strip_prefix(prefix)?;
    let (length, path) = rest.split_once(':')?;
    let parsed = length.parse::<usize>().ok()?;
    if length != parsed.to_string() || path.len() != parsed {
        return None;
    }
    matches_scoped_identity(value, owner, kind, parsed, path).then_some(path)
}

pub(super) mod streamed;

#[cfg(test)]
mod metered_owner;
#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;

#[cfg(test)]
pub(crate) use metered_owner::general_matches as oracle_owner_matches_with_work;

#[cfg(test)]
pub(in crate::hir) mod fixed_operation_tests;

#[path = "ids/instance_match.rs"]
pub(in crate::hir) mod instance_match;

#[cfg(test)]
#[path = "ids/declaration_tag.rs"]
mod declaration_tag;

#[cfg(test)]
mod lowering_identity_tests;
