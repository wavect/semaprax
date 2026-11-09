//! Deterministic HIR identity newtypes.
//!
//! Declaration, function-instance, execution, value, and expression
//! identities plus the exact string encodings that derive them.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;


use crate::hir::ids::compiler::CompilerDeclaration;

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
    CompilerStatic(&'static str),
    Shared(Arc<str>),
}

// The former `String` footprint, which frame constants, layout pins and
// size-of forecasts are written against, including through `Option`. These
// check the actual layout rather than assuming it.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(std::mem::size_of::<DeclarationId>() == 24);
    assert!(std::mem::size_of::<DeclarationId>() == std::mem::size_of::<String>());
    assert!(std::mem::align_of::<DeclarationId>() == std::mem::align_of::<String>());
    assert!(std::mem::size_of::<Option<DeclarationId>>() == std::mem::size_of::<Option<String>>());
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
        Self(DeclarationText::CompilerStatic(id.text()))
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
            DeclarationText::CompilerStatic(text) => text,
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


// Complete unchanged DeclarationId cache codec; outside the identity prefix.
use crate::cache_codec::{Codec, Decoder, Encoder};
impl Codec for DeclarationId {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        encoder.nested(|encoder| encoder.text(self.as_str()))
    }

    /// Decodes borrowed wire text straight into the shared backing; no
    /// temporary `String` is made. The backing is charged to two independent
    /// ledgers: the decoder's own allocation cap, and the decode caller's
    /// active output budget, passed explicitly as the construction payer. A
    /// refusal by either returns an error and constructs nothing.
    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        decoder.nested(|decoder| {
            let text = decoder.text()?;
            let refused = || {
                crate::cache_codec::capacity(
                    "cache codec declaration identity backing allocation was refused",
                )
            };
            let Some(backing) = DeclarationId::shared_backing_bytes(text.len()) else {
                return Err(refused());
            };
            decoder.allocate(backing)?;
            DeclarationId::paid_borrowed(text, |bytes| {
                if crate::bounded_output::reserve_active_required(bytes) {
                    Ok(())
                } else {
                    Err(refused())
                }
            })
        })
    }
}
